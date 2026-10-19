# Arquitetura

O LearnVault transforma pastas locais de cursos em uma plataforma pessoal de estudos, usada de
duas formas:

- **Servidor self-hosted**, com WebUI acessada pela rede.
- **Desktop offline**, executado localmente sem expor portas de rede.

## Estado atual

O LearnVault é implementado no workspace Rust — `crates/server` para o servidor Axum, `crates/desktop`
para a aplicação Tauri offline e `apps/web` para a SPA Svelte 5 + Vite — com o núcleo compartilhado
em `crates/core`. O backend legado em Go foi integralmente removido na Fase 5, consolidando Rust como
stack backend exclusiva do projeto.

A migração do stack Rust foi concluída. O desktop Tauri 2 embute a SPA por `frontendDist`, define
uma janela `main` em `crates/desktop/tauri.conf.json` e não abre portas de rede. Ele expõe o
catálogo agregado de cursos (`desktop_catalog`), com
ACL app-defined mínima e derivação pelo `learnvault-core` sobre um banco SQLite local, no
diretório de dados do aplicativo; os comandos `desktop_import_course` e `desktop_remove_course`,
que usam o seletor nativo interno ao Rust e persistem o **registro de cursos** (versão 2, com
migração do formato anterior de raiz única) de forma atômica e serializada no diretório de
configuração do aplicativo, fora das pastas de curso e do SQLite — o webview nunca envia nem
recebe um caminho; o comando `desktop_rescan`, que indexa explicitamente todos os cursos
registrados reutilizando o `learnvault-core`, com estado ocupado que serializa chamadas
concorrentes, agregação de falhas por curso e nunca varredura automática; os comandos de estudo
`desktop_study_progress`, `desktop_record_position`, `desktop_set_completed` e
`desktop_recent_study`, que reutilizam `learnvault-core::study` com o dono local fixo resolvido no
Rust; e o **protocolo de mídia**, que entrega imagens/thumbnails, materiais pequenos, vídeo, áudio
e arquivos de legenda por `learnvault-media://localhost/<media-id>` — no Windows (WebView2) o webview pede
`http://learnvault-media.localhost/<media-id>`, que o runtime reverte para esse esquema antes do
handler —, com o webview apontando pelo `media_id` opaco (nunca por um caminho), resolução da
biblioteca pelo próprio identificador, allowlist de tipo de conteúdo, faixas HTTP limitadas,
limite explícito de resposta e revalidação pelo `learnvault-core`. As **capas personalizadas dos
cursos** usam dois caminhos equivalentes: no servidor, as rotas administrativas
`PUT`/`DELETE /api/admin/courses/{id}/cover` e a leitura autenticada `GET /api/courses/{id}/cover`;
no desktop, o protocolo `learnvault-cover://localhost/<course-id>` e os comandos
`desktop_import_course_cover`/`desktop_remove_course_cover`, em que o seletor nativo de imagens roda
no Rust e nenhum byte de arquivo cruza o IPC. A capa é um único arquivo de até 4 MiB por curso,
guardado ao lado do banco no servidor e no diretório de dados do aplicativo no desktop — nunca
dentro da pasta do curso —, com o formato decidido pelos bytes iniciais (PNG, JPEG ou WebP),
gravação atômica e versão em segundos exposta no catálogo como `cover_version`. No servidor self-hosted, o
registro de cursos vive em `courses.json` ao lado do banco, com endpoints administrativos de
importação e remoção, e `LIBRARY_DIR`/`--library-root` continua aceito apenas como semente do
primeiro curso. A API de Study tem paridade entre o servidor Rust e a SPA web,
e o desktop a expõe por IPC; o player desktop reproduz mídia local e sincroniza
posição/conclusão. O player desktop renderiza legendas VTT; SRT, ASS e SSA são baixáveis, e o build
HTTP pode converter SRT sob demanda. O desktop não tem autenticação por decisão de arquitetura.
O instalador Windows NSIS está disponível; o pacote Linux desktop/Flatpak continua pendente até a
validação em sandbox, incluindo acesso persistente à biblioteca e reprodução. Veja
[distribuição](../operations/distribution.md).

```text
Proxy HTTPS local
        |
        v
Serviço HTTP Rust -- SQLite + registro de cursos (/data)
        |-- cache (/cache)
        `-- pastas de curso (/courses/*:ro)
```

O serviço HTTP Go foi removido na Fase 5 (PR #206); o servidor atual é a implementação Rust
descrita pelo fluxo acima.

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
