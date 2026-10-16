# Primeiros passos

O LearnVault exige Rust 1.99 (definido em `rust-toolchain.toml`), Node.js 24+ e pnpm 11.28.5
para o desenvolvimento atual. Go 1.26+ é necessário apenas para a implementação de referência
em `services/go`. Docker e Podman são adaptadores opcionais de deploy.

```sh
pnpm install --frozen-lockfile   # uma vez por checkout
pnpm dev                         # API Rust + SPA com recarga automática
```

`pnpm dev` sobe a API em `http://127.0.0.1:8090` e a SPA em `http://localhost:5173`, com
proxy de `/api` para o servidor. Antes do pull request, `pnpm verify` roda os checks de PR
(use `--fast` para pular builds e audit durante a iteração). O desktop Tauri usa
`pnpm dev:tauri`, sem portas de rede.

A implementação de referência em Go continua disponível:

```sh
go -C services/go test ./...
go -C services/go run ./cmd/learnvault
```

Na ausência de configuração, o primeiro acesso usa literalmente `admin`/`admin`. Essa é uma
credencial temporária, não um segredo permanente, e deve ser trocada imediatamente pela tela de
primeiro acesso. Quando configuradas, as credenciais fornecidas têm precedência: o servidor Rust
usa `ADMIN_USERNAME`/`ADMIN_PASSWORD`; a referência Go usa `ADMIN_USER`/`ADMIN_PASS` (os nomes
legados `ADMIN_USERNAME`/`ADMIN_PASSWORD` são aceitos apenas como aliases de configuração).
Use secret file ou mecanismo equivalente do ambiente para valores configurados; nunca publique
valores reais em documentação, compose ou logs. Use a fixture sintética em `testdata/`. Nunca aponte a
automação para dados externos; a biblioteca deve permanecer somente leitura.

O aplicativo desktop Tauri (`crates/desktop`, Fase 4) e a distribuição por plataforma (Fase 5)
ainda não estão concluídos; o atalho `pnpm dev:tauri` cobre apenas a fatia atual do desktop
(configuração da biblioteca, catálogo e rescan). A entrega parcial de mídia está em #76 (Draft,
não mesclada); não trate-a como suporte de mídia ou distribuição concluídos.

Consulte o README do repositório para configuração e deploy opcional com Compose/Quadlet.
