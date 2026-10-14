# Architecture

LearnVault is a single portable service. It can run as a native executable or through
an optional container adapter:

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

Rootless Podman with Quadlet and Docker Compose are deployment adapters for the homelab,
not application dependencies. Native desktop and server builds remain first-class.
Durable application data and disposable cache use separate locations in every adapter.

See [distribution.md](distribution.md) for executable and package targets.
