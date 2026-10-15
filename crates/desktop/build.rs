//! Build script do crate desktop.
//!
//! `try_build` lê `tauri.conf.json`, resolve o ACL a partir de `capabilities/` e, com
//! [`tauri_build::AppManifest`], gera as permissões `allow-<comando>` dos comandos IPC do
//! aplicativo (`desktop_config`, `desktop_catalog`, `desktop_select_library` e
//! `desktop_rescan`). Ele não abre sockets nem depende de display; a validação de runtime
//! pertence ao `tauri dev`/`tauri build`.

fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "desktop_config",
            "desktop_catalog",
            "desktop_select_library",
            "desktop_rescan",
        ]));
    tauri_build::try_build(attributes).expect("falha ao executar o tauri-build do LearnVault");
}
