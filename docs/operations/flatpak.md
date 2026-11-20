# Flatpak desktop (Linux)

**Status:** built in CI and attached to every GitHub Release since v0.9.1 (a pre-release); v0.9.5 is the first non-pre-release that ships it. The bundle is distributed only through GitHub Releases. There is no Flathub submission. The desktop updater is off inside the Flatpak.

## Scope across systems

| System | Desktop app | Plan |
| :--- | :--- | :--- |
| Windows | NSIS installer with signed updater | Available. |
| Linux (x86_64, aarch64) | Flatpak bundle attached to each GitHub Release | Built by `.github/workflows/flatpak.yml`. |
| macOS | none | Not planned. No build job exists and the Tauri config targets `nsis` only. |
| Any OS (server) | Linux tarball and OCI image | Available. The Flatpak does not replace them. |

The Flatpak contains the desktop app only.

## What exists

- `deploy/packages/app.learnvault.desktop.yml`: manifest on `org.gnome.Platform` 51. It installs the prebuilt `target/release/learnvault-desktop`, the `.desktop` file, the metainfo and the icon. It grants IPC, Wayland, X11 fallback, PulseAudio and DRI, and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Without that variable the app exits at once with `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display` on NVIDIA + Wayland sessions (reproduced on an RTX 3060 Ti with the GNOME 51 runtime).
- `.github/workflows/flatpak.yml` (called by `release.yml`, also runnable with `workflow_dispatch`): builds the release binary with the release version injected into the Tauri config (`tauri.release.json`, from `.github/release-please/version.txt`, as the Windows job does). The workspace version is `0.0.0`, which Settings > Updates would otherwise show as the installed version. It then builds, installs and permission-checks the bundle, starts it under a virtual display, and checks that the FileChooser and OpenURI portals answer.
- `crates/desktop/tests/foundation.rs`: fails if the manifest grants `--filesystem=home` or `--filesystem=host`.
- `crates/desktop/tauri.conf.json`: `bundle.targets` is `["nsis"]`, so Tauri produces no Linux bundle; `flatpak.yml` builds the Flatpak from the release binary.
- The folder picker uses `tauri-plugin-dialog` with its `xdg-portal` feature, so inside the Flatpak it goes through the FileChooser portal.
- The updater is gated on `FLATPAK_ID` (`crates/desktop/src/command.rs`).
- "Open in system player" returns `false` inside the Flatpak (`crates/desktop/src/command.rs`): the file path belongs to the host and the sandbox has no filesystem access.
- The metainfo is validated with `appstreamcli` in CI. No store screenshots or age rating are needed because nothing is submitted to a store.
- The bundle is published as `learnvault-linux-desktop-<arch>.flatpak`, listed in `SHA256SUMS` and in the asset table in [`process.md`](../releases/process.md#published-release-assets).

## CI sandbox checks

Both architectures (`ubuntu-latest`, `ubuntu-24.04-arm`) build with `flatpak-builder` on the GNOME 51 runtime, then:

1. Install the bundle.
2. Assert its permissions: no `home` or `host` filesystem, PulseAudio present, DMA-BUF variable present.
3. Start it under a virtual display and require that it is still running after 40 seconds.
4. Check that the FileChooser and OpenURI portals answer.

## Remaining

- The sandbox job does not import a synthetic course folder through the portal, play a lesson, or close and reopen the app.
- "Open in system player" is not routed through the OpenURI portal.

## Decisions

- **Channel:** GitHub Release asset only. No Flathub or other store; adding one would need its own review.
- **Runtime:** `org.gnome.Platform` branch 51. GNOME 48 reached end of life in March 2026. CI checks that the branch exists on Flathub before building.
- **Release gate:** the same `publish` job that uploads the other assets publishes the bundle, so it waits for approval in the `release` environment.

## Acceptance

- The bundle installs and starts on both architectures in the CI sandbox job (met).
- The folder picker works through the portal and no path comes from the webview. CI checks only that the portal answers.
- Lesson playback has sound. The socket is granted; CI does not play a lesson.
- The updater does not run inside the Flatpak (met).
- `foundation.rs` and the `SHA256SUMS` check pass with the Flatpak asset (met).
- [`distribution.md`](distribution.md) and the README table list the Flatpak (met).
