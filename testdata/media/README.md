# Amostra de mídia

`amostra.webm` é um clipe de um segundo, gerado uma única vez com `canvas` +
`MediaRecorder` (Chromium) e versionado aqui para o teste end-to-end ser
reproduzível: o `MediaRecorder` só existe em tempo de execução, e um arquivo de
texto com cara de `.mp4` nunca prova que a mídia toca.

O E2E (`tests/e2e/server.spec.ts`) copia este arquivo para a biblioteca temporária
da fixture e exige que o player decodifique de verdade (`videoWidth > 0`,
`readyState >= 2`) e que o servidor entregue os bytes com `content-type` de vídeo,
`accept-ranges: bytes` e resposta `206` para um pedido `Range`.

O arquivo tem 4 KB e é gerado sem material de terceiros: nenhum codec, imagem ou
áudio de licença chega ao repositório.
