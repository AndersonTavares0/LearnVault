# Primeiros passos

O LearnVault exige Go 1.26+ e Node.js 24+ para desenvolvimento nativo. Docker e Podman
são adaptadores opcionais de deploy.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
go test ./...
go -C services/go run ./cmd/learnvault
```

A aplicação exige `ADMIN_USERNAME` e `ADMIN_PASSWORD` somente na primeira inicialização.
Use a fixture sintética em `testdata/` para desenvolvimento e testes. Nunca aponte a
automação para dados externos; a biblioteca deve permanecer somente leitura.

Consulte o README do repositório para configuração e deploy opcional com Compose/Quadlet.
