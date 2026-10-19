# Primeiros passos

O LearnVault exige Rust 1.99 (definido em `rust-toolchain.toml`), Node.js 24+ e pnpm 11.28.5
para o desenvolvimento. Docker e Podman são adaptadores opcionais de deploy.

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
```

`pnpm dev` sobe a API em `http://127.0.0.1:8090` e a SPA em `http://localhost:5173`, com
proxy de `/api` para o servidor. A biblioteca começa vazia: entre com `admin`/`admin`, clique em
"Importar curso" e informe o caminho de uma pasta de curso no servidor. Antes do pull request,
`pnpm verify` roda os checks de PR (use `--fast` para pular builds e audit durante a iteração). O
desktop Tauri usa `pnpm dev:tauri`, sem portas de rede, e importa cursos pelo seletor nativo.

Na ausência de configuração, o primeiro acesso usa literalmente `admin`/`admin`. Essa é uma
credencial temporária, não um segredo permanente, e deve ser trocada imediatamente pela tela de
primeiro acesso. Para o bootstrap, o servidor Rust aceita `ADMIN_USERNAME`/`ADMIN_PASSWORD` ou
`ADMIN_USERNAME_FILE`/`ADMIN_PASSWORD_FILE`; configure apenas uma origem para cada credencial. Os
arquivos são lidos somente na inicialização e não substituem o administrador de um banco existente.
Proteja-os com permissões do sistema operacional; o servidor não valida o modo do arquivo. Nunca
publique valores reais em documentação, compose ou logs. Use a fixture sintética em `testdata/`. Nunca aponte a
automação para dados externos; a biblioteca deve permanecer somente leitura.

O aplicativo desktop Tauri (`crates/desktop`) é um cliente offline com janela real, IPC de cursos,
catálogo, rescan e estudo, além de reprodução local por protocolo de mídia. Ele não tem autenticação
nem abre portas de rede. `pnpm dev:tauri` inicia o app para desenvolvimento; não valida pacotes de
release. O instalador Windows NSIS está disponível. Flatpak e distribuição Linux desktop continuam
pendentes até validar o app em sandbox, incluindo acesso persistente à biblioteca e reprodução.

Consulte o README do repositório para configuração e deploy opcional com Compose/Quadlet.
