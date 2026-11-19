# LearnVault {{version}}{{prerelease}}

{{summary}}

## Highlights

{{highlights}}

## Changes

{{changes}}

{{migrations}}

## Assets

- `learnvault-linux-amd64.tar.gz`, `learnvault-linux-arm64.tar.gz`: Linux server bundles.
- `learnvault-linux-desktop-amd64.flatpak`, `learnvault-linux-desktop-arm64.flatpak`: Linux desktop (Flatpak).
- `learnvault-windows-{{tag}}.exe`, `learnvault-windows-{{tag}}.nsis.zip` (+ `.sig`): Windows desktop installer and updater payload.
- `latest.json`: desktop updater manifest.
- `learnvault-{{tag}}.sbom.cdx.json`: CycloneDX source/lockfile software bill of materials (`trivy fs .`).
- `SHA256SUMS`, `SHA256SUMS.sigstore.json`: checksums and keyless Sigstore bundle for the assets above.

Container image: `ghcr.io/andersontavares0/learnvault:{{version}}`. Verify it with cosign as described in `docs/releases/process.md`.

Full commit history: {{compare_url}}
