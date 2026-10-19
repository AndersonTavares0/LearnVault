# Primeiros passos

Esta página é para quem vai rodar o LearnVault pela primeira vez, seja para usar ou para
desenvolver. O caminho mais curto é instalar as ferramentas, rodar `pnpm dev` e entrar com a
credencial temporária. Se algo falhar, comece pela tabela de problemas comuns no fim da página.

Docker e Podman são opcionais. Você não precisa deles para desenvolver nem para usar o desktop.

## Ferramentas de desenvolvimento

| Ferramenta | Versão esperada | Fonte da verdade | Como conferir |
| --- | --- | --- | --- |
| Rust | linha 1.99 | `rust-toolchain.toml` (fixado; `rustup` resolve) | `rustc --version` |
| Node.js | linha 24.x | requisitos do projeto (SPA) | `node --version` |
| pnpm | 11.28.5 | `packageManager` em `package.json` | `pnpm --version` |

Use as linhas acima em vez de fixar uma versão de patch:

- **Rust**: instale e atualize com o [rustup](https://rustup.rs). O `rust-toolchain.toml` fixa a
  versão; qualquer `cargo` executado no checkout usa essa versão.
- **Node.js 24.x**: use o instalador oficial do [Node.js](https://nodejs.org/) ou um gerenciador de
  versões (nvm ou fnm no Linux/macOS; nvm-windows ou o instalador oficial no Windows). Fique na
  linha major 24.
- **pnpm**: o `corepack enable pnpm` instala a versão declarada em `packageManager`. Se o Corepack
  não estiver disponível na sua instalação, siga o [instalador standalone do
  pnpm](https://pnpm.io/installation).

## Rodar em desenvolvimento

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
```

`pnpm install --frozen-lockfile` instala exatamente o que está no lockfile. Se o comando pedir para
atualizar algo, a versão do pnpm ou o lockfile estão diferentes do esperado.

`pnpm dev` sobe a API Rust em `http://127.0.0.1:8090` e a interface web (SPA) em
`http://localhost:5173`, com proxy de `/api` para o servidor. Abra a SPA e faça login. Na primeira
execução, a biblioteca começa vazia: entre com `admin`/`admin`, clique em "Importar curso" e
escolha a pasta de um curso. O LearnVault lê a pasta e não altera os arquivos.

Antes de abrir um pull request, rode `pnpm verify`. Durante a iteração, `pnpm verify --fast` pula
builds e audit e mantém os checks rápidos.

## Primeiro acesso em detalhe

Desenvolvimento local no PowerShell:

```powershell
pnpm install --frozen-lockfile
pnpm dev
# abra http://localhost:5173 e entre com a credencial temporária
```

POSIX (bash/zsh), mudando a porta da API quando necessário:

```sh
pnpm install --frozen-lockfile
LEARNVAULT_LISTEN_ADDR=127.0.0.1:8090 pnpm dev
```

## Credenciais de bootstrap

Sem configuração, o primeiro acesso usa `admin`/`admin`. Troque a senha na tela de primeiro acesso;
essa credencial serve só para o bootstrap.

Para definir outra credencial, use `ADMIN_USERNAME`/`ADMIN_PASSWORD` ou
`ADMIN_USERNAME_FILE`/`ADMIN_PASSWORD_FILE`. Escolha uma origem por credencial: definir as duas é
erro de configuração. Prefira os arquivos `*_FILE`, porque variáveis de ambiente aparecem na lista
de processos. O servidor lê os arquivos na inicialização e não altera um administrador que já
existe. Ajuste as permissões do arquivo no sistema operacional; o servidor não verifica o modo.

Nunca coloque valores reais em documentação, Compose ou logs. Testes usam a fixture sintética de
`testdata/`, e a biblioteca de cursos continua somente leitura.

## Caminho rápido com Compose

Para subir um servidor com Docker ou Podman, copie `deploy/compose/.env.example`, ajuste
`ADMIN_PASSWORD` para uma senha forte e siga o README. O Compose é um adaptador de deploy, não o
único caminho.

## Problemas comuns

| Sintoma | Causa provável | Correção |
| --- | --- | --- |
| `pnpm` avisa incompatibilidade com Node | Node fora da linha 24.x | troque com nvm/fnm/nvm-windows ou o instalador oficial e rode `node --version` |
| `pnpm: command not found` | pnpm não instalado ou Corepack desabilitado | `corepack enable pnpm` ou instalador standalone do pnpm |
| Porta 8090 ocupada | outro processo usa a porta | rode o dev com `LEARNVAULT_LISTEN_ADDR` em outra porta; o proxy da SPA acompanha |
| `invalid library root` | `LIBRARY_DIR` aponta para um caminho inexistente | crie o diretório ou remova a variável e registre cursos pela interface |
| Bootstrap falhou | origem dupla ou arquivo ilegível de `ADMIN_*` | declare apenas uma origem por credencial e verifique as permissões do arquivo |

## Aplicativo desktop

O desktop Tauri (`crates/desktop`) é um cliente offline com janela real, comandos internos (IPC)
para cursos, catálogo, releitura da biblioteca e estudo, além de reprodução local de mídia. Ele não
tem autenticação nem abre portas de rede. `pnpm dev:tauri` inicia o app para desenvolvimento; esse
comando não valida pacotes de release. O instalador Windows NSIS está disponível. Flatpak e distribuição Linux desktop ainda
dependem de validação em sandbox, incluindo acesso persistente à biblioteca e reprodução.
