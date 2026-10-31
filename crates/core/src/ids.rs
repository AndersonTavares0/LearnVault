//! Primitivas de identidade opaca.
//!
//! A identidade de uma biblioteca ou de um arquivo de mídia é um identificador opaco
//! gerado pelo domínio. Ele nunca é derivado do caminho: renomear ou mover um arquivo
//! não pode mudar a identidade de outro registro, e a mesma localização reutilizada
//! não reaproveita a identidade por acidente.
//!
//! Os identificadores são UUID versão 7, ordenáveis pelo instante de criação.
//!
//! A geração usa o relógio do sistema. Um relógio anterior à época Unix ou uma fonte de
//! aleatoriedade indisponível fariam `Uuid::now_v7` entrar em pânico; esses cenários
//! exigem um sistema fundamentalmente quebrado e são registrados como limitação
//! conhecida, em vez de mascarados por um identificador repetido.

use uuid::Uuid;

/// Gera um identificador opaco de biblioteca.
pub fn new_library_id() -> String {
    Uuid::now_v7().to_string()
}

/// Gera um identificador opaco de mídia.
///
/// Este é o identificador exposto ao frontend: a interface aponta para a mídia pelo
/// `media_id`, nunca pelo caminho do arquivo.
pub fn new_media_id() -> String {
    Uuid::now_v7().to_string()
}

/// Gera um identificador opaco de anotação de estudo.
pub fn new_note_id() -> String {
    Uuid::now_v7().to_string()
}
