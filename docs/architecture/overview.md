# Arquitetura

O LearnVault transforma pastas locais de cursos em uma plataforma pessoal de estudos, usada de
duas formas:

- **Servidor self-hosted**, com WebUI acessada pela rede.
- **Desktop offline**, executado localmente sem expor portas de rede.

## Estado atual

O repositório ainda contém a implementação em **Go com SQLite e SPA SvelteKit embutida**. Ela é
a implementação atual, a **referência funcional e visual** da reescrita, e será removida em pull
request próprio quando o MVP em Rust cobrir os requisitos aprovados.

A reescrita em Rust já entrega `crates/core`, `crates/server` e o `crates/desktop` com a
**fundação** (crate Tauri 2 que embute a SPA por `frontendDist`, capability em deny-by-default e
sem portas de rede) e as **fatias de IPC**: os comandos `desktop_config` e `desktop_catalog`, com
ACL app-defined mínima e o catálogo derivado do `learnvault-core` sobre um banco SQLite local, no
diretório de dados do aplicativo; o comando `desktop_select_library`, que aciona o seletor nativo
interno ao Rust e persiste a raiz canonicalizada de forma versionada, atômica e serializada no
diretório de configuração do aplicativo, fora da biblioteca e do SQLite; o comando
`desktop_rescan`, que indexa explicitamente a raiz persistida reutilizando o `learnvault-core`, com
estado ocupado que serializa chamadas concorrentes e nunca dispara varredura automática; e o
**protocolo de mídia de arquivos pequenos**, que entrega somente imagens/thumbnails e materiais
pequenos por `learnvault-media://localhost/<media-id>`, com o webview apontando pelo `media_id`
opaco (nunca por um caminho), allowlist de tipo de conteúdo, limite explícito de resposta e
revalidação pelo `learnvault-core`. **Vídeo e áudio ficam fora deste protocolo** nesta fatia por
causa da reprodução parcial incompatível no webview do Linux (`WebKitGTK`); legendas continuam no
caminho HTTP. Autenticação e janelas reais ainda não existem no desktop.

```text
Proxy HTTPS local
        |
        v
Serviço HTTP Go -- SQLite (/data)
        |-- cache (/cache)
        `-- biblioteca de cursos (/library:ro)
```

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
  política de rescan, a persistência SQLite e o estado de estudo. Não conhece HTTP nem Tauri e
  recebe a raiz da biblioteca já resolvida.
- `crates/server` traduz o domínio para HTTP: autenticação, sessões, `Range`, cabeçalhos de
  segurança e entrega da SPA.
- `crates/desktop` integra com o sistema de arquivos e entrega mídia ao webview, sem autenticação e sem portas de rede. Expõe o IPC de configuração e catálogo sobre o banco local, a seleção da raiz da biblioteca (seletor nativo interno ao Rust, com persistência atômica fora da biblioteca), o rescan explícito dessa raiz, reutilizando o núcleo com estado ocupado e sem varredura automática, e um protocolo customizado que entrega somente **arquivos pequenos** (imagens/thumbnails e materiais pequenos) pelo `media_id` opaco. Vídeo/áudio e autenticação pertencem a fatias posteriores: a reprodução parcial de vídeo e áudio é incompatível no webview do Linux (`WebKitGTK`) e permanece pendente.
- `apps/web/` é uma única SPA em Svelte 5 + TypeScript + Vite, com roteamento por hash e dois
  builds que escolhem o transporte em tempo de compilação.

Invariantes:

- A biblioteca física de cursos é externa e somente leitura; nada a modifica.
- O SQLite guarda metadados e estado pessoal de estudo, nunca mídia de cursos.
- O rescan nunca exclui registros automaticamente: arquivos ausentes são marcados como ausentes
  e podem reaparecer no mesmo caminho reutilizando a mesma identidade de mídia.
- A remoção de uma mídia é sempre explícita e persistente: ela sai do catálogo sem apagar
  registros nem progresso, e nenhuma varredura a desfaz.
- A raiz da biblioteca é configuração da plataforma, não dado do catálogo.
- Ações administrativas são explícitas; não há scan silencioso na inicialização.
- Cache e mídia derivada são descartáveis e ficam em locais separados dos dados duráveis.
- Docker e Podman são adaptadores opcionais de deploy, não dependências do runtime.

Consulte [distribuição](../operations/distribution.md) para executáveis e pacotes.
