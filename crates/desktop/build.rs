//! Build script do crate desktop.
//!
//! `try_build` lê `tauri.conf.json`, resolve o ACL a partir de `capabilities/` e, com
//! [`tauri_build::AppManifest`], gera as permissões `allow-<comando>` dos comandos IPC do
//! aplicativo (`desktop_catalog`, `desktop_import_course`, `desktop_remove_course`,
//! `desktop_import_course_cover`, `desktop_remove_course_cover`, `desktop_rescan`,
//! `desktop_study_progress`, `desktop_record_position`, `desktop_set_completed`,
//! `desktop_recent_study`, `desktop_set_course_order`, `desktop_reset_course_order`,
//! `desktop_study_notes`, `desktop_study_notes_all`, `desktop_create_study_note`,
//! `desktop_update_study_note` e `desktop_delete_study_note`). Ele não abre sockets nem
//! depende de display; a validação
//! de runtime pertence ao `tauri dev`/`tauri build`.

fn main() {
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "desktop_catalog",
            "desktop_import_course",
            "desktop_remove_course",
            "desktop_import_course_cover",
            "desktop_remove_course_cover",
            "desktop_rescan",
            "desktop_study_progress",
            "desktop_record_position",
            "desktop_set_completed",
            "desktop_recent_study",
            "desktop_media_removal_impact",
            "desktop_remove_media",
            "desktop_set_course_order",
            "desktop_reset_course_order",
            "desktop_study_notes",
            "desktop_study_notes_all",
            "desktop_create_study_note",
            "desktop_update_study_note",
            "desktop_delete_study_note",
        ]));
    tauri_build::try_build(attributes).expect("falha ao executar o tauri-build do LearnVault");
}
