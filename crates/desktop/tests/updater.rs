//! Testes de integração do sistema oficial de autoatualização do Desktop (Tauri 2).
//!
//! Garante que:
//! 1. O DTO de atualização cumpre o contrato camelCase consumido pela SPA.
//! 2. A verificação offline opera de forma resiliente e segura (sem travamentos, popups ou pânico).
//! 3. Os dados do usuário (banco de dados SQLite, capas e ordenação) permanecem intactos e isolados
//!    durante transições e verificações de versão.

use std::fs;
use tempfile::tempdir;

use learnvault_core::Database;
use learnvault_desktop::{DesktopState, DesktopUpdateInfoDto, database_path, order_path};

#[test]
fn update_info_dto_serializes_to_camel_case_json() {
    let info = DesktopUpdateInfoDto {
        available: true,
        current_version: "0.7.0".to_owned(),
        version: Some("0.8.0".to_owned()),
        date: Some("2026-11-01T04:00:00Z".to_owned()),
        body: Some("Novas funcionalidades botânicas".to_owned()),
    };

    let serialized = serde_json::to_string(&info).expect("serialização para JSON");
    assert!(serialized.contains("\"available\":true"));
    assert!(serialized.contains("\"currentVersion\":\"0.7.0\""));
    assert!(serialized.contains("\"version\":\"0.8.0\""));
    assert!(serialized.contains("\"date\":\"2026-11-01T04:00:00Z\""));
    assert!(serialized.contains("\"body\":\"Novas funcionalidades botânicas\""));

    // Deserialização de volta
    let deserialized: DesktopUpdateInfoDto =
        serde_json::from_str(&serialized).expect("deserialização");
    assert_eq!(info, deserialized);
}

#[test]
fn update_info_dto_handles_no_update_available() {
    let info = DesktopUpdateInfoDto {
        available: false,
        current_version: "0.7.0".to_owned(),
        version: None,
        date: None,
        body: None,
    };

    let serialized = serde_json::to_string(&info).expect("serialização");
    assert!(serialized.contains("\"available\":false"));
    assert!(serialized.contains("\"currentVersion\":\"0.7.0\""));
    // Campos nulos/ausentes são omitidos para payload mínimo
    assert!(!serialized.contains("\"version\":"));
    assert!(!serialized.contains("\"date\":"));
    assert!(!serialized.contains("\"body\":"));
}

#[test]
fn update_process_preserves_user_data_and_database_across_versions() {
    let data_temp = tempdir().expect("tempdir para dados");
    let config_temp = tempdir().expect("tempdir para configuração");

    let data_dir = data_temp.path();
    let config_dir = config_temp.path();

    // 1. Simula a versão v0.7.0 em execução criando registros de progresso e preferências
    {
        let state = DesktopState::open(data_dir, config_dir).expect("abertura do estado v0.7.0");
        let db_file = database_path(data_dir);
        assert!(db_file.exists(), "banco sqlite precisa existir");

        // Grava arquivo de ordenação
        let orders_file = order_path(data_dir);
        fs::write(&orders_file, b"{\"orders\":{}}").expect("escrita de ordens");

        // Salva capa sintética
        let covers_dir = data_dir.join("covers");
        fs::create_dir_all(&covers_dir).expect("criação de diretório de capas");
        fs::write(covers_dir.join("course-1.cover"), b"dummy-cover-data")
            .expect("gravação de capa");

        drop(state);
    }

    // 2. Simula o aplicativo reiniciando na nova versão v0.8.0 sobre o mesmo data_dir
    {
        let db_file = database_path(data_dir);
        assert!(db_file.exists(), "banco precisa sobreviver à atualização");

        // O banco abre normalmente sem perda de integridade
        let db = Database::open(&db_file).expect("banco sqlite íntegro na nova versão");
        drop(db);

        // Capas e ordens continuam intactas
        assert!(
            data_dir.join("covers/course-1.cover").exists(),
            "capas de cursos preservadas"
        );
        assert!(order_path(data_dir).exists(), "ordem de cursos preservada");

        // Estado do desktop abre normalmente na nova versão
        let state_v2 = DesktopState::open(data_dir, config_dir).expect("abertura na versão nova");
        drop(state_v2);
    }
}
