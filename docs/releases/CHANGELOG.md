# Changelog

Todas as mudanças relevantes do LearnVault são documentadas neste arquivo.

O formato segue o Keep a Changelog e as versões usam Semantic Versioning.

## [0.2.0](https://github.com/AndersonTavares0/LearnVault/compare/v0.1.0...v0.2.0) (2026-10-15)


### Funcionalidades

* automate releases with Release Please ([#26](https://github.com/AndersonTavares0/LearnVault/issues/26)) ([2584724](https://github.com/AndersonTavares0/LearnVault/commit/2584724d3bbbe58c354e204034f761e8583d33dd))


### Correções

* remove invalid Release Please input ([#38](https://github.com/AndersonTavares0/LearnVault/issues/38)) ([a9aff27](https://github.com/AndersonTavares0/LearnVault/commit/a9aff27e54db4d6952088be2fbbc73c21882d3c2))

## [Unreleased]

### Adicionado

- Fundação do aplicativo desktop em `crates/desktop` (Tauri 2.12.x): shell da SPA embutido por `frontendDist`, capability deny-by-default para a janela principal, CSP explícita e nenhum listener de rede ou plugin privilegiado.
- IPC de catálogo e configuração no desktop: comandos `desktop_config` e `desktop_catalog` offline sobre o `learnvault-core`, com ACL app-defined mínima, DTOs snake_case do catálogo e estado offline determinístico ("biblioteca não configurada") que nunca oferece login no desktop. Entrega de mídia, rescan e autenticação continuam pendentes.
- Seleção e persistência segura da raiz da biblioteca no desktop: o comando `desktop_select_library`, sem argumentos, aciona o seletor nativo via `tauri-plugin-dialog` interno ao Rust (não concedido ao webview) e devolve apenas `{ configured }`. A raiz é validada (existência, diretório, canonicalização, recusa de link/junction e leitura) e gravada de forma versionada, atômica e serializada (temporário exclusivo com `create_new` e substituição por `rename`, sem apagar antes) no diretório de configuração do aplicativo, fora da biblioteca e do SQLite; a biblioteca permanece somente leitura e revalidada no startup. O webview nunca envia nem recebe caminho. Rescan, mídia e autenticação continuam pendentes.
- Rescan explícito da biblioteca no desktop: o comando `desktop_rescan`, sem argumentos e assíncrono, indexa a raiz persistida em `tauri::async_runtime::spawn_blocking` reutilizando o `learnvault-core` (`rescan`/`RescanSummary`) sem duplicar domínio. O rescan nunca é automático — nem no startup, nem após a seleção da raiz — e a biblioteca externa permanece somente leitura, com o banco SQLite fora dela. Um estado ocupado explícito serializa chamadas concorrentes, que recebem o erro estável `rescan_in_progress` sem cancelar nem corromper o estado; as falhas de raiz não configurada (`library_not_configured`) e raiz inacessível (`rescan_unavailable`) são erros serializáveis por código, sem caminhos. Na SPA, o transporte desktop dispara o comando por `invoke` (sem `fetch`) e o shell oferece um controle de indexação apenas quando há biblioteca configurada, com estados de carregamento, sucesso e erro sanitizado; o rescan HTTP permanece inalterado. A raiz é revalidada imediatamente antes de cada varredura, recusando uma raiz removida, inacessível ou substituída por link/junction, e a varredura do núcleo não segue um link no topo. O resumo do rescan expõe apenas contagens — nunca as chaves relativas ambíguas. Limitação conhecida desta fatia: o cadeado do banco SQLite é mantido durante a varredura, então uma leitura concorrente do catálogo aguarda a indexação terminar; um segundo rescan é recusado pelo estado ocupado. Mídia e autenticação continuam pendentes.
- Entrega de **arquivos pequenos** no desktop por protocolo customizado: o Tauri registra `learnvault-media://localhost/<media-id>` e o webview aponta pelo `media_id` opaco, nunca por um caminho. O parsing é estrito (esquema, host, método `GET`/`HEAD`, caminho com um único segmento, sem query e identificador de alfabeto seguro) e a mídia é resolvida somente no catálogo e aberta uma única vez pelo `learnvault-core` (`open_media`), que revalida a raiz, a disponibilidade, o confinamento sob a raiz, o componente final (link simbólico) e a identidade do descritor aberto; o `File` já aberto é o que é lido. Só imagens/thumbnails e materiais pequenos entram, por allowlist de tipo de conteúdo, com limite explícito de resposta (`200`/`HEAD`/`206`/`416`, `Range` de faixa única) e headers seguros (`Accept-Ranges`, `Content-Length`, `Content-Range`, `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`); nenhum corpo ou header carrega caminho ou erro de IO, e a biblioteca permanece somente leitura. A CSP libera a origem customizada e o ACL continua deny-by-default, sem capability nova nem plugin de fs/http/shell/process/updater/store/remote. **Limitação declarada desta fatia:** só arquivos pequenos — **vídeo e áudio ficam fora do protocolo** por causa da reprodução parcial incompatível no webview do Linux (`WebKitGTK`), e o `Player` mantém a reprodução de vídeo/áudio do desktop explicitamente indisponível; legendas continuam no caminho HTTP (a conversão de SRT sob demanda é `fetch` e só existe no build HTTP). A Fase 4 **não** está concluída.

### Correções

- Catálogo recursivo preserva aulas e arquivos em qualquer profundidade, com rescan seguro contra resultados parciais.

### Planejado

- Player focado em estudo e persistência de progresso.
- Curadoria de cursos, thumbnails, busca, anotações e favoritos.

## [0.1.0] - 2026-10-14

### Adicionado

- Governança inicial do projeto e documentação de contribuição.
- Políticas, playbooks, checklists e template de ADR em `.agents/`.
- Layout canônico da documentação e fluxo controlado de sincronização da Wiki.

### Alterado

- Padronização do repositório em torno de Go 1.26, Node.js 24, pnpm 11.28.5 e lockfile congelado do pnpm.
- Atualização de `golang.org/x/crypto` para v0.58.0 e `modernc.org/sqlite` para v1.60.1 junto com o toolchain Go 1.26.
- Manutenção da saída gerada da SPA, dados de runtime, caches, segredos e ferramentas locais fora do Git.

### Segurança

- A biblioteca externa de cursos permanece somente para leitura e fora do controle de versão,
  dos testes e dos backups.
