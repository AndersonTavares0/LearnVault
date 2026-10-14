# LearnVault roadmap

This roadmap is the durable product summary. It is intentionally outcome-oriented;
implementation details belong in linked GitHub Issues and pull requests.

## Current — foundation and catalog

- [x] Go service with embedded SvelteKit SPA
- [x] Local admin bootstrap and secure sessions
- [x] SQLite migrations and catalog persistence
- [x] Explicit read-only library scan
- [x] Authenticated catalog and Range-capable media endpoint
- [x] Reproducible Docker/Podman deployment baseline
- [x] CI, release automation, and repository governance

## Next — usable study loop

- [ ] Course detail and lesson navigation
- [ ] Direct-play player with subtitle/material links
- [ ] Individual progress and resume position
- [ ] Mark lesson complete and recent-history dashboard
- [ ] Admin course-access permissions for multiple local users

## Interface — mesa de estudo

- [ ] Establish persistent desktop sidebar and mobile navigation
- [ ] Prioritize “Continuar estudando” on the home screen
- [ ] Build the player as the primary study canvas with a lesson index
- [ ] Add course covers, meaningful empty states, loading states, and recovery states
- [ ] Validate keyboard navigation, focus visibility, contrast, reduced motion, and screen-reader labels
- [ ] Keep the visual identity original: blue-petroleum, dark ink, cool paper, and lime signal

## Then — richer library experience

- [ ] Generated and manual course covers
- [ ] Remux-on-demand cache for incompatible media
- [ ] Search across catalog, materials, and notes
- [ ] Favorites and timestamped notes
- [ ] Responsive “mesa de estudo” navigation and accessibility polish

## Distribution — portable first

- [ ] Keep the core application buildable without Docker or Podman
- [ ] Publish versioned Windows `.exe` artifacts
- [ ] Package Windows installers as `.msi`
- [ ] Publish Linux `.deb` packages
- [ ] Publish Linux `.rpm` packages
- [ ] Publish a Flatpak bundle and metadata
- [ ] Sign desktop artifacts where platform credentials are available
- [ ] Generate checksums and provenance metadata for every release
- [ ] Keep Docker Compose and rootless Podman/Quadlet as optional server adapters

## Later — optional intelligence and operations

- [ ] On-demand Whisper integration as a separately packaged capability
- [ ] Transcript indexing and searchable captions
- [ ] Backup automation with retention and restore verification
- [ ] Homelab deployment behind Caddy at `cursos.home.arpa`
- [ ] Native mobile client after the web experience is stable

## Delivery rules

- Each milestone must have a clear user outcome, tests, documentation, and rollback notes.
- Distribution targets are additive: container build failures must not block native
  desktop development, and packaging failures must not block server builds.
- Performance-sensitive work must include resource and cache behavior measurements.
- No roadmap item may modify or copy the physical library; `/mnt/cursos` remains external
  and read-only.
- Scope changes are discussed in GitHub Issues and reflected here through a pull request.
