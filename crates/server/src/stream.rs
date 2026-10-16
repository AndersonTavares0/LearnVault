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
            // A leitura é I/O da origem, não ociosidade do consumidor. O watchdog mede
            // apenas quanto tempo o transporte deixa de retirar dados, então a leitura
            // NÃO disputa o deadline: um chunk já lido nunca é descartado só porque o
            // deadline expirou no mesmo instante (era a origem do flake da CI, que via o
            // body terminar vazio antes de qualquer frame).
            let read = match reader.read(&mut buffer).await {
                Ok(read) => read,
                Err(error) => {
                    // Erro de origem é terminal; entregue-o ainda sob a guarda de
                    // ociosidade para não reter capacidade se o consumidor já parou.
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
                // O produtor pode descobrir EOF antes de o transporte consultar o último frame.
                // Mantenha o slot enquanto o body ainda está retido; `poll_next` libera-o assim
                // que o consumidor observa `None`, e drop/timeout cobrem os demais casos.
                deadline.as_mut().await;
                release_permit(&task_permit);
                return;
            }

            let chunk = Bytes::copy_from_slice(&buffer[..read]);
            // `biased` prioriza a entrega: se o deadline expirou mas ainda há espaço no
            // canal, o chunk disponível é enviado em vez de descartado pelo timeout. O
            // deadline só encerra quando o envio realmente bloquearia (consumidor parado).
            tokio::select! {
                biased;
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
    // `Mutex` padrão nunca é envenenado por `panic` em outro executor aqui, mas um
    // `expect` em `poll_next`/`Drop` transformaria veneno em `panic` por request:
    // recupere o guarda e libere o permit de qualquer forma.
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
                // EOF observado pelo consumidor encerra a vida útil do body. O produtor pode
                // já ter terminado, mas não deve reter a capacidade até o timeout ocioso.
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

    /// Leitor cuja primeira leitura só completa depois de `delay`: simula o IO de
    /// arquivo que termina no mesmo instante (ou depois) do deadline ocioso expirar.
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

    /// Causa-raiz do flake de
    /// `idle_media_streams_release_capacity_without_reading_the_body`: o watchdog
    /// encerrava o body quando o deadline expirava no mesmo instante em que a leitura
    /// terminava, descartando um chunk já lido e deixando o body vazio (`frame()`
    /// devolvia `None`). A leitura é IO da origem, não ociosidade do consumidor, então
    /// não pode disputar o deadline. Aqui a leitura só completa 1s depois do deadline e
    /// ainda assim é entregue — o que falhava antes da correção.
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
