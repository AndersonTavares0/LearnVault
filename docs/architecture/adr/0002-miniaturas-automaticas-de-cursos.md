# ADR 0002: Gerar miniaturas de cursos no WebView

**Status:** Accepted

## Contexto

As capas atuais são geradas a partir do título do curso. Uma miniatura extraída da primeira
mídia de vídeo disponível na sequência de aulas pode tornar os cards mais reconhecíveis, mas
o LearnVault precisa continuar offline no desktop e nunca escrever nas pastas dos cursos.
O desktop entrega mídia por um protocolo Tauri próprio, cuja origem é diferente da SPA e varia
por plataforma; exportar um frame para canvas exige uma origem limpa e, portanto, CORS estrito.
O cache deve ser descartável, limitado e separado por perfil.

## Decisão

Usar o decoder de vídeo nativo do WebView e canvas para capturar o frame, sem adicionar FFmpeg
ou outro executável/dependência de codec. Gerar sob demanda quando o card entrar na área visível;
guardar JPEGs em IndexedDB local, com chave por perfil, curso, mídia, tamanho, `mtime_seconds` e
versão do gerador. No desktop, o protocolo de mídia só emitirá `Access-Control-Allow-Origin`
quando o `Origin` da requisição for exatamente a origem da janela Tauri que a iniciou; não usar
wildcard nem credenciais. Falhas de origem, CORS, decode, canvas ou cache mantêm a capa gerada
por título. A precedência será capa personalizada, quando disponível, depois miniatura automática
e, por fim, capa gerada; implementar upload manual continua fora deste ADR e pertence à issue
#237.

## Consequências

- O desktop e o servidor usam os decoders que já precisam para reproduzir vídeo, sem binário,
  custo de empacotamento ou nova licença de codec.
- A extração é lazy e limitada: até duas tarefas simultâneas, frame em 10% da duração limitado
  a 10 segundos, imagem com até 320×180, JPEG, máximo de 256 KiB por imagem, 256 imagens e
  64 MiB por perfil; remover entradas antigas quando os limites forem excedidos.
- A revisão da mídia (`size` e `mtime_seconds`) precisa estar disponível no DTO de catálogo para
  invalidar cache sem enviar caminhos absolutos. Caminhos de arquivos nunca são armazenados no
  cache nem enviados ao IPC.
- O IndexedDB pode ser limpo pelo navegador/ambiente. A miniatura é derivada e descartável; se
  não puder ser criada/lida, o card segue usando a capa gerada existente.
- A origem CORS é obtida da janela identificada pelo contexto do protocolo e comparada com o
  cabeçalho `Origin`; respostas não refletem origens arbitrárias, não permitem credenciais e
  devem variar por `Origin`. A captura usa modo CORS anônimo.

## Validação e rollback

- Testar com vídeos sintéticos e perfis/cache temporários: frame/canvas válido, formato/tamanho,
  cache hit, invalidação por alteração de tamanho/mtime/versão, limite/evicção e isolamento de
  perfis.
- Testar CORS desktop: apenas a origem exata recebe ACAO; origem ausente, diferente,
  malformada ou arbitrária não recebe ACAO e não permite exportar o canvas. A requisição
  de mídia continua sendo atendida para não afetar a reprodução normal; exercitar captura e
  fallback nos builds HTTP e desktop.
- Verificar visualmente cards em viewport desktop/mobile e confirmar fallback após decode/CORS
  falhar; executar os checks SPA e desktop documentados no playbook.
- Antes de habilitar a miniatura em cada plataforma desktop suportada, validar no WebView real
  daquela plataforma que a resposta do protocolo permite a exportação do canvas. Se falhar,
  manter o fallback; não ampliar a allowlist nem enfraquecer CSP para contornar a falha.
- Rollback: remover o caminho de geração/leitura e a tabela/versão do IndexedDB. Não é necessária
  migração nem restauração de dados do curso, pois a biblioteca nunca é modificada e o cache é
  descartável.
