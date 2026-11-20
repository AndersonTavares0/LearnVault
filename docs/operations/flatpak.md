# Flatpak desktop plan (Linux)

**Status:** implemented in CI and shipped as a release asset since v0.9.5. The bundle is distributed only through GitHub Releases; there is no store submission (no Flathub). The desktop updater is off inside the Flatpak.

## Scope across systems

| System | Desktop app | Plan |
| :--- | :--- | :--- |
| Windows | NSIS installer with signed updater | Available. No change. |
| Linux (x86_64, aarch64) | Flatpak bundle attached to each GitHub Release | Implemented in `.github/workflows/flatpak.yml`. |
| macOS | none | Not planned. No build job exists and the Tauri config targets `nsis` only. |
| Any OS (server) | Linux tarball and OCI image | Available. Flatpak does not replace them. |

The Flatpak is the desktop app only. The server keeps its tarball and container image.

## What exists

- `deploy/packages/app.learnvault.desktop.yml`: manifest on `org.gnome.Platform` 51, installs a prebuilt `target/release/learnvault-desktop`, the `.desktop` file, the metainfo and the icon. Grants IPC, Wayland, X11 fallback, PulseAudio and DRI only, and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`: without it the app exits immediately with `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display` on NVIDIA + Wayland sessions (reproduced on an RTX 3060 Ti with the GNOME 51 runtime). The sandbox check in CI asserts that this variable and the audio socket are present.
- `.github/workflows/flatpak.yml`: builds the release binary with the release version injected into the Tauri config (`tauri.release.json`, from `.github/release-please/version.txt`, as the Windows job does; the workspace version is `0.0.0`, which would show as the installed version in Settings > Updates), then builds, installs and permission-checks the bundle, starts it under a virtual display and checks that the FileChooser and OpenURI portals answer.
- `crates/desktop/tests/foundation.rs`: fails if the manifest grants broad `home` or `host` filesystem access.
- `crates/desktop/tauri.conf.json`: `bundle.targets` is `["nsis"]`, so no Tauri Linux bundle is produced (the Flatpak is built from the release binary by `flatpak.yml`).
- The folder picker uses `tauri-plugin-dialog`. Under Flatpak it goes through `xdg-desktop-portal` (FileChooser; the `xdg-portal` feature of the plugin).
- `crates/desktop/src/command.rs` opens the media file in the system player with `xdg-open`. The sandbox grants no filesystem access, so this action is not expected to work inside the Flatpak; whether to route it through the OpenURI portal is open.

## Delivered

1. **Linux desktop build in CI** on `ubuntu-latest` and `ubuntu-24.04-arm` (`flatpak.yml`).
2. **Flatpak build in CI** with `flatpak-builder` on the GNOME 51 runtime, both architectures.
3. **Audio.** `--socket=pulseaudio` (PipeWire goes through it).
4. **Folder picker through the FileChooser portal**, with no direct filesystem grant.
5. **Updater off in Flatpak**, gated on `FLATPAK_ID` (`crates/desktop/src/command.rs`). Updates arrive as a new bundle from the next release.
6. **Metainfo** validated with `appstreamcli` in CI. No store screenshots or age rating are required, since nothing is submitted to a store.
7. **Sandbox checks in CI**, both architectures: install the bundle, assert permissions (no `home`/`host`, PulseAudio, DMA-BUF variable), start it under a virtual display, and check that the FileChooser and OpenURI portals answer.
8. **Release asset.** `learnvault-linux-desktop-<arch>.flatpak` is in `SHA256SUMS` and the asset table in `docs/releases/process.md`.

## Remaining

- The sandbox job does not yet import a synthetic course folder through the portal, play a lesson, or close and reopen the app.
- "Open in system player" (see above) is not portal-aware.

## Decisions (resolved)

- **Channel:** GitHub Release asset only. No Flathub or other store.
- **Runtime:** `org.gnome.Platform` branch 51, the current release. GNOME 48 reached end of life in March 2026. The CI job checks the branch exists on Flathub before building.
- **Release gate:** the bundle is published by the same `publish` job as the other assets, so it waits for the `release` environment approval.

## Open decisions

None blocking. Whether to add a Flathub submission later is a separate decision and would need its own review.

## Acceptance

- The bundle installs and starts on both architectures in the CI sandbox job (met).
- The folder picker works through the portal; no path is sent from the webview (portal reachability is checked in CI; the full import flow is not).
- Lesson playback has sound (the socket is granted; playback itself is not exercised in CI).
- The updater does not run inside the Flatpak (met).
- `foundation.rs` and the `SHA256SUMS` check pass with the new asset (met).
- `docs/operations/distribution.md` and the README table list the Flatpak as available (met).
