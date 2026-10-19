//! Erros tipados do domínio.
//!
//! Cada variante nomeia uma falha concreta do domínio. Nenhum caminho de erro usa
//! `panic` ou `unwrap`: falhas são propagadas como valores e traduzidas pelos
//! adaptadores (servidor HTTP, comandos Tauri) para o formato de cada plataforma.
//!
//! As mensagens de `Display` são para log e diagnóstico local. Os adaptadores escolhem o
//! texto apresentado ao cliente e não devem ecoar caminhos do sistema de arquivos.

use std::path::PathBuf;

/// Erro do domínio do LearnVault.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// O arquivo do banco de dados não pôde ser aberto ou criado.
    #[error("não foi possível abrir o banco de dados em {path}")]
    OpenDatabase {
        /// Caminho do arquivo do banco de dados.
        path: PathBuf,
        /// Causa original reportada pelo SQLite.
        #[source]
        source: rusqlite::Error,
    },

    /// A versão registrada no banco não pôde ser lida.
    ///
    /// É uma leitura, não uma migração: separar as duas variantes evita reportar uma
    /// migração que nunca chegou a ser tentada.
    #[error("não foi possível ler a versão do schema do banco de dados")]
    ReadSchemaVersion {
        /// Causa original reportada pelo SQLite.
        #[source]
        source: rusqlite_migration::Error,
    },

    /// A cadeia de migrações não pôde ser aplicada.
    #[error("não foi possível aplicar as migrações do banco de dados")]
    Migrations(#[from] rusqlite_migration::Error),

    /// O banco foi criado por uma versão mais nova deste programa.
    ///
    /// O schema nunca é rebaixado automaticamente: prosseguir poderia corromper dados
    /// gravados por uma versão que este binário não conhece.
    #[error(
        "o schema do banco está na versão {found}, acima da versão {known} conhecida por este programa"
    )]
    SchemaTooNew {
        /// Versão gravada no banco de dados.
        found: usize,
        /// Versão conhecida por este programa.
        known: usize,
    },

    /// O arquivo pertence ao schema legado do serviço Go.
    ///
    /// A conversão não é automática: abrir esse arquivo como Rust poderia criar um segundo
    /// administrador e deixar os dados legados invisíveis.
    #[error(
        "legacy_go_database_requires_migration: back up the database and migrate it before using the Rust server"
    )]
    LegacyGoDatabase,

    /// Falha de entrada/saída durante a importação legada.
    #[error("falha de {operation} no arquivo de importação {path}")]
    LegacyImportIo {
        /// Operação que falhou.
        operation: String,
        /// Caminho envolvido.
        path: PathBuf,
        /// Causa do sistema operacional.
        #[source]
        source: std::io::Error,
    },

    /// Um valor do banco Go não satisfaz o contrato de migração.
    #[error("valor inválido na importação legada: {field} = {value}")]
    LegacyImportInvalidValue {
        /// Campo inválido.
        field: String,
        /// Valor recusado.
        value: String,
    },

    /// Dois registros diferentes representam a mesma chave Rust.
    #[error("colisão na importação legada para o caminho {path}")]
    LegacyImportCollision {
        /// Caminho normalizado em colisão.
        path: String,
    },

    /// Uma linha legada aponta para uma entidade que não existe.
    #[error("referência órfã na importação legada: {relation} = {key}")]
    LegacyImportOrphan {
        /// Relação que contém a referência.
        relation: String,
        /// Chave apontada.
        key: String,
    },

    /// A importação seria ambígua porque um caminho é usado como mais de uma chave.
    #[error("caminho inválido na importação legada: {path}")]
    LegacyImportInvalidPath {
        /// Caminho recusado.
        path: String,
    },

    /// Falha de SQL que não se enquadra nas variantes anteriores.
    #[error("erro de banco de dados")]
    Database(#[from] rusqlite::Error),

    /// A raiz da biblioteca não existe.
    #[error("a raiz da biblioteca não existe em {path}")]
    LibraryRootMissing {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
    },

    /// A raiz da biblioteca existe, mas não é um diretório.
    #[error("a raiz da biblioteca em {path} não é um diretório")]
    LibraryRootNotDirectory {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
    },

    /// A raiz da biblioteca existe, mas não pôde ser lida.
    #[error("não foi possível ler a raiz da biblioteca em {path}")]
    LibraryRootUnreadable {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
        /// Causa original reportada pelo sistema de arquivos.
        #[source]
        source: std::io::Error,
    },

    /// A raiz configurada da biblioteca foi substituída por um link, junction ou ponto de
    /// reparse depois de ter sido resolvida.
    ///
    /// A raiz já é entregue canonicalizada pela integração de plataforma. Se o componente
    /// final virou um link, segui-lo autorizaria arquivos fora da biblioteca, então a
    /// substituição é recusada em vez de resolvida.
    #[error("a raiz da biblioteca é um link e não é seguida em {path}")]
    LibraryRootIsLink {
        /// Caminho informado como raiz da biblioteca.
        path: PathBuf,
    },

    /// Um caminho relativo da biblioteca é inválido ou tentaria escapar da raiz.
    #[error("caminho relativo inválido na biblioteca: {path}")]
    InvalidRelativePath {
        /// Caminho recusado.
        path: String,
    },

    /// A tentativa de varredura não pôde ser registrada no histórico.
    ///
    /// Registrar o histórico é secundário: a falha original da varredura é preservada
    /// como causa, para que a razão pela qual a biblioteca não pôde ser lida nunca seja
    /// escondida por um erro de escrita no banco.
    #[error("não foi possível registrar a varredura com falha no histórico")]
    ScanHistoryNotRecorded {
        /// Falha original da varredura.
        #[source]
        scan_error: Box<Error>,
        /// Causa original reportada pelo SQLite ao gravar o histórico.
        history_error: rusqlite::Error,
    },

    /// O identificador de mídia não existe na biblioteca.
    #[error("mídia desconhecida na biblioteca: {media_id}")]
    MediaNotFound {
        /// Identificador consultado.
        media_id: String,
    },

    /// A linha de catálogo existe, mas está marcada como ausente.
    #[error("a mídia está marcada como ausente: {media_id}")]
    MediaUnavailable {
        /// Identificador consultado.
        media_id: String,
    },

    /// A mídia estava disponível na última varredura e não está mais no disco.
    #[error("a mídia desapareceu do disco: {media_id}")]
    MediaMissingOnDisk {
        /// Identificador consultado.
        media_id: String,
    },

    /// O caminho resolvido saiu da raiz da biblioteca.
    #[error("o caminho resolvido está fora da biblioteca: {media_id}")]
    MediaOutsideLibrary {
        /// Identificador consultado.
        media_id: String,
    },

    /// O componente final do caminho é um link simbólico.
    ///
    /// A biblioteca é indexada sem seguir links, então um link não é uma mídia conhecida.
    #[error("a mídia é um link simbólico e não é seguida: {media_id}")]
    MediaIsSymlink {
        /// Identificador consultado.
        media_id: String,
    },

    /// O caminho aponta para algo que não é um arquivo comum.
    #[error("a mídia não é um arquivo comum: {media_id}")]
    MediaNotAFile {
        /// Identificador consultado.
        media_id: String,
    },

    /// O arquivo não pôde ser aberto ou inspecionado.
    #[error("não foi possível abrir a mídia: {media_id}")]
    MediaUnreadable {
        /// Identificador consultado.
        media_id: String,
        /// Causa original reportada pelo sistema de arquivos.
        #[source]
        source: std::io::Error,
    },

    /// O arquivo aberto não é o arquivo que foi validado no caminho.
    ///
    /// Só é verificável no Unix, onde o sistema expõe a identidade do descritor aberto.
    #[error("o arquivo aberto não corresponde ao caminho validado: {media_id}")]
    MediaIdentityMismatch {
        /// Identificador consultado.
        media_id: String,
    },

    /// O tamanho registrado no banco é inválido (negativo ou fora do `u64`).
    ///
    /// Um valor assim indica corrupção do catálogo, não um arquivo vazio: saturar
    /// para zero esconderia a falha e falsearia a detecção de alteração.
    #[error("o tamanho registrado é inválido no banco: {media_id}")]
    MediaCorruptSize {
        /// Identificador consultado.
        media_id: String,
    },

    /// O dono do progresso de estudo não foi identificado.
    ///
    /// O identificador de dono é opaco para o domínio, mas precisa existir: o progresso é
    /// sempre de alguém, e um dono vazio misturaria dados de donos diferentes.
    #[error("o dono do progresso precisa ser identificado")]
    InvalidOwnerId,

    /// A posição de retomada é inválida.
    #[error("posição de retomada inválida: {position_seconds}")]
    InvalidStudyPosition {
        /// Posição recusada, em segundos.
        position_seconds: i64,
    },

    /// O texto de uma nota é vazio ou excede o limite em bytes.
    #[error("texto de nota inválido")]
    InvalidStudyNote,

    /// A nota não existe para o dono consultado.
    #[error("a nota não existe")]
    StudyNoteNotFound,

    /// A mídia já tem o teto de notas para este dono.
    #[error("a mídia já tem o número máximo de notas")]
    TooManyNotes,

    /// A mídia foi removida explicitamente do catálogo.
    ///
    /// A linha de `media` e o progresso continuam existindo: a remoção tira a mídia do
    /// catálogo, mas não apaga dados. Uma mídia removida não é entregue nem listada.
    #[error("a mídia foi removida do catálogo: {media_id}")]
    MediaRemoved {
        /// Identificador consultado.
        media_id: String,
    },

    /// A remoção exige confirmação porque existe progresso associado.
    ///
    /// O progresso nunca é apagado por uma remoção; quando ele existe, quem chama precisa
    /// confirmar explicitamente que quer retirar a mídia do catálogo mesmo assim.
    #[error(
        "a mídia tem progresso de {progress_owners} dono(s) e exige confirmação para ser removida: {media_id}"
    )]
    MediaRemovalNeedsConfirmation {
        /// Identificador consultado.
        media_id: String,
        /// Quantidade de donos distintos com progresso na mídia.
        progress_owners: usize,
    },
}

/// Resultado das operações do domínio.
pub type Result<T> = std::result::Result<T, Error>;
