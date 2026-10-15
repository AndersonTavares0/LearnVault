# Primeiros passos

O LearnVault exige Go 1.26+ e Node.js 24+ para desenvolvimento nativo. Docker e Podman
são adaptadores opcionais de deploy.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
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

Consulte o README do repositório para configuração e deploy opcional com Compose/Quadlet.
