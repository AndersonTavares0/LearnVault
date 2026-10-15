//! Aplicativo desktop do LearnVault (Tauri 2).
//!
//! Esta é a fatia de fundação: apenas o webview com a SPA embutida e o ACL mínimo.
//! Não há comando IPC, listener de rede, servidor HTTP nem plugin de shell, filesystem,
//! http ou updater. O transporte IPC completo pertence a uma fatia posterior.

/// Inicializa o webview Tauri apontando para a SPA embutida em `frontend-dist`.
///
/// O handler de comandos está vazio de propósito: enquanto o IPC de domínio não
/// existir, nenhuma ação privilegiada é exposta ao frontend. O deny-by-default do
/// ACL do Tauri garante que nada fora das permissões declaradas em `capabilities/`
/// seja executável pelo webview.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
