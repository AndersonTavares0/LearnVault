# Flatpak desktop plan (Linux)

**Status:** implemented in CI, shipped as a release asset with the next release. The bundle is distributed only through GitHub Releases; there is no store submission (no Flathub). The desktop updater is off inside the Flatpak.

## Scope across systems

| System | Desktop app | Plan |
| :--- | :--- | :--- |
| Windows | NSIS installer with signed updater | Available. No change. |
| Linux (x86_64, aarch64) | Flatpak bundle attached to each GitHub Release | Implemented in `.github/workflows/flatpak.yml`. |
| macOS | none | Not planned. No build job exists and the Tauri config targets `nsis` only. |
| Any OS (server) | Linux tarball and OCI image | Available. Flatpak does not replace them. |

The Flatpak is the desktop app only. The server keeps its tarball and container image.

## What exists

- `deploy/packages/app.learnvault.desktop.yml`: manifest on `org.gnome.Platform` 48, installs a prebuilt `target/release/learnvault-desktop`, the `.desktop` file, the metainfo and the icon. Grants IPC, Wayland, X11 fallback and DRI only, and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`: without it the app exits immediately with `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display` on NVIDIA + Wayland sessions (reproduced on an RTX 3060 Ti with the GNOME 51 runtime).
- `crates/desktop/tests/foundation.rs`: fails if the manifest grants broad `home` or `host` filesystem access.
- `crates/desktop/tauri.conf.json`: `bundle.targets` is `["nsis"]`, so no Linux bundle is produced.
- The folder picker uses `tauri-plugin-dialog`. Under Flatpak it must go through `xdg-desktop-portal` (FileChooser).
- `crates/desktop/src/command.rs` opens external URLs with `xdg-open`. Inside the sandbox that must use the OpenURI portal.

## Gaps to close

1. **Linux desktop build in CI.** A job on `ubuntu-latest` (and `ubuntu-24.04-arm`) that installs the WebKitGTK and GTK development packages, runs `pnpm run build:desktop`, and builds `learnvault-desktop` in release mode.
2. **Flatpak build in CI.** `flatpak-builder` on the GNOME 48 runtime, using `deploy/packages/app.learnvault.desktop.yml`, for both architectures.
3. **Audio and media.** Add `--socket=pulseaudio` (PipeWire goes through it). Lesson playback needs it; the current manifest does not grant it.
4. **Portals instead of host calls.** FileChooser for importing a course folder. OpenURI instead of `xdg-open`. No direct filesystem grant.
5. **Updater off in Flatpak.** The in-app updater would replace the binary, which a Flatpak must not do. Updates arrive as a new bundle from the next release. The updater is gated on `FLATPAK_ID` (done in `crates/desktop/src/command.rs`).
6. **Metainfo.** Validated with `appstreamcli` in CI. No store screenshots or age rating are required, since nothing is submitted to a store.
7. **Smoke test in a sandbox.** Install the built bundle with `flatpak install --user`, start it, import a synthetic course folder through the portal, play a lesson, close and reopen. This is a CI job, not a manual step, and it runs on both architectures.
8. **Release asset.** Add `learnvault-linux-desktop-<arch>.flatpak` and its checksum to `SHA256SUMS`, and update the asset table in `docs/releases/process.md`.

## Decisions (resolved)

- **Channel:** GitHub Release asset only. No Flathub or other store.
- **Runtime:** `org.gnome.Platform` branch 51, the current release. GNOME 48 reached end of life in March 2026. The CI job checks the branch exists on Flathub before building.
- **Release gate:** the bundle is published by the same `publish` job as the other assets, so it waits for the `release` environment approval.

## Open decisions

None blocking. Whether to add a Flathub submission later is a separate decision and would need its own review.

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
4. Updater disabled in Flatpak, metainfo validation (both done).
5. Release asset wiring, then the docs switch from planned to available.
