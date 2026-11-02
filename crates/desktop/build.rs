//! Build script do crate desktop.
//!
//! `try_build` lê `tauri.conf.json`, resolve o ACL a partir de `capabilities/` e, com
//! [`tauri_build::AppManifest`], gera as permissões `allow-<comando>` dos comandos IPC do
//! aplicativo (`desktop_catalog`, `desktop_import_course`, `desktop_remove_course`,
//! `desktop_import_course_cover`, `desktop_remove_course_cover`, `desktop_rescan`,
//! `desktop_study_progress`, `desktop_record_position`, `desktop_set_completed` e
//! `desktop_recent_study`, `desktop_set_course_order` e `desktop_reset_course_order`). Ele
//! não abre sockets nem depende de display; a validação de runtime pertence ao
//! `tauri dev`/`tauri build`.

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
            "desktop_study_summary",
            "desktop_record_focus_session",
            "desktop_focus_summary",
            "desktop_reward_summary",
            "desktop_claim_daily_mission",
            "desktop_weekly_goal",
            "desktop_set_weekly_goal",
            "desktop_media_removal_impact",
            "desktop_remove_media",
            "desktop_set_course_order",
            "desktop_reset_course_order",
            "desktop_check_update",
            "desktop_install_update",
            "desktop_media_capabilities",
            "desktop_open_in_system_player",
        ]));
    tauri_build::try_build(attributes).expect("falha ao executar o tauri-build do LearnVault");
}
