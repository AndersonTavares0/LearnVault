//! Registro local e persistência segura dos cursos do desktop.
//!
//! Cada curso é uma pasta externa escolhida pelo usuário por um seletor nativo acionado
//! pelo Rust. O webview nunca envia nem recebe um caminho: os únicos dados que cruzam o
//! IPC são a identidade opaca do curso e o booleano de configuração.
//!
//! Esta camada resolve um caminho escolhido para a forma canônica, recusa pastas que não
//! sejam diretórios legíveis (inclusive links simbólicos, junctions e pontos de reparse no
//! topo) e grava a lista canonicalizada em um arquivo de configuração versionado do
//! próprio aplicativo.
//!
//! Regras de segurança preservadas aqui:
//!
//! - os cursos são externos e **somente leitura**: nada é criado, movido ou modificado
//!   dentro deles, nem quando uma escolha é recusada;
//! - a configuração vive fora das pastas de curso e fora do banco SQLite;
//! - a gravação é serializada: o temporário é criado exclusivamente (`create_new`, sem
//!   seguir link nem reaproveitar arquivo existente) e a troca usa `rename`; no Windows,
//!   como as APIs padrão não substituem um destino existente, ele é removido imediatamente
//!   antes da troca, deixando uma janela inevitável em que a configuração pode não existir;
//! - a leitura revalida cada raiz no startup: configuração ausente, corrompida, em versão
//!   desconhecida ou com pasta removida/inacessível simplesmente não produz aquele curso;
//! - nenhuma mensagem de erro carrega caminho do sistema de arquivos.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use learnvault_core::{CourseRoot, LEGACY_LIBRARY_ID, course_id_from_path, course_title};
use serde::{Deserialize, Serialize};

/// Serializa as gravações da configuração no processo inteiro.
///
/// Sem o cadeado, duas gravações concorrentes competiriam pelo mesmo destino e poderiam
/// intercalar temporários; com ele, a configuração em disco é sempre o resultado de uma
/// gravação completa, na ordem de chegada. O cadeado é desta função, nunca do SQLite.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Número máximo de nomes de temporário tentados antes de desistir da gravação.
const MAX_TEMP_ATTEMPTS: u32 = 32;

/// Versão do formato de configuração dos cursos.
pub const CONFIG_VERSION: u32 = 2;

/// Nome do arquivo de configuração dentro do diretório de configuração do app.
pub const CONFIG_FILE_NAME: &str = "library.json";

/// Curso registrado no desktop: identidade estável e raiz já validada.
///
/// O caminho é privado de propósito: ele existe apenas para o Rust e nunca é serializado
/// para o webview. O `Debug` também oculta o caminho, para que um registro de diagnóstico
/// não vaze a localização do curso.
#[derive(Clone, PartialEq, Eq)]
pub struct CourseEntry {
    /// Identidade estável usada como `library_id` no banco.
    pub id: String,
    /// Raiz canônica validada do curso.
    pub root: CourseRoot,
}

impl CourseEntry {
    /// Cria a entrada derivando a identidade do caminho canônico.
    pub fn from_root(root: CourseRoot) -> Self {
        Self {
            id: course_id_from_path(root.as_path()),
            root,
        }
    }

    /// Título apresentado, derivado do nome da pasta do curso.
    pub fn title(&self) -> String {
        course_title(self.root.as_path())
    }
}

impl std::fmt::Debug for CourseEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CourseEntry")
            .field("id", &self.id)
            .field("root", &self.root)
            .finish()
    }
}

/// Falha estável da persistência da configuração. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryError {
    /// A configuração não pôde ser gravada com segurança.
    WriteFailed,
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            LibraryError::WriteFailed => "não foi possível gravar a configuração do aplicativo",
        })
    }
}

impl std::error::Error for LibraryError {}

/// Formato serializado da configuração, versionado para permitir migração futura.
#[derive(Debug, Serialize, Deserialize)]
struct StoredConfig {
    version: u32,
    courses: Vec<StoredCourse>,
}

/// Entrada persistida de um curso. O caminho é a única informação durável.
#[derive(Debug, Serialize, Deserialize)]
struct StoredCourse {
    id: String,
    path: PathBuf,
}

/// Formato anterior (versão 1): uma única raiz de biblioteca.
///
/// A raiz antiga vira um único curso com a identidade histórica
/// ([`LEGACY_LIBRARY_ID`]), para que as mídias, varreduras e o progresso de estudo já
/// gravados continuem associados ao mesmo `library_id`.
#[derive(Debug, Deserialize)]
struct LegacyConfig {
    version: u32,
    library_root: PathBuf,
}

/// Carrega e revalida os cursos persistidos.
///
/// Devolve lista vazia quando a configuração está ausente, corrompida ou em versão
/// desconhecida, e descarta cada curso cuja pasta não passa mais na validação (removida,
/// inacessível ou transformada em link). A leitura nunca falha de forma fatal nem cria
/// nada no sistema de arquivos.
pub fn load_courses(config_dir: &Path) -> Vec<CourseEntry> {
    let Ok(raw) = fs::read(config_dir.join(CONFIG_FILE_NAME)) else {
        return Vec::new();
    };

    let stored: Vec<StoredCourse> = if let Ok(config) = serde_json::from_slice::<StoredConfig>(&raw)
    {
        if config.version != CONFIG_VERSION {
            return Vec::new();
        }
        config.courses
    } else if let Ok(legacy) = serde_json::from_slice::<LegacyConfig>(&raw) {
        if legacy.version != 1 {
            return Vec::new();
        }
        let Some(root) = CourseRoot::resolve(&legacy.library_root).ok() else {
            return Vec::new();
        };
        return vec![CourseEntry {
            id: LEGACY_LIBRARY_ID.to_owned(),
            root,
        }];
    } else {
        return Vec::new();
    };

    let mut courses: Vec<CourseEntry> = Vec::new();
    for entry in stored {
        let Ok(root) = CourseRoot::resolve(&entry.path) else {
            continue;
        };
        if courses.iter().any(|course| course.id == entry.id) {
            continue;
        }
        courses.push(CourseEntry { id: entry.id, root });
    }
    courses
}

/// Grava a lista canonicalizada no diretório de configuração.
///
/// Se `config_dir` estiver dentro de algum curso — uma configuração errada — a gravação é
/// recusada **antes** de qualquer criação, para que nada seja criado dentro das pastas de
/// curso. O diretório de configuração é criado fora das raízes, e o arquivo é escrito em
/// um temporário e depois renomeado sobre o destino. No Windows, o destino é removido
/// imediatamente antes da troca, deixando uma pequena janela não atômica.
pub fn store_courses(config_dir: &Path, courses: &[CourseEntry]) -> Result<(), LibraryError> {
    if config_dir_is_inside(config_dir, courses) {
        return Err(LibraryError::WriteFailed);
    }

    fs::create_dir_all(config_dir).map_err(|_| LibraryError::WriteFailed)?;

    let payload = serde_json::to_vec(&StoredConfig {
        version: CONFIG_VERSION,
        courses: courses
            .iter()
            .map(|course| StoredCourse {
                id: course.id.clone(),
                path: course.root.as_path().to_path_buf(),
            })
            .collect(),
    })
    .map_err(|_| LibraryError::WriteFailed)?;

    write_atomically(&config_dir.join(CONFIG_FILE_NAME), &payload)
}

/// Indica se o diretório de configuração está dentro de algum curso registrado.
///
/// Compara formas canônicas do ancestral existente mais próximo, então funciona mesmo
/// quando o diretório de configuração ainda não existe. A comparação é contra as raízes
/// já canonicalizadas, evitando falsos negativos por diferença de prefixo (por exemplo o
/// prefixo verbatim do Windows).
fn config_dir_is_inside(config_dir: &Path, courses: &[CourseEntry]) -> bool {
    let mut probe = config_dir;
    let resolved = loop {
        if probe.exists() {
            break fs::canonicalize(probe).ok();
        }
        match probe.parent() {
            Some(parent) => probe = parent,
            None => break None,
        }
    };
    courses.iter().any(|course| {
        resolved.as_ref().map_or_else(
            || config_dir.starts_with(course.root.as_path()),
            |resolved| resolved.starts_with(course.root.as_path()),
        )
    })
}

/// Escreve `bytes` em `dest` de forma serializada.
///
/// O temporário é criado com `create_new` e depois trocado por `rename`. O cadeado
/// [`STORE_LOCK`] garante que gravações concorrentes não se intercalem. No Windows, o
/// destino é removido imediatamente antes de `rename`, deixando uma janela sem o arquivo.
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    let directory = dest.parent().ok_or(LibraryError::WriteFailed)?;
    let _guard = STORE_LOCK.lock().map_err(|_| LibraryError::WriteFailed)?;

    let temporary = create_temporary(directory, bytes)?;
    if replace_file(&temporary, dest).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(LibraryError::WriteFailed);
    }

    Ok(())
}

/// Cria um temporário **novo** com o conteúdo, sem seguir link nem reaproveitar arquivo
/// existente.
///
/// Cada tentativa abre com `create_new`, que recusa qualquer caminho já existente — um
/// arquivo comum, um symlink ou uma junction. Assim um temporário pré-existente nunca é
/// sobrescrito nem usado como destino da escrita; a gravação simplesmente tenta o próximo
/// nome. O conteúdo é sincronizado antes da troca.
fn create_temporary(directory: &Path, bytes: &[u8]) -> Result<PathBuf, LibraryError> {
    for attempt in 0..MAX_TEMP_ATTEMPTS {
        let candidate = temporary_candidate(directory, attempt);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if file
                    .write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .is_err()
                {
                    let _ = fs::remove_file(&candidate);
                    return Err(LibraryError::WriteFailed);
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(LibraryError::WriteFailed),
        }
    }
    Err(LibraryError::WriteFailed)
}

/// Nome candidato de temporário dentro do diretório de configuração.
///
/// O primeiro mantém o formato histórico `.{CONFIG_FILE_NAME}.{pid}.tmp`; as tentativas
/// seguintes acrescentam um contador, para não colidir com um temporário deixado por uma
/// execução anterior ou por outra gravação.
fn temporary_candidate(directory: &Path, attempt: u32) -> PathBuf {
    if attempt == 0 {
        directory.join(format!(".{CONFIG_FILE_NAME}.{}.tmp", std::process::id()))
    } else {
        directory.join(format!(
            ".{CONFIG_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            attempt
        ))
    }
}

/// Substitui o destino usando apenas APIs padrão.
///
/// Fora do Windows, `fs::rename` troca o nome no mesmo sistema de arquivos e substitui o
/// destino. No Windows, a API padrão não substitui um destino existente: removê-lo
/// imediatamente antes de `rename` deixa uma janela não atômica em que o destino pode não
/// existir.
fn replace_file(source: &Path, dest: &Path) -> Result<(), LibraryError> {
    #[cfg(windows)]
    {
        match fs::remove_file(dest) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LibraryError::WriteFailed),
        }
        if fs::rename(source, dest).is_err() {
            return Err(LibraryError::WriteFailed);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fs::rename(source, dest).map_err(|_| LibraryError::WriteFailed)
}
