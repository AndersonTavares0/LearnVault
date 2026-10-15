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

/// Produz o corpo fora do `Body`, para que o prazo de ociosidade continue valendo mesmo quando o
/// transporte deixa de consultar o stream. O produtor é dono do reader e do permit: ao expirar,
/// ambos são descartados e o canal é fechado, encerrando o body após qualquer chunk já enfileirado.
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
            let read = tokio::select! {
                _ = &mut deadline => {
                    release_permit(&task_permit);
                    return;
                },
                read = reader.read(&mut buffer) => match read {
                    Ok(read) => read,
                    Err(error) => {
                        let _ = sender.send(Err(error)).await;
                        release_permit(&task_permit);
                        return;
                    }
                },
            };
            if read == 0 {
                drop(reader);
                drop(sender);
                deadline.as_mut().await;
                release_permit(&task_permit);
                return;
            }

            let chunk = Bytes::copy_from_slice(&buffer[..read]);
            tokio::select! {
                _ = &mut deadline => {
                    release_permit(&task_permit);
                    return;
                },
                sent = sender.send(Ok(chunk)) => {
                    if sent.is_err() {
                        return;
                    }
                    // O primeiro chunk apenas ocupa o buffer. Cada envio seguinte só completa
                    // depois que o consumidor retirou o anterior, portanto é progresso real.
                    if sent_chunk {
                        deadline.as_mut().reset(Instant::now() + timeout);
                    }
                    sent_chunk = true;
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
    drop(
        permit
            .lock()
            .expect("stream permit mutex is not poisoned")
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
        Pin::new(&mut self.get_mut().receiver).poll_next(cx)
    }
}

impl Drop for IdleTimeoutStream {
    fn drop(&mut self) {
        release_permit(&self.permit);
        self.task.abort();
    }
}
