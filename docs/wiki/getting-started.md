# Primeiros passos

Este guia cobre a instalação das ferramentas, o primeiro acesso ao servidor e os
problemas mais comuns ao rodar o LearnVault pela primeira vez. Docker e Podman são
adaptadores opcionais de deploy, não requisitos de runtime.

## Ferramentas de desenvolvimento

| Ferramenta | Versão esperada | Fonte da verdade | Como conferir |
| --- | --- | --- | --- |
| Rust | linha 1.99 | `rust-toolchain.toml` (fixado; `rustup` resolve) | `rustc --version` |
| Node.js | linha 24.x | requisitos do projeto (SPA) | `node --version` |
| pnpm | 11.28.5 | `packageManager` em `package.json` | `pnpm --version` |

Não fixe versões de patch manualmente: use as linhas acima e atualize pela fonte
correspondente quando a versão estiver fora do alcance:

- **Rust**: instale e atualize com o [rustup](https://rustup.rs). Com
  `rust-toolchain.toml` no repositório, qualquer `cargo` executado no checkout usa a
  versão correta automaticamente após `rustup update`.
- **Node.js 24.x**: use o instalador oficial do [Node.js](https://nodejs.org/) ou um
  gerenciador de versões (nvm ou fnm no Linux/macOS; nvm-windows ou o instalador
  oficial no Windows). Mantenha-se na linha major 24.
- **pnpm**: o Node.js 24 ainda distribui o Corepack; `corepack enable pnpm` instala
  a versão declarada em `packageManager`. Se o Corepack não estiver disponível na
  sua instalação, siga o [instalador standalone do
  pnpm](https://pnpm.io/installation) para o Windows (PowerShell) e para POSIX.

## Executar em desenvolvimento

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
```

Comportamento esperado de cada comando:

- `pnpm install --frozen-lockfile` instala exatamente as dependências do lockfile.
  Sucesso: termina sem pedir para atualizar nada, com pnpm 11.28.5.
- `pnpm dev` sobe a API Rust em `http://127.0.0.1:8090` e a SPA com recarga
  automática em `http://localhost:5173`, com proxy de `/api` para o servidor.
  Sucesso: o terminal mostra as duas URLs e abrir a SPA exibe a tela de login.

Na primeira execução, a biblioteca começa vazia: entre com `admin`/`admin`, clique
em "Importar curso" e informe o caminho de uma pasta de curso no servidor. O LearnVault
nunca altera os arquivos da pasta original.

Antes do pull request, `pnpm verify` roda os checks de PR (use `--fast` para pular
builds e audit durante a iteração).

## Exemplo de primeiro acesso

Desenvolvimento local PowerShell:

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

Na ausência de configuração, o primeiro acesso usa literalmente `admin`/`admin`. Essa
é uma credencial temporária, não um segredo permanente, e deve ser trocada
imediatamente pela tela de primeiro acesso. Para o bootstrap, o servidor Rust aceita
`ADMIN_USERNAME`/`ADMIN_PASSWORD` ou `ADMIN_USERNAME_FILE`/`ADMIN_PASSWORD_FILE`;
configure apenas uma origem para cada credencial. Os arquivos são lidos somente na
inicialização e não substituem o administrador de um banco existente. Proteja-os com
permissões do sistema operacional; o servidor não valida o modo do arquivo. Valores
declarados em variáveis de ambiente aparecem na lista de processos; prefira a variante
`*_FILE`. Nunca publique valores reais em documentação, compose ou logs. Use a fixture
sintética em `testdata/`. Nunca aponte a automação para dados externos; a biblioteca
deve permanecer somente leitura.

## Caminho rápido com Compose

Para um servidor com Docker/Podman, copie `deploy/compose/.env.example`, ajuste
`ADMIN_PASSWORD` para uma senha forte e consulte o README para o deploy opcional com
Compose/Quadlet.

## Solução de problemas

| Sintoma | Causa provável | Correção |
| --- | --- | --- |
| `pnpm` avisa incompatibilidade com Node | Node fora da linha 24.x | troque com nvm/fnm/nvm-windows ou o instalador oficial e rode `node --version` |
| `pnpm: command not found` | pnpm não instalado ou Corepack desabilitado | `corepack enable pnpm` ou instalador standalone do pnpm |
| Porta 8090 ocupada | outro processo usa a porta | rode o dev com `LEARNVAULT_LISTEN_ADDR` em outra porta; o proxy da SPA acompanha |
| `invalid library root` | `LIBRARY_DIR` definida com caminho inexistente | crie o diretório ou remova a variável e registre cursos pela interface |
| Bootstrap falhou | origem dupla ou arquivo ilegível de `ADMIN_*` | declare apenas uma origem por credencial e verifique permissões do arquivo |

## Aplicativo desktop

O desktop Tauri (`crates/desktop`) é um cliente offline com janela real, IPC de
cursos, catálogo, rescan e estudo, além de reprodução local por protocolo de mídia.
Ele não tem autenticação nem abre portas de rede. `pnpm dev:tauri` inicia o app para
desenvolvimento; não valida pacotes de release. O instalador Windows NSIS está
disponível. Flatpak e distribuição Linux desktop continuam pendentes até validar o
app em sandbox, incluindo acesso persistente à biblioteca e reprodução.
