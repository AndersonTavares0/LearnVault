# Flatpak desktop (Linux)

**Status:** built in CI and attached to every GitHub Release since v0.9.1 (a pre-release); v0.9.5 is the first non-pre-release that ships it. The bundle is distributed only through GitHub Releases. There is no Flathub submission. The desktop updater is off inside the Flatpak.

## Scope across systems

| System | Desktop app | Plan |
| :--- | :--- | :--- |
| Windows | NSIS installer with signed updater | Available. |
| Linux (x86_64, aarch64) | Flatpak bundle from GitHub Releases | Built by `.github/workflows/flatpak.yml`. |
| macOS | none | Not planned. No build job exists and the Tauri config targets `nsis` only. |
| Linux (server) | Linux tarball and OCI image | Available. The Flatpak does not replace them. |

The Flatpak contains the desktop app only.

## Packaging and permissions

- `deploy/packages/app.learnvault.desktop.yml`: manifest on `org.gnome.Platform` 51. It installs the prebuilt `target/release/learnvault-desktop`, the `.desktop` file, the metainfo and the icon. It grants IPC, Wayland, X11 fallback, PulseAudio and DRI, and sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`. The variable works around a WebKitGTK DMA-BUF renderer failure on NVIDIA + Wayland sessions, where the app exited at once with `Gdk-Message: Error 71 (Protocol error) dispatching to Wayland display` (fixed in v0.9.6, #697).
- `.github/workflows/flatpak.yml` (called by `release.yml`, also runnable with `workflow_dispatch`): builds the release binary with the release version injected into the Tauri config (`tauri.release.json`, from `.github/release-please/version.txt`, as the Windows job does). The workspace version is `0.0.0`, which Settings > Updates would otherwise show as the installed version. It then builds, installs and permission-checks the bundle, starts it under a virtual display, and checks that the FileChooser and OpenURI portals answer.
- `crates/desktop/tests/foundation.rs`: fails if the manifest grants `--filesystem=home` or `--filesystem=host`.
- `crates/desktop/tauri.conf.json`: `bundle.targets` is `["nsis"]`, so Tauri produces no Linux bundle; `flatpak.yml` builds the Flatpak from the release binary.
- The folder picker uses `tauri-plugin-dialog` with its `xdg-portal` feature, so inside the Flatpak it goes through the FileChooser portal.
- The updater is gated on `FLATPAK_ID` (`crates/desktop/src/command.rs`).
- Inside Flatpak, the Open in system player command returns `false` before attempting to open the file (`crates/desktop/src/command.rs`). The sandbox grants no broad `home` or `host` access.
- The metainfo is validated with `appstreamcli` in CI. No store screenshots or age rating are needed because nothing is submitted to a store.
- The bundle is published as `learnvault-linux-desktop-<arch>.flatpak`, listed in `SHA256SUMS` and in the asset table in [`process.md`](../releases/process.md#published-release-assets).

## CI sandbox checks

The amd64 job runs on `ubuntu-24.04` and the arm64 job on `ubuntu-24.04-arm`. Both build on GNOME 51 with `flatpak-builder`, then:

1. Install the bundle.
2. Assert its permissions: no `home` or `host` filesystem, PulseAudio present, DMA-BUF variable present.
3. Start it under a virtual display and require that it is still running after 40 seconds.
4. Check that the FileChooser and OpenURI portals answer.

## Open work

- Import a synthetic course folder through the portal, play a lesson, and close and reopen the app in the sandbox job.
- Route "Open in system player" through the OpenURI portal.

## Decisions

- **Channel:** GitHub Release asset only. No Flathub or other store; adding one would need its own review.
- **Runtime:** `org.gnome.Platform` branch 51. GNOME 48 reached end of life in March 2026. CI checks that the branch exists on Flathub before building.
- **Release gate:** the same `publish` job that uploads the other assets publishes the bundle, so it waits for approval in the `release` environment.

## CI limits

The folder picker is checked only for portal reachability, and no lesson is played, so audio is verified only as a granted socket.
