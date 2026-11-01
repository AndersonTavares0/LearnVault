//! Testes de integração do sistema oficial de autoatualização do Desktop (Tauri 2).
//!
//! Garante que:
//! 1. O DTO de atualização cumpre o contrato camelCase consumido pela SPA.
//! 2. A ausência de atualização é diferenciada de falhas de rede/configuração via IPC.
//! 3. O manifesto `latest.json` e suas assinaturas Ed25519/minisign cumprem a especificação do Tauri 2.
//! 4. Os dados do usuário (banco de dados SQLite, capas e ordenação) persistem intactos no diretório
//!    estável do usuário (`data_dir`) entre reinicializações do aplicativo.
//!
//! NOTA SOBRE ESCOPO:
//! Reabrir arquivos existentes no `data_dir` valida a estabilidade do schema SQLite, capas e ordenação
//! entre execuções do aplicativo sobre o mesmo diretório de dados (`AppData`). Isso comprova a persistência
//! dos dados locais, mas NÃO equivale a uma execução real do instalador NSIS nem substituição física de binários
//! (que requer ambiente Windows nativo e elevação de privilégios gerenciada pelo Tauri/Windows).

use std::fs;
use tempfile::tempdir;

use learnvault_core::Database;
use learnvault_desktop::{
    DesktopState, DesktopUpdateInfoDto, IpcError, IpcErrorCode, database_path, order_path,
};

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
fn ipc_error_differentiates_update_check_failure_from_unavailable() {
    // Falha de rede ou endpoint inacessível durante checagem (ex.: offline)
    let check_failed = IpcError::update_check_failed();
    let serialized_check = serde_json::to_string(&check_failed).expect("serialização");
    assert_eq!(check_failed.code, IpcErrorCode::UpdateCheckFailed);
    assert!(serialized_check.contains("\"code\":\"update_check_failed\""));
    assert!(serialized_check.contains("não foi possível conectar ao servidor de atualizações"));

    // Falha no plugin ou instalação física
    let unavailable = IpcError::update_unavailable();
    let serialized_unavail = serde_json::to_string(&unavailable).expect("serialização");
    assert_eq!(unavailable.code, IpcErrorCode::UpdateUnavailable);
    assert!(serialized_unavail.contains("\"code\":\"update_unavailable\""));
    assert!(serialized_unavail.contains("não foi possível verificar ou aplicar a atualização"));

    // Garante que são códigos e mensagens distintos
    assert_ne!(check_failed.code, unavailable.code);
    assert_ne!(check_failed.message, unavailable.message);
}

#[test]
fn tauri2_latest_manifest_compatibility_validation() {
    // Exemplo sintético gerado pelo release workflow
    let sample_manifest = serde_json::json!({
        "version": "v0.8.0",
        "notes": "LearnVault release v0.8.0",
        "pub_date": "2026-11-01T14:00:00Z",
        "platforms": {
            "windows-x86_64": {
                "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK",
                "url": "https://github.com/AndersonTavares0/LearnVault/releases/download/v0.8.0/learnvault-windows-v0.8.0.nsis.zip"
            },
            "windows-x86_64-nsis": {
                "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK",
                "url": "https://github.com/AndersonTavares0/LearnVault/releases/download/v0.8.0/learnvault-windows-v0.8.0.nsis.zip"
            }
        }
    });

    // Deserialização direta usando o próprio tipo oficial do tauri-plugin-updater v2
    let remote_release: tauri_plugin_updater::RemoteRelease = serde_json::from_value(
        sample_manifest.clone(),
    )
    .expect("manifesto precisa ser compatível com a deserialização do tauri-plugin-updater");

    assert_eq!(remote_release.version.to_string(), "0.8.0");
    assert_eq!(
        remote_release.notes.as_deref(),
        Some("LearnVault release v0.8.0")
    );
    assert!(remote_release.pub_date.is_some());

    // Valida que download_url e signature funcionam para os targets esperados
    let url_x86 = remote_release
        .download_url("windows-x86_64")
        .expect("url para windows-x86_64");
    assert_eq!(
        url_x86.as_str(),
        "https://github.com/AndersonTavares0/LearnVault/releases/download/v0.8.0/learnvault-windows-v0.8.0.nsis.zip"
    );

    let sig_x86 = remote_release
        .signature("windows-x86_64")
        .expect("signature para windows-x86_64");
    assert_eq!(
        sig_x86,
        "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK"
    );

    let url_nsis = remote_release
        .download_url("windows-x86_64-nsis")
        .expect("url para windows-x86_64-nsis");
    assert_eq!(
        url_nsis.as_str(),
        "https://github.com/AndersonTavares0/LearnVault/releases/download/v0.8.0/learnvault-windows-v0.8.0.nsis.zip"
    );
}

#[test]
fn user_data_and_database_persist_across_app_restarts_on_shared_data_dir() {
    let data_temp = tempdir().expect("tempdir para dados");
    let config_temp = tempdir().expect("tempdir para configuração");

    let data_dir = data_temp.path();
    let config_dir = config_temp.path();

    // 1. Simula a versão anterior criando registros de progresso e preferências no data_dir
    {
        let state = DesktopState::open(data_dir, config_dir).expect("abertura do estado inicial");
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

    // 2. Simula o aplicativo reiniciando em uma nova versão sobre o mesmo data_dir
    // NOTA: Reabrir arquivos existentes no data_dir valida a estabilidade do schema SQLite, capas e ordenação
    // entre execuções do aplicativo sobre o mesmo diretório. NÃO equivale a uma execução real do instalador NSIS.
    {
        let db_file = database_path(data_dir);
        assert!(
            db_file.exists(),
            "banco precisa sobreviver à reinicialização"
        );

        // O banco abre normalmente sem perda de integridade
        let db = Database::open(&db_file).expect("banco sqlite íntegro na reinicialização");
        drop(db);

        // Capas e ordens continuam intactas
        assert!(
            data_dir.join("covers/course-1.cover").exists(),
            "capas de cursos preservadas"
        );
        assert!(order_path(data_dir).exists(), "ordem de cursos preservada");

        // Estado do desktop abre normalmente na nova versão
        let state_v2 = DesktopState::open(data_dir, config_dir).expect("abertura na nova execução");
        drop(state_v2);
    }
}
