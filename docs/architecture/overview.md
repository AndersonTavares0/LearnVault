# Arquitetura

O LearnVault transforma pastas locais de cursos em uma plataforma pessoal de estudos, usada de
duas formas:

- **Servidor self-hosted**, com WebUI acessada pela rede.
- **Desktop offline**, executado localmente sem expor portas de rede.

## Estado atual

O LearnVault roda no workspace Rust: `crates/server` (Axum), `crates/desktop` (Tauri 2) e
`apps/web` (Svelte 5 + Vite), com o domínio compartilhado em `crates/core`. O backend Go foi
removido na Fase 5 (PR #206).

- **Servidor self-hosted**: API HTTP, autenticação e entrega da SPA. O registro de cursos fica em
  `courses.json` ao lado do banco SQLite; `LIBRARY_DIR`/`--library-root` é apenas semente do
  primeiro curso.
- **Desktop offline**: janela `main` em `crates/desktop/tauri.conf.json`, com a SPA embutida por
  `frontendDist`, sem autenticação nem portas de rede. Importação, remoção, rescan e estudo são
  comandos IPC explícitos; o webview recebe identificadores opacos e nunca caminhos.
- **Mídia**: o protocolo `learnvault-media://localhost/<media-id>` entrega imagens, materiais,
  vídeo, áudio e legendas com allowlist de tipo, faixas HTTP e limite de resposta. No Windows
  (WebView2), o webview usa `http://learnvault-media.localhost/<media-id>`.
- **Capas**: um arquivo de até 4 MiB por curso, com formato detectado pelos bytes iniciais,
  gravação atômica e `cover_version` no catálogo. Ficam fora das pastas de curso; no desktop,
  usam o protocolo `learnvault-cover://` e comandos IPC dedicados.
- **Estudo**: posição, conclusão e recentes têm paridade entre servidor, SPA e IPC desktop. O
  player desktop reproduz mídia local, renderiza VTT e permite baixar SRT/ASS/SSA; o build HTTP
  converte SRT sob demanda.
- **Distribuição**: instalador Windows NSIS disponível; pacote Linux/Flatpak pendente de
  validação em sandbox. Veja [distribuição](../operations/distribution.md).

```text
Proxy HTTPS local
        |
        v
Serviço HTTP Rust -- SQLite + registro de cursos (/data)
        |-- cache (/cache)
        `-- pastas de curso (/courses/*:ro)
```

O núcleo também possui uma API de importação explícita do formato legado de SQLite (`legacy_import`):
a origem é somente leitura, recebe backup antes da operação e a saída é promovida somente após transação,
integridade, chaves estrangeiras e contagens. A biblioteca física não participa da migração.

## Arquitetura aprovada

O alvo é um workspace Rust com um núcleo de domínio compartilhado. A decisão e as alternativas
consideradas estão no [ADR 0001](adr/0001-reescrita-em-rust.md).

```text
Servidor self-hosted                    Desktop offline
  navegador / WebUI                       webview embutido
        |                                       |
        v                                       v
  crates/server (Axum)                 crates/desktop (Tauri 2)
        |  HTTP, cookie, Range                |  IPC, protocolo de mídia
        +-------------------+-----------------+
                            v
                    crates/core (domínio, SQLite)
                            |
        +-------------------+-------------------+
        v                                       v
  biblioteca de cursos (:ro)              dados e cache locais
```

Responsabilidades:

- `crates/core` contém as regras de domínio, o catálogo, o scanner, a identidade de arquivos, a
  política de rescan, a persistência SQLite, o estado de estudo, a validação compartilhada da raiz
  de cada curso e a identidade estável derivada do caminho. Não conhece HTTP nem Tauri e recebe
  cada raiz de curso já resolvida.
- `crates/server` traduz o domínio para HTTP: autenticação, sessões, `Range`, cabeçalhos de
  segurança, registro de cursos e entrega da SPA.
- `crates/desktop` integra com o sistema de arquivos e entrega mídia ao webview, sem autenticação e sem portas de rede. Expõe IPC para cursos, catálogo, capas, rescan e estudo; protocolos locais entregam mídia por identificadores opacos e faixas limitadas. O pacote Linux/Flatpak continua pendente de validação em sandbox.
- `apps/web/` é uma única SPA em Svelte 5 + TypeScript + Vite, com roteamento por hash e dois
  builds que escolhem o transporte em tempo de compilação.

Invariantes:

- A biblioteca física de cursos é externa e somente leitura; nada a modifica, e cada pasta é
  registrada individualmente como um curso.
- O SQLite guarda metadados e estado pessoal de estudo, nunca mídia de cursos.
- O rescan nunca exclui registros automaticamente: arquivos ausentes são marcados como ausentes
  e podem reaparecer no mesmo caminho reutilizando a mesma identidade de mídia.
- A remoção de uma mídia é sempre explícita e persistente: ela sai do catálogo sem apagar
  registros nem progresso, e nenhuma varredura a desfaz.
- A pasta de cada curso é configuração da plataforma, não dado do catálogo: o registro fica ao
  lado do banco (servidor) ou no diretório de configuração do aplicativo (desktop).
- Ações administrativas são explícitas; não há scan silencioso na inicialização nem importação
  que indexe sem pedido.
- Cache e mídia derivada são descartáveis e ficam em locais separados dos dados duráveis.
- Uma capa personalizada é dado do aplicativo, não do curso: fica ao lado do banco (servidor) ou
  no diretório de dados do aplicativo (desktop), com no máximo 4 MiB por curso, formato decidido
  pelos bytes iniciais e gravação atômica. Nenhuma pasta de curso recebe arquivo.
- Docker e Podman são adaptadores opcionais de deploy, não dependências do runtime.

Consulte [distribuição](../operations/distribution.md) para executáveis e pacotes.
