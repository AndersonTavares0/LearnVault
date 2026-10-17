# Changelog

Todas as mudanças relevantes do LearnVault são documentadas neste arquivo.

O formato segue o Keep a Changelog e as versões usam Semantic Versioning.

## [0.4.0](https://github.com/AndersonTavares0/LearnVault/compare/v0.3.0...v0.4.0) (2026-10-17)


### Funcionalidades

* instalador Windows NSIS por usuário em pt-BR/en ([#152](https://github.com/AndersonTavares0/LearnVault/issues/152)) ([9b570e4](https://github.com/AndersonTavares0/LearnVault/commit/9b570e4fa000fe1dd34ea01e82c6593f219f53d2))
* navegar entre aulas no player com indice do curso ([#157](https://github.com/AndersonTavares0/LearnVault/issues/157)) ([613ba90](https://github.com/AndersonTavares0/LearnVault/commit/613ba90396c5a00bed85bc6ec2a384125fa678b3))
* **web:** melhorar login e troca inicial de senha ([#184](https://github.com/AndersonTavares0/LearnVault/issues/184)) ([6046867](https://github.com/AndersonTavares0/LearnVault/commit/6046867f33416cf1119566d75ba1c8e469293ff1))
* **web:** reorganizar o catalogo como curriculo escaneavel ([#175](https://github.com/AndersonTavares0/LearnVault/issues/175)) ([0312c69](https://github.com/AndersonTavares0/LearnVault/commit/0312c69eaa13b8e258b4ac139f1c052bf24ed920))


### Correções

* identidade de versao do servidor Rust por injecao ([d8cc5b5](https://github.com/AndersonTavares0/LearnVault/commit/d8cc5b52807bb1a31291eba52057d7324d298951))
* passar o repositorio ao gh release create no RC draft ([e434384](https://github.com/AndersonTavares0/LearnVault/commit/e4343846be4816c109eb19b57a289958769ea82e)), closes [#164](https://github.com/AndersonTavares0/LearnVault/issues/164)
* **web:** preservar aplicativo ao abrir mídia e materiais ([#187](https://github.com/AndersonTavares0/LearnVault/issues/187)) ([ae76ba1](https://github.com/AndersonTavares0/LearnVault/commit/ae76ba1c0c6acfe9b5fd13106694f6fe1f866d27))


### Melhorias de desempenho

* **ci:** otimizar argon2 nos builds de teste ([#168](https://github.com/AndersonTavares0/LearnVault/issues/168)) ([93a8e9f](https://github.com/AndersonTavares0/LearnVault/commit/93a8e9f19b5cb2856e1cc643513b0a2af2c823e9))


### Reversões

* **release:** desfazer o release 0.4.0 ([#192](https://github.com/AndersonTavares0/LearnVault/issues/192)) ([c4f51f3](https://github.com/AndersonTavares0/LearnVault/commit/c4f51f3ba51ddbc6279a7a7ae188e7a8668b0fd0))

## [0.3.0](https://github.com/AndersonTavares0/LearnVault/compare/v0.2.0...v0.3.0) (2026-10-17)


### Funcionalidades

* adicionar fundação Tauri offline ([552de8a](https://github.com/AndersonTavares0/LearnVault/commit/552de8ad8bd23016ddb6d37b163680c7e4932bac))
* adicionar progresso de estudo persistente ([14e7ba6](https://github.com/AndersonTavares0/LearnVault/commit/14e7ba65eea5406cdc1d4a7840115a08e40ab94b))
* avançar migração Go para Rust ([d34c8fa](https://github.com/AndersonTavares0/LearnVault/commit/d34c8fae1ce807427a513f2a197a18bd01000c88))
* **core:** adicionar remoção explícita de mídia do catálogo ([#54](https://github.com/AndersonTavares0/LearnVault/issues/54)) ([c1896aa](https://github.com/AndersonTavares0/LearnVault/commit/c1896aaa61db4670f72747181c733da2392c22fe))
* criar o workspace Rust e o esqueleto do núcleo ([#44](https://github.com/AndersonTavares0/LearnVault/issues/44)) ([1fb09a4](https://github.com/AndersonTavares0/LearnVault/commit/1fb09a4214f4e0eeb3804d7a3bab82306768f8bc))
* derivar o catálogo de cursos, módulos e aulas ([#50](https://github.com/AndersonTavares0/LearnVault/issues/50)) ([2ed0c06](https://github.com/AndersonTavares0/LearnVault/commit/2ed0c0638089eec4e19dd4bbc54690b13fbc242c))
* **desktop:** integrar progresso de estudo via IPC ([3b568f4](https://github.com/AndersonTavares0/LearnVault/commit/3b568f4256095ee5efb6fab27bb7764065635444))
* **desktop:** servir arquivos pequenos offline por media ID ([a32464d](https://github.com/AndersonTavares0/LearnVault/commit/a32464dc2fb644a796ea5ddb9acba383fd6b8c4e))
* expor migração Go pelo CLI Rust ([b8b1635](https://github.com/AndersonTavares0/LearnVault/commit/b8b16356c95e1848b6bb6d36b09ae409fa7ea364))
* **go:** alinhar Study com a API Rust ([2dc4b39](https://github.com/AndersonTavares0/LearnVault/commit/2dc4b397dc917dd449e1c209767a2910699cd00c))
* implementar a política de rescan e a persistência da descoberta ([#49](https://github.com/AndersonTavares0/LearnVault/issues/49)) ([81594fe](https://github.com/AndersonTavares0/LearnVault/commit/81594fec830d43ab93cf50cf260af2c8dcdaaf1a))
* implementar o scanner e a identidade de mídia do núcleo ([#48](https://github.com/AndersonTavares0/LearnVault/issues/48)) ([53ee93d](https://github.com/AndersonTavares0/LearnVault/commit/53ee93dc265b8645ecdfb282376801efe47d8f71))
* implementar persistência SQLite do núcleo em Rust ([#46](https://github.com/AndersonTavares0/LearnVault/issues/46)) ([c827882](https://github.com/AndersonTavares0/LearnVault/commit/c8278824cf3b324ed336a90d5efa01028cb31c7b))
* migrar SPA para Vite e endurecer primeiro acesso ([#67](https://github.com/AndersonTavares0/LearnVault/issues/67)) ([2470657](https://github.com/AndersonTavares0/LearnVault/commit/2470657af2685da228acf796ce7707ebe0ce4ecd))
* persistir e atualizar o progresso de estudo ([#52](https://github.com/AndersonTavares0/LearnVault/issues/52)) ([d70af71](https://github.com/AndersonTavares0/LearnVault/commit/d70af711c7ebfd19c05432dcd57510eb16a9ad2f))
* playback desktop e release Windows ([c93a0a5](https://github.com/AndersonTavares0/LearnVault/commit/c93a0a5ad8a221f10dfcd0339854b3de1d283771))
* resolver mídia por identificador com confinamento na biblioteca ([#51](https://github.com/AndersonTavares0/LearnVault/issues/51)) ([5a6cf4d](https://github.com/AndersonTavares0/LearnVault/commit/5a6cf4d0001c32e0da04a64592a98527d92660c2))
* selecionar e persistir raiz da biblioteca desktop ([278ec8a](https://github.com/AndersonTavares0/LearnVault/commit/278ec8a82351478e7e04875a39a2581c87cec971))
* **server:** adicionar catálogo autenticado e rescan administrativo (F2.4) ([d215753](https://github.com/AndersonTavares0/LearnVault/commit/d21575352d8fb427bfe57a6fec9a042b6e68b07a))
* **server:** entregar mídia autenticada com Range (F2.3) ([5889045](https://github.com/AndersonTavares0/LearnVault/commit/5889045cca2478c354b851d6f42ca412f0845862))
* **server:** expose study capabilities ([e3d9caf](https://github.com/AndersonTavares0/LearnVault/commit/e3d9caf5238ffc4044f3724386912b37ed28ed76))
* **server:** implementar autenticação e sessões (F2.2) ([125aeb6](https://github.com/AndersonTavares0/LearnVault/commit/125aeb616fb4145a1f7966a4dbb4188e60cfa595))
* **server:** iniciar infraestrutura Axum e healthz ([#56](https://github.com/AndersonTavares0/LearnVault/issues/56)) ([9619013](https://github.com/AndersonTavares0/LearnVault/commit/96190130b4b81163e9b9e0a9d713d8d2b52a0f1a))
* **server:** servir SPA estatica e endurecer a entrega ([#64](https://github.com/AndersonTavares0/LearnVault/issues/64)) ([df7bc92](https://github.com/AndersonTavares0/LearnVault/commit/df7bc927d3ef68fd4f4a7ca3c2546e136cde450d))
* usar portal para seleção da biblioteca Linux ([e133a88](https://github.com/AndersonTavares0/LearnVault/commit/e133a88fe70540534ac3e72fa83eb4b87973c7fa))
* **web:** adicionar busca client-side ao catálogo ([ad25013](https://github.com/AndersonTavares0/LearnVault/commit/ad25013a279dca31f7b61491ab4cf1a899cc6c36))
* **web:** integrar progresso de estudo com sincronização segura ([5e7866b](https://github.com/AndersonTavares0/LearnVault/commit/5e7866b80fae1709be856356fba7d1d0878947b8))


### Correções

* aceitar binario static-pie no smoke do RC Rust ([31970f6](https://github.com/AndersonTavares0/LearnVault/commit/31970f692afecc4e52bfdb490b6934dbd12615f6))
* **ci:** falhar fechado no estado de alerta do auto-merge ([b40b400](https://github.com/AndersonTavares0/LearnVault/commit/b40b4004ec442704b2e8eefef3069ac88a4bca90))
* **core:** classificar diretórios de mídia corretamente ([b85433f](https://github.com/AndersonTavares0/LearnVault/commit/b85433f75851acfde1853017f602bb207da64823))
* **release:** include Apache license in Go archives ([aac814c](https://github.com/AndersonTavares0/LearnVault/commit/aac814c213b414dcb6ae0d841c0dfc22f952c51f))
* **release:** validate published tag via environment ([97f0de7](https://github.com/AndersonTavares0/LearnVault/commit/97f0de76be91ba326c79aaad92180a5008dd9a67))
* **server:** make idle media timeout deterministic ([63412fe](https://github.com/AndersonTavares0/LearnVault/commit/63412fe3ff578a09f27e846c125eba62fa8c37d6))
* **test:** estabilizar a drenagem do stream ocioso da SPA ([#65](https://github.com/AndersonTavares0/LearnVault/issues/65)) ([5c81318](https://github.com/AndersonTavares0/LearnVault/commit/5c813182b9a8210ff838d53b1a34e747d3e6a21f))
* **verify:** make pnpm verify a real gate ([1629f38](https://github.com/AndersonTavares0/LearnVault/commit/1629f3838117b23b9e71fcc1be1a9170aae7eeab))

## [0.2.0](https://github.com/AndersonTavares0/LearnVault/compare/v0.1.0...v0.2.0) (2026-10-15)


### Funcionalidades

* automate releases with Release Please ([#26](https://github.com/AndersonTavares0/LearnVault/issues/26)) ([2584724](https://github.com/AndersonTavares0/LearnVault/commit/2584724d3bbbe58c354e204034f761e8583d33dd))


### Correções

* remove invalid Release Please input ([#38](https://github.com/AndersonTavares0/LearnVault/issues/38)) ([a9aff27](https://github.com/AndersonTavares0/LearnVault/commit/a9aff27e54db4d6952088be2fbbc73c21882d3c2))

## [Unreleased]

### Adicionado

- Seleção de biblioteca no Linux configurada com o backend oficial `xdg-portal` do
  `tauri-plugin-dialog`; o manifesto Flatpak removeu o acesso amplo ao filesystem. O build e o
  smoke test reais no sandbox Flatpak, incluindo acesso persistente e reprodução de mídia,
  continuam pendentes, e a automação de release permanece desativada.

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
