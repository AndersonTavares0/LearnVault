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

- Estado das fatias de mídia desktop: rescan explícito e entrega de materiais, vídeo, áudio e
  arquivos de legenda estão implementados pelo protocolo customizado, com faixas únicas e limite
  explícito por resposta. O player desktop renderiza VTT; SRT, ASS e SSA são baixáveis, e o build
  HTTP pode converter SRT sob demanda. Autenticação, janelas reais e distribuição por plataforma
  continuam pendentes.
- Paridade do ciclo de Study entre o servidor Rust, a SPA web e o serviço Go de referência:
  progresso, retomada, conclusão e histórico recente compartilham o mesmo contrato. O desktop
  expõe o mesmo contrato por IPC com o dono local fixo e sincroniza a reprodução local.
- Progresso de estudo no desktop por IPC: os comandos `desktop_study_progress`,
  `desktop_record_position`, `desktop_set_completed` e `desktop_recent_study`, assíncronos
  (`spawn_blocking`) e com ACL app-defined mínima, reutilizam `learnvault-core::study` sem
  duplicar regras. O dono do progresso é um identificador local fixo escolhido pelo Rust: nenhum
  comando aceita `owner_id`, então o webview só lê e grava o progresso do dono local. Os
  comandos recebem apenas `media_id`, posição, conclusão e limite, devolvem o mesmo contrato
  snake_case do adaptador HTTP (`recorded`, `updated_at`) e traduzem as falhas para códigos
  estáveis (`media_not_found`, `invalid_position`, `study_unavailable`, `database_unavailable`)
  sem ecoar identificadores nem caminhos. Na SPA, o transporte desktop implementa Study por
  `invoke` — sem `fetch` — e a barra de estudo do player é habilitada quando o transporte entrega
  uma fonte para a mídia. Limitação conhecida desta fatia: apenas um dono local, sem sessão nem
  seletor de contas.
- Fundação do aplicativo desktop em `crates/desktop` (Tauri 2.12.x): shell da SPA embutido por `frontendDist`, capability deny-by-default para a janela principal, CSP explícita e nenhum listener de rede ou plugin privilegiado.
- IPC de catálogo e configuração no desktop: comandos `desktop_config` e `desktop_catalog` offline sobre o `learnvault-core`, com ACL app-defined mínima, DTOs snake_case do catálogo e estado offline determinístico ("biblioteca não configurada") que nunca oferece login no desktop. Autenticação e janelas reais continuam pendentes.
- Seleção e persistência segura da raiz da biblioteca no desktop: o comando `desktop_select_library`, sem argumentos, aciona o seletor nativo via `tauri-plugin-dialog` interno ao Rust (não concedido ao webview) e devolve apenas `{ configured }`. A raiz é validada (existência, diretório, canonicalização, recusa de link/junction e leitura) e gravada de forma versionada, atômica e serializada (temporário exclusivo com `create_new` e substituição por `rename`, sem apagar antes) no diretório de configuração do aplicativo, fora da biblioteca e do SQLite; a biblioteca permanece somente leitura e revalidada no startup. O webview nunca envia nem recebe caminho. Autenticação e janelas reais continuam pendentes.
- Rescan explícito da biblioteca no desktop: o comando `desktop_rescan`, sem argumentos e assíncrono, indexa a raiz persistida em `tauri::async_runtime::spawn_blocking` reutilizando o `learnvault-core` (`rescan`/`RescanSummary`) sem duplicar domínio. O rescan nunca é automático — nem no startup, nem após a seleção da raiz — e a biblioteca externa permanece somente leitura, com o banco SQLite fora dela. Um estado ocupado explícito serializa chamadas concorrentes, que recebem o erro estável `rescan_in_progress` sem cancelar nem corromper o estado; as falhas de raiz não configurada (`library_not_configured`) e raiz inacessível (`rescan_unavailable`) são erros serializáveis por código, sem caminhos. Na SPA, o transporte desktop dispara o comando por `invoke` (sem `fetch`) e o shell oferece um controle de indexação apenas quando há biblioteca configurada, com estados de carregamento, sucesso e erro sanitizado; o rescan HTTP permanece inalterado. A raiz é revalidada imediatamente antes de cada varredura, recusando uma raiz removida, inacessível ou substituída por link/junction, e a varredura do núcleo não segue um link no topo. O resumo do rescan expõe apenas contagens — nunca as chaves relativas ambíguas. Limitação conhecida desta fatia: o cadeado do banco SQLite é mantido durante a varredura, então uma leitura concorrente do catálogo aguarda a indexação terminar; um segundo rescan é recusado pelo estado ocupado. Autenticação e janelas reais continuam pendentes.
- Entrega de mídia no desktop por protocolo customizado: o Tauri registra `learnvault-media://localhost/<media-id>` e o webview aponta pelo `media_id` opaco, nunca por um caminho. O parsing é estrito (esquema, host, método `GET`/`HEAD`, caminho com um único segmento, sem query e identificador de alfabeto seguro) e a mídia é resolvida somente no catálogo e aberta uma única vez pelo `learnvault-core` (`open_media`), que revalida a raiz, a disponibilidade, o confinamento sob a raiz e o componente final; a identidade do descritor aberto é confirmada por `dev`/`ino` no Unix e, no Windows, pela abertura do componente final **sem seguir** um ponto de reparse (link/junction), e o `File` já aberto é o que é lido. Uma raiz configurada que tenha sido substituída por link/junction é recusada, nunca seguida. Limitação residual no Windows: uma troca do componente final por **hardlink** ou a troca de um componente **intermediário** entre a validação e a abertura não é detectada (`std` estável não expõe `dev`/`ino`), registrada em `crates/core/src/media.rs`. Materiais, vídeo, áudio e legendas entram na allowlist de tipo de conteúdo, com limite explícito de resposta (200/HEAD/206/413/416, Range de faixa única) e headers seguros (Accept-Ranges, Content-Length, Content-Range, Cache-Control: no-store, X-Content-Type-Options: nosniff); nenhum corpo ou header carrega caminho ou erro de IO, e a biblioteca permanece somente leitura. O player desktop usa a fonte local para reprodução e Study; a conversão de SRT sob demanda continua disponível somente no build HTTP. A Fase 4 **não** está concluída.

### Correções

- Catálogo recursivo preserva aulas e arquivos em qualquer profundidade, com rescan seguro contra resultados parciais.
- Protocolo de mídia no desktop: a SPA passa a endereçar `http://learnvault-media.localhost/<media-id>` no Windows (WebView2), a única forma que o WebView2 resolve para um esquema customizado; o runtime reverte para `learnvault-media://localhost/<media-id>` antes do handler, e Linux (WebKitGTK) e macOS (WKWebView) seguem com o esquema direto. Sem mudança de contrato, capability, CSP nem de transporte HTTP.

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
