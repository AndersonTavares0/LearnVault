# Arquitetura

O LearnVault transforma pastas locais de cursos em uma plataforma pessoal de estudos, usada de
duas formas:

- **Servidor self-hosted**, com WebUI acessada pela rede.
- **Desktop offline**, executado localmente sem expor portas de rede.

## Estado atual

O repositório ainda contém a implementação em **Go com SQLite e SPA SvelteKit embutida**. Ela é
a implementação atual, a **referência funcional e visual** da reescrita, e será removida em pull
request próprio quando o MVP em Rust cobrir os requisitos aprovados.

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
- `crates/desktop` integra com o sistema de arquivos e entrega a mídia ao webview, sem
  autenticação e sem portas de rede.
- `web/` é uma única SPA em Svelte 5 + TypeScript + Vite, com roteamento por hash e dois builds
  que escolhem o transporte em tempo de compilação.

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
