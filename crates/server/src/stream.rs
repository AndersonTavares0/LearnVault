use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use axum::body::Bytes;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::{OwnedSemaphorePermit, mpsc},
    time::{Instant, Sleep},
};
use tokio_stream::{Stream, wrappers::ReceiverStream};

const CHUNK_SIZE: usize = 8 * 1024;

/// Produces the body outside the `Body`, so that the idle deadline keeps applying even when the
/// transport stops polling the stream. The producer owns the reader and the permit: on expiry,
/// both are dropped and the channel is closed, ending the body after any chunk already queued.
pub(crate) fn idle_timeout_stream<R>(
    reader: R,
    permit: OwnedSemaphorePermit,
    timeout: Duration,
) -> IdleTimeoutStream
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let (sender, receiver) = mpsc::channel(1);
    let permit = Arc::new(Mutex::new(Some(permit)));
    let task_permit = permit.clone();
    let task = tokio::spawn(async move {
        let mut reader = reader;
        let mut buffer = vec![0; CHUNK_SIZE];
        let mut sent_chunk = false;
        let mut deadline: std::pin::Pin<Box<Sleep>> = Box::pin(tokio::time::sleep(timeout));

        loop {
            // The read is the source's I/O, not consumer idleness. The watchdog measures
            // only how long the transport stops taking data, so the read
            // does NOT contend for the deadline: a chunk already read is never discarded just because the
            // deadline expired at the same instant (that was the origin of the CI flake, which saw the
            // body end empty before any frame).
            let read = match reader.read(&mut buffer).await {
                Ok(read) => read,
                Err(error) => {
                    // A source error is terminal; deliver it still under the idle
                    // guard so as not to hold capacity if the consumer has already stopped.
                    tokio::select! {
                        _ = sender.send(Err(error)) => {}
                        _ = &mut deadline => {}
                    }
                    release_permit(&task_permit);
                    return;
                }
            };
            if read == 0 {
                drop(reader);
                drop(sender);
                // The producer may discover EOF before the transport polls the last frame.
                // Keep the slot while the body is still held; `poll_next` releases it as
                // soon as the consumer observes `None`, and drop/timeout cover the other cases.
                deadline.as_mut().await;
                release_permit(&task_permit);
                return;
            }

            let chunk = Bytes::copy_from_slice(&buffer[..read]);
            // `biased` prioritizes delivery: if the deadline expired but there is still room in the
            // channel, the available chunk is sent instead of being dropped by the timeout. The
            // deadline only ends things when the send would really block (stalled consumer).
            tokio::select! {
                biased;
                sent = sender.send(Ok(chunk)) => {
                    if sent.is_err() {
                        return;
                    }
                    // The first chunk only occupies the buffer. Each following send only completes
                    // after the consumer took the previous one, so it is real progress.
                    if sent_chunk {
                        deadline.as_mut().reset(Instant::now() + timeout);
                    }
                    sent_chunk = true;
                }
                _ = &mut deadline => {
                    release_permit(&task_permit);
                    return;
                }
            }
        }
    });
    IdleTimeoutStream {
        receiver: ReceiverStream::new(receiver),
        permit,
        task: task.abort_handle(),
    }
}

fn release_permit(permit: &Arc<Mutex<Option<OwnedSemaphorePermit>>>) {
    // A standard `Mutex` is never poisoned by a `panic` in another executor here, but an
    // `expect` in `poll_next`/`Drop` would turn poison into a `panic` per request:
    // recover the guard and release the permit anyway.
    drop(
        permit
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take(),
    );
}

pub(crate) struct IdleTimeoutStream {
    receiver: ReceiverStream<std::io::Result<Bytes>>,
    permit: Arc<Mutex<Option<OwnedSemaphorePermit>>>,
    task: tokio::task::AbortHandle,
}

impl Stream for IdleTimeoutStream {
    type Item = std::io::Result<Bytes>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.receiver).poll_next(cx) {
            Poll::Ready(None) => {
                // EOF observed by the consumer ends the body's lifetime. The producer may
                // have already finished, but must not hold the capacity until the idle timeout.
                release_permit(&this.permit);
                Poll::Ready(None)
            }
            result => result,
        }
    }
}

impl Drop for IdleTimeoutStream {
    fn drop(&mut self) {
        release_permit(&self.permit);
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
        time::Duration,
    };

    use tokio::{
        io::{AsyncRead, ReadBuf},
        sync::Semaphore,
        time::Sleep,
    };
    use tokio_stream::StreamExt;

    use super::{CHUNK_SIZE, idle_timeout_stream};

    /// Reader whose first read only completes after `delay`: it simulates the file
    /// I/O that finishes at the same instant as (or after) the idle deadline expires.
    struct DelayedReader {
        chunk: Option<Vec<u8>>,
        sleep: Pin<Box<Sleep>>,
    }

    impl DelayedReader {
        fn new(delay: Duration, chunk: Vec<u8>) -> Self {
            Self {
                chunk: Some(chunk),
                sleep: Box::pin(tokio::time::sleep(delay)),
            }
        }
    }

    impl AsyncRead for DelayedReader {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let this = self.get_mut();
            match this.sleep.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    if let Some(chunk) = this.chunk.take() {
                        buf.put_slice(&chunk);
                    }
                    Poll::Ready(Ok(()))
                }
            }
        }
    }

    /// Root cause of the flake in
    /// `idle_media_streams_release_capacity_without_reading_the_body`: the watchdog
    /// ended the body when the deadline expired at the same instant the read
    /// finished, discarding a chunk already read and leaving the body empty (`frame()`
    /// returned `None`). The read is the source's I/O, not consumer idleness, so it
    /// cannot contend for the deadline. Here the read only completes 1s after the deadline and
    /// is still delivered — which failed before the fix.
    #[tokio::test(start_paused = true)]
    async fn idle_timeout_stream_delivers_a_chunk_read_after_the_deadline() {
        let timeout = Duration::from_secs(30);
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = semaphore.clone().try_acquire_owned().unwrap();
        let reader = DelayedReader::new(timeout + Duration::from_secs(1), vec![b'x'; CHUNK_SIZE]);
        let mut stream = idle_timeout_stream(reader, permit, timeout);

        let first = stream
            .next()
            .await
            .expect("chunk lido depois do deadline deve ser entregue");
        assert!(first.is_ok(), "nenhum erro de IO em stream ocioso");
        assert!(
            stream.next().await.is_none(),
            "o body fecha limpo depois do chunk"
        );
        assert!(
            semaphore.try_acquire_owned().is_ok(),
            "o watchdog libera o permit ao encerrar"
        );
    }
}
