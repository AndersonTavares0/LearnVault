# Distribution and release targets

LearnVault is a portable Go application with an embedded SvelteKit SPA. Docker is one
deployment adapter for homelabs and servers; it is not a runtime requirement or the
primary development model.

## Release matrix

| Target | Artifact | Purpose | Status |
| --- | --- | --- | --- |
| Linux | native binary | Server, terminal, and lightweight installations | Foundation build |
| Windows | `.exe` | Direct local execution | Planned |
| Windows | `.msi` | Managed installation and upgrades | Planned |
| Debian/Ubuntu | `.deb` | Native package management | Planned |
| Fedora/RHEL | `.rpm` | Native package management | Planned |
| Linux desktop | Flatpak | Sandboxed desktop distribution | Planned |
| OCI | Docker/Podman image | Optional homelab/server deployment | Available |

## Build boundaries

- The Go command in `cmd/learnvault` is the source of truth for executable builds.
- The SPA is built once and embedded into that executable.
- Package recipes belong under `deploy/packages/` when implemented.
- Container definitions belong under `deploy/container/`, `deploy/compose/`, and
  `deploy/quadlet/`; they must not define application behavior that the binary lacks.
- CI should build a platform artifact independently before assembling installers.
- Release Please owns the changelog and GitHub Release metadata. The published-release
  workflow adds versioned binaries and a `SHA256SUMS` file to that existing release.

## Planned tooling

The packaging tool will be selected after the first native binary milestone. It must
support reproducible versioned builds without making Docker a hidden prerequisite.
Candidate layers include Go cross-compilation, a Windows installer builder, Debian/RPM
recipes, and Flatpak metadata. The final choice will be recorded in an ADR under
`docs/architecture/adr/`.

## Configuration model

Native installations use environment variables or an adjacent configuration file for
`DATA_DIR`, `CACHE_DIR`, `LIBRARY_DIR`, `LISTEN_ADDR`, `PORT`, and authentication bootstrap.
The physical course library remains external and read-only on every platform. Installers
must never copy the library into an application directory or backup.
