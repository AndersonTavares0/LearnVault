# Flatpak desktop plan (Linux)

**Status:** planned. Nothing in this document is shipped yet. The manifest in `deploy/packages/` exists, but no CI job builds the Linux desktop binary, runs `flatpak-builder`, or publishes a bundle.

## Scope across systems

| System | Desktop app | Plan |
| :--- | :--- | :--- |
| Windows | NSIS installer with signed updater | Available. No change. |
| Linux (x86_64, aarch64) | Flatpak from Flathub or a release bundle | This plan. |
| macOS | none | Not planned. No build job exists and the Tauri config targets `nsis` only. |
| Any OS (server) | Linux tarball and OCI image | Available. Flatpak does not replace them. |

The Flatpak is the desktop app only. The server keeps its tarball and container image.

## What exists

- `deploy/packages/app.learnvault.desktop.yml`: manifest on `org.gnome.Platform` 48, installs a prebuilt `target/release/learnvault-desktop`, the `.desktop` file, the metainfo and the icon. Grants IPC, Wayland, X11 fallback and DRI only.
- `crates/desktop/tests/foundation.rs`: fails if the manifest grants broad `home` or `host` filesystem access.
- `crates/desktop/tauri.conf.json`: `bundle.targets` is `["nsis"]`, so no Linux bundle is produced.
- The folder picker uses `tauri-plugin-dialog`. Under Flatpak it must go through `xdg-desktop-portal` (FileChooser).
- `crates/desktop/src/command.rs` opens external URLs with `xdg-open`. Inside the sandbox that must use the OpenURI portal.

## Gaps to close

1. **Linux desktop build in CI.** A job on `ubuntu-latest` (and `ubuntu-24.04-arm`) that installs the WebKitGTK and GTK development packages, runs `pnpm run build:desktop`, and builds `learnvault-desktop` in release mode.
2. **Flatpak build in CI.** `flatpak-builder` on the GNOME 48 runtime, using `deploy/packages/app.learnvault.desktop.yml`, for both architectures.
3. **Audio and media.** Add `--socket=pulseaudio` (PipeWire goes through it). Lesson playback needs it; the current manifest does not grant it.
4. **Portals instead of host calls.** FileChooser for importing a course folder. OpenURI instead of `xdg-open`. No direct filesystem grant.
5. **Updater off in Flatpak.** The in-app updater reads `latest.json` and replaces the binary. Flathub owns updates for a Flatpak, so the updater must be disabled when running inside one (detect `FLATPAK_ID`).
6. **Metainfo for Flathub.** Screenshots, release notes per version, OARS content rating, licence, and developer information. Validate with `appstream-util validate-relax`.
7. **Smoke test in a sandbox.** Install the built bundle with `flatpak install --user`, start it, import a synthetic course folder through the portal, play a lesson, close and reopen. This is a CI job, not a manual step, and it runs on both architectures.
8. **Release asset.** Add `learnvault-linux-desktop-<arch>.flatpak` and its checksum to `SHA256SUMS`, and update the asset table in `docs/releases/process.md`.

## Decisions for the maintainer

- **Distribution channel:** a bundle attached to GitHub Releases only, or also a Flathub submission. Flathub implies review and a stable app id, and the id `app.learnvault.desktop` is already fixed in the manifest.
- **Which GNOME runtime** to keep current. Runtime 48 needs a bump on each GNOME release.
- **Whether a Flatpak release is gated by the same `release` environment** as the container and assets. The plan assumes yes.

## Acceptance

- The bundle installs and starts on both architectures in the CI sandbox job.
- The folder picker works through the portal; no path is sent from the webview.
- Lesson playback has sound.
- The updater does not run inside the Flatpak.
- `foundation.rs` and the `SHA256SUMS` check pass with the new asset.
- `docs/operations/distribution.md` and the README table list the Flatpak as available.

## Order of work

1. Linux desktop build job (no packaging yet). Proves the binary builds on both architectures.
2. Portals (FileChooser, OpenURI) and the pulseaudio socket, tested with a local Flatpak build.
3. `flatpak-builder` job and the sandbox smoke test.
4. Updater disabled in Flatpak, metainfo validation.
5. Release asset wiring, then the docs switch from planned to available.
