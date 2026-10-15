//! Seleção e persistência segura da raiz da biblioteca desktop.
//!
//! A raiz da biblioteca é escolhida pelo usuário por um seletor nativo acionado pelo
//! Rust. O webview nunca envia nem recebe um caminho: o único dado que cruza o IPC é o
//! booleano `configured`. Esta camada resolve o caminho escolhido para a forma canônica,
//! recusa raízes que não sejam diretórios legíveis (inclusive links simbólicos, junctions
//! e pontos de reparse no topo) e grava o caminho canonicalizado em um arquivo de
//! configuração versionado do próprio aplicativo.
//!
//! Regras de segurança preservadas aqui:
//!
//! - a biblioteca é externa e **somente leitura**: nada é criado, movido ou modificado
//!   dentro dela, nem quando a seleção é recusada;
//! - a configuração vive fora da raiz da biblioteca e fora do banco SQLite;
//! - a gravação é atômica (arquivo temporário + `rename`), então uma interrupção nunca
//!   deixa a configuração pela metade;
//! - a leitura revalida a raiz no startup: configuração ausente, corrompida, em versão
//!   desconhecida ou com raiz removida/inacessível resulta em "não configurado";
//! - nenhuma mensagem de erro carrega caminho do sistema de arquivos.

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Versão do formato de configuração da raiz da biblioteca.
pub const CONFIG_VERSION: u32 = 1;

/// Nome do arquivo de configuração da raiz dentro do diretório de configuração do app.
pub const CONFIG_FILE_NAME: &str = "library.json";

/// Raiz da biblioteca já validada e canonicalizada.
///
/// O caminho é privado de propósito: ele existe apenas para o Rust e nunca é serializado
/// para o webview. O `Debug` também oculta o caminho, para que um registro de diagnóstico
/// não vaze a localização da biblioteca.
#[derive(Clone, PartialEq, Eq)]
pub struct LibraryRoot(PathBuf);

impl LibraryRoot {
    /// Valida e canonicaliza um caminho candidato a raiz da biblioteca.
    ///
    /// O caminho precisa existir, ser um diretório, não ser um link simbólico, junction
    /// ou ponto de reparse no próprio topo, e ser legível (`read_dir`). O resultado é a
    /// forma canônica — a única forma persistida. Nenhum arquivo é criado ou modificado.
    pub fn resolve(path: &Path) -> Result<Self, LibraryError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(LibraryError::Missing);
            }
            Err(_) => return Err(LibraryError::NotReadable),
        };

        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(LibraryError::Link);
        }
        if !metadata.is_dir() {
            return Err(LibraryError::NotADirectory);
        }

        let canonical = fs::canonicalize(path).map_err(|_| LibraryError::NotReadable)?;
        fs::read_dir(&canonical).map_err(|_| LibraryError::NotReadable)?;

        Ok(Self(canonical))
    }

    /// Caminho canônico da raiz, somente para uso interno do Rust.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Debug for LibraryRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LibraryRoot(<oculto>)")
    }
}

/// Falha estável da validação ou da persistência da raiz. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryError {
    /// O caminho escolhido não existe.
    Missing,
    /// O caminho escolhido não é um diretório.
    NotADirectory,
    /// O caminho escolhido não pôde ser lido (canonicalização ou listagem).
    NotReadable,
    /// O caminho escolhido é um link, junction ou ponto de reparse.
    Link,
    /// A configuração não pôde ser gravada com segurança.
    WriteFailed,
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            LibraryError::Missing => "a raiz da biblioteca não existe",
            LibraryError::NotADirectory => "a raiz da biblioteca não é um diretório",
            LibraryError::NotReadable => "a raiz da biblioteca não pode ser lida",
            LibraryError::Link => "a raiz da biblioteca não pode ser um link",
            LibraryError::WriteFailed => "não foi possível gravar a configuração do aplicativo",
        })
    }
}

impl std::error::Error for LibraryError {}

/// Formato serializado da configuração, versionado para permitir migração futura.
#[derive(Debug, Serialize, Deserialize)]
struct StoredConfig {
    version: u32,
    library_root: PathBuf,
}

/// Carrega e revalida a raiz persistida, se houver uma.
///
/// Devolve `None` quando a configuração está ausente, corrompida, em versão desconhecida
/// ou quando a raiz registrada não passa mais na validação (removida, inacessível ou
/// transformada em link). O chamador trata `None` como "não configurado"; a leitura nunca
/// falha de forma fatal nem cria nada no sistema de arquivos.
pub fn load_library_root(config_dir: &Path) -> Option<LibraryRoot> {
    let raw = fs::read(config_dir.join(CONFIG_FILE_NAME)).ok()?;
    let stored: StoredConfig = serde_json::from_slice(&raw).ok()?;
    if stored.version != CONFIG_VERSION {
        return None;
    }
    LibraryRoot::resolve(&stored.library_root).ok()
}

/// Grava atomicamente a raiz canonicalizada no diretório de configuração.
///
/// Se `config_dir` estiver dentro da própria biblioteca — uma configuração errada — a
/// gravação é recusada **antes** de qualquer criação, para que nada seja criado dentro
/// da biblioteca. O diretório de configuração é criado fora da raiz, e o arquivo é
/// escrito em um temporário e depois renomeado sobre o destino.
pub fn store_library_root(config_dir: &Path, root: &LibraryRoot) -> Result<(), LibraryError> {
    if config_dir_is_inside(config_dir, root) {
        return Err(LibraryError::WriteFailed);
    }

    fs::create_dir_all(config_dir).map_err(|_| LibraryError::WriteFailed)?;

    let payload = serde_json::to_vec(&StoredConfig {
        version: CONFIG_VERSION,
        library_root: root.as_path().to_path_buf(),
    })
    .map_err(|_| LibraryError::WriteFailed)?;

    write_atomically(&config_dir.join(CONFIG_FILE_NAME), &payload)
}

/// Indica se o diretório de configuração está dentro da biblioteca.
///
/// Compara formas canônicas do ancestral existente mais próximo, então funciona mesmo
/// quando o diretório de configuração ainda não existe. A comparação é contra a raiz já
/// canonicalizada, evitando falsos negativos por diferença de prefixo (por exemplo o
/// prefixo verbatim do Windows).
fn config_dir_is_inside(config_dir: &Path, root: &LibraryRoot) -> bool {
    let mut probe = config_dir;
    loop {
        if probe.exists() {
            return match fs::canonicalize(probe) {
                Ok(canonical) => canonical.starts_with(root.as_path()),
                Err(_) => config_dir.starts_with(root.as_path()),
            };
        }
        match probe.parent() {
            Some(parent) => probe = parent,
            None => return config_dir.starts_with(root.as_path()),
        }
    }
}

/// Escreve `bytes` em `dest` de forma atômica: temporário, `sync` e `rename`.
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    let directory = dest.parent().ok_or(LibraryError::WriteFailed)?;
    let temporary = directory.join(format!(".{}.{}.tmp", CONFIG_FILE_NAME, std::process::id()));

    let written = fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(LibraryError::WriteFailed);
    }

    if fs::rename(&temporary, dest).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(LibraryError::WriteFailed);
    }

    Ok(())
}

/// Verdadeiro quando o metadado marca um ponto de reparse (junction ou symlink) no Windows.
#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Fora do Windows, junctions e pontos de reparse não existem; links já são detectados.
#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
