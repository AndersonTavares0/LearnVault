# Getting started

LearnVault requires Go 1.25+ and Node.js 24+ for native development. Docker and Podman
are optional deployment adapters.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
go test ./...
go run ./cmd/learnvault
```

The app requires `ADMIN_USERNAME` and `ADMIN_PASSWORD` only for its first startup. Use
the synthetic fixture in `testdata/` for development and tests. Never point automated
work at `/mnt/cursos`; it is external, user-owned, and read-only.

See the repository README for configuration and optional Compose/Quadlet deployment.
