# Arquitetura

**Para quem é:** engenheiros, revisores e mantenedores que precisam decidir onde uma mudança entra.
Se você só quer usar o LearnVault, comece por
[Getting-Started](../wiki/getting-started.md).

**Resumo:** o LearnVault lê pastas de cursos que já existem, monta um catálogo em SQLite e serve
uma interface web de página única (SPA). O servidor HTTP e o desktop offline compartilham o mesmo
núcleo de domínio. Nenhum dos dois altera os arquivos da biblioteca.

## Dois modos, um núcleo

O produto atende dois cenários:

- **Servidor self-hosted:** uma pessoa administra a biblioteca e as contas; as demais acessam pela
  rede com login.
- **Desktop offline:** uma pessoa usa o aplicativo na própria máquina, sem login e sem abrir portas
  de rede.

A divisão existe para não duplicar as regras de catálogo e estudo. `crates/core` concentra essas
regras; `crates/server` e `crates/desktop` apenas conectam o núcleo ao HTTP ou ao IPC do Tauri.
Assim, a mesma lógica roda nos dois modos e os testes de domínio não precisam de rede nem de
webview.

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

## O que cada parte faz

| Parte | Responsabilidade | Fronteira |
| --- | --- | --- |
| `crates/core` | Domínio, catálogo, scanner, identidade de mídia, rescan, SQLite e estado de estudo | Não conhece HTTP nem Tauri; recebe cada raiz de curso já resolvida |
| `crates/server` | Autenticação, sessões, `Range`, cabeçalhos de segurança, registro de cursos e entrega da SPA | Traduz o domínio para HTTP; não reimplementa regras |
| `crates/desktop` | Janela Tauri, IPC de cursos, catálogo, capas, rescan e estudo, além de mídia local | Não tem autenticação nem portas de rede |
| `apps/web` | SPA Svelte 5 + TypeScript + Vite, com roteamento por hash e dois builds | Escolhe o transporte em tempo de compilação; não acessa arquivos diretamente |

## Estado atual

O workspace Rust está completo: `crates/server` (Axum), `crates/desktop` (Tauri 2) e `apps/web`
(Svelte 5 + Vite), com o domínio compartilhado em `crates/core`. O backend Go foi removido na Fase
5 (PR #206).

- **Servidor self-hosted:** expõe a API HTTP, autentica usuários e entrega a SPA. O registro de
  cursos fica em `courses.json` ao lado do banco SQLite; `LIBRARY_DIR`/`--library-root` serve
  apenas como semente do primeiro curso.
- **Desktop offline:** abre a janela `main` definida em `crates/desktop/tauri.conf.json`, com a SPA
  embutida por `frontendDist`. Importação, remoção, rescan e estudo passam por comandos IPC
  explícitos; o webview recebe identificadores opacos e nunca caminhos.
- **Mídia:** o protocolo `learnvault-media://localhost/<media-id>` entrega imagens, materiais,
  vídeo, áudio e legendas com allowlist de tipo, faixas HTTP e limite de resposta. No Windows
  (WebView2), o webview usa `http://learnvault-media.localhost/<media-id>`.
- **Capas:** cada curso pode ter um arquivo de até 4 MiB, com formato detectado pelos bytes
  iniciais, gravação atômica e `cover_version` no catálogo. A capa fica fora da pasta do curso; no
  desktop, usa o protocolo `learnvault-cover://` e comandos IPC dedicados.
- **Estudo:** posição, conclusão e recentes funcionam do mesmo jeito no servidor, na SPA e no IPC
  desktop. O player desktop reproduz mídia local, renderiza VTT e permite baixar SRT/ASS/SSA; o
  build HTTP converte SRT sob demanda.
- **Distribuição:** o instalador Windows NSIS está disponível; o pacote Linux/Flatpak ainda depende
  de validação em sandbox. Veja [distribuição](../operations/distribution.md).

```text
Proxy HTTPS local
        |
        v
Serviço HTTP Rust -- SQLite + registro de cursos (/data)
        |-- cache (/cache)
        `-- pastas de curso (/courses/*:ro)
```

## Regras que qualquer mudança deve respeitar

Estas regras, ou invariantes, explicam decisões que já custaram discussão. Se uma mudança precisa
quebrar uma delas, a conversa começa no GitHub, não no código.

- A biblioteca física é externa e somente leitura. Cada pasta registrada vira um curso.
- O SQLite guarda metadados e estado pessoal de estudo. A mídia dos cursos fica na biblioteca.
- O rescan nunca exclui registros. Ele marca arquivos ausentes e reutiliza a identidade quando o
  arquivo reaparece no mesmo caminho.
- Remover uma mídia é uma ação explícita e persistente. O registro e o progresso continuam, e
  nenhum rescan desfaz a remoção.
- A pasta de cada curso é configuração da plataforma, não dado do catálogo. O registro fica ao lado
  do banco no servidor ou no diretório de configuração do aplicativo no desktop.
- Ações administrativas são explícitas. Não existe scan silencioso na inicialização nem importação
  que indexe sem pedido.
- Cache e mídia derivada são descartáveis e ficam separados dos dados duráveis.
- Uma capa personalizada pertence ao aplicativo, não ao curso. Ela fica ao lado do banco no
  servidor ou no diretório de dados do aplicativo no desktop, com no máximo 4 MiB, formato
  decidido pelos bytes iniciais e gravação atômica. Nenhuma pasta de curso recebe arquivo.
- Docker e Podman são adaptadores opcionais de deploy. O runtime não depende deles.

## Importação de bancos antigos

O núcleo mantém `legacy_import` para quem vem da versão Go. A importação é explícita: a origem é
somente leitura, recebe backup antes da operação e a saída só é promovida depois de transação,
integridade, chaves estrangeiras e contagens. A biblioteca física não participa da migração.

## Decisões e alternativas

A decisão pela reescrita em Rust e as alternativas consideradas estão no
[ADR 0001](adr/0001-reescrita-em-rust.md). Consulte [distribuição](../operations/distribution.md)
para executáveis e pacotes.
