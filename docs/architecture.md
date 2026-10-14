# Architecture

LearnVault is a single self-hosted service:

```text
Caddy (local HTTPS)
       |
       v
Go HTTP service -- SQLite (/data)
       |-- cache (/cache)
       `-- course library (/library:ro)
```

The SvelteKit application is built as a static SPA and embedded into the Go binary. The Go service owns authentication, authorization, catalog persistence, and authenticated media delivery.

The physical library is external and read-only. SQLite contains metadata and personal study state, not course media. Cache and derived media are disposable. Admin actions are explicit; the application does not silently rescan at startup.

The primary homelab target is rootless Podman with Quadlet. The same OCI image should work with Docker Compose. Durable application data and disposable cache use separate mounts.
