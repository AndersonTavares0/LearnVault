use std::{
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, ReadBuf},
    sync::{OwnedSemaphorePermit, mpsc},
};

/// Encerra um stream sem progresso sem depender de o transporte continuar consultando o corpo.
pub(crate) struct IdleTimeoutReader<R> {
    reader: R,
    permit: Arc<Mutex<Option<OwnedSemaphorePermit>>>,
    activity: mpsc::Sender<()>,
    timed_out: Arc<AtomicBool>,
}

impl<R> IdleTimeoutReader<R> {
    pub(crate) fn new(reader: R, permit: OwnedSemaphorePermit, timeout: Duration) -> Self {
        let permit = Arc::new(Mutex::new(Some(permit)));
        let timed_out = Arc::new(AtomicBool::new(false));
        let (activity, mut activity_receiver) = mpsc::channel(1);
        let timeout_permit = permit.clone();
        let timeout_expired = timed_out.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(timeout) => {
                        timeout_expired.store(true, Ordering::Release);
                        drop(
                            timeout_permit
                                .lock()
                                .expect("stream permit mutex is not poisoned")
                                .take(),
                        );
                        return;
                    }
                    activity = activity_receiver.recv() => {
                        if activity.is_none() {
                            drop(
                                timeout_permit
                                    .lock()
                                    .expect("stream permit mutex is not poisoned")
                                    .take(),
                            );
                            return;
                        }
                    }
                }
            }
        });
        Self {
            reader,
            permit,
            activity,
            timed_out,
        }
    }

    fn release_permit(&self) {
        drop(
            self.permit
                .lock()
                .expect("stream permit mutex is not poisoned")
                .take(),
        );
    }
}

impl<R> Drop for IdleTimeoutReader<R> {
    fn drop(&mut self) {
        self.release_permit();
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for IdleTimeoutReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if this.timed_out.load(Ordering::Acquire) {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "stream idle timeout",
            )));
        }
        let filled = buf.filled().len();
        match Pin::new(&mut this.reader).poll_read(cx, buf) {
            Poll::Ready(Ok(())) if buf.filled().len() == filled => {
                this.release_permit();
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Ok(())) => {
                let _ = this.activity.try_send(());
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(error)) => {
                this.release_permit();
                Poll::Ready(Err(error))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
