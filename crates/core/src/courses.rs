//! Cursos registrados: raiz validada, identidade estável e título derivado.
//!
//! A raiz física de cada curso é configuração da plataforma, não dado do domínio. Este
//! módulo concentra o que o servidor e o desktop precisam compartilhar: a validação segura
//! da pasta escolhida, a identidade estável usada como `library_id` no banco e o título
//! apresentado nos cards.
//!
//! A pasta de um curso é externa e **somente leitura**: resolver uma raiz nunca cria,
//! move nem altera nada dentro dela.

use std::fmt;
use std::path::{Path, PathBuf};

/// Raiz de um curso já validada e canonicalizada.
///
/// O caminho é privado de propósito: ele existe apenas para o Rust e nunca é serializado
/// para o webview. O `Debug` também oculta o caminho, para que um registro de diagnóstico
/// não vaze a localização do curso.
#[derive(Clone, PartialEq, Eq)]
pub struct CourseRoot(PathBuf);

impl CourseRoot {
    /// Valida e canonicaliza um caminho candidato a raiz de curso.
    ///
    /// O caminho precisa existir, ser um diretório, não ser um link simbólico, junction
    /// ou ponto de reparse no próprio topo, e ser legível (`read_dir`). O resultado é a
    /// forma canônica — a única forma persistida. Nenhum arquivo é criado ou modificado.
    pub fn resolve(path: &Path) -> Result<Self, CourseRootError> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CourseRootError::Missing);
            }
            Err(_) => return Err(CourseRootError::NotReadable),
        };

        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(CourseRootError::Link);
        }
        if !metadata.is_dir() {
            return Err(CourseRootError::NotADirectory);
        }

        let canonical = std::fs::canonicalize(path).map_err(|_| CourseRootError::NotReadable)?;
        std::fs::read_dir(&canonical).map_err(|_| CourseRootError::NotReadable)?;

        Ok(Self(canonical))
    }

    /// Caminho canônico da raiz, somente para uso interno do Rust.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Debug for CourseRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CourseRoot(<oculto>)")
    }
}

/// Falha estável da validação de uma raiz de curso. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourseRootError {
    /// O caminho escolhido não existe.
    Missing,
    /// O caminho escolhido não é um diretório.
    NotADirectory,
    /// O caminho escolhido não pôde ser lido (canonicalização ou listagem).
    NotReadable,
    /// O caminho escolhido é um link, junction ou ponto de reparse.
    Link,
}

impl fmt::Display for CourseRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            CourseRootError::Missing => "a pasta do curso não existe",
            CourseRootError::NotADirectory => "a pasta do curso não é um diretório",
            CourseRootError::NotReadable => "a pasta do curso não pode ser lida",
            CourseRootError::Link => "a pasta do curso não pode ser um link",
        })
    }
}

impl std::error::Error for CourseRootError {}

/// Identificador legado da primeira biblioteca.
///
/// A raiz configurada pela implantação antiga (e a raiz persistida pelo desktop antes dos
/// cursos individuais) continua associada a esta identidade, para que as mídias, varreduras
/// e o progresso de estudo já gravados não se percam quando o curso é recarregado.
pub const LEGACY_LIBRARY_ID: &str = "library-1";

/// Identidade estável de um curso derivado do caminho canônico.
///
/// A identidade precisa ser estável entre execuções para que reimportar a mesma pasta
/// reencontre as mídias e o progresso de estudo já persistidos, e precisa ser opaca para
/// que o caminho do sistema de arquivos nunca cruze a fronteira do webview.
pub fn course_id_from_path(path: &Path) -> String {
    let digest = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        path.as_os_str().as_encoded_bytes(),
    );
    format!("course-{digest}")
}

/// Título apresentado de um curso: o nome da pasta, sem prefixos de ordenação.
///
/// Uma pasta sem nome utilizável (por exemplo a raiz de um volume) recebe um título
/// neutro, para que o card nunca fique vazio.
pub fn course_title(root: &Path) -> String {
    let fallback = "Curso";
    let Some(name) = root.file_name().and_then(|name| name.to_str()) else {
        return fallback.to_owned();
    };
    let cleaned = crate::catalog::clean_title(name);
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned
    }
}

/// Verdadeiro quando o metadado marca um ponto de reparse (junction ou symlink) no Windows.
#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Fora do Windows, junctions e pontos de reparse não existem; links já são detectados.
#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CourseRoot, CourseRootError, course_id_from_path, course_title};
    use tempfile::tempdir;

    #[test]
    fn resolve_rejects_missing_files_and_directories_pass() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("arquivo.txt");
        std::fs::write(&file, "x").unwrap();

        assert_eq!(
            CourseRoot::resolve(&temp.path().join("nao-existe")),
            Err(CourseRootError::Missing)
        );
        assert_eq!(
            CourseRoot::resolve(&file),
            Err(CourseRootError::NotADirectory)
        );

        let root = CourseRoot::resolve(temp.path()).unwrap();
        assert_eq!(
            root.as_path(),
            temp.path().canonicalize().unwrap().as_path()
        );
    }

    #[test]
    fn id_is_stable_and_opaque() {
        let temp = tempdir().unwrap();
        let other = tempdir().unwrap();
        let first = course_id_from_path(temp.path());
        let second = course_id_from_path(temp.path());

        assert_eq!(first, second);
        assert_ne!(first, course_id_from_path(other.path()));
        assert!(first.starts_with("course-"));
        assert!(!first.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn title_is_the_cleaned_folder_name() {
        let temp = tempdir().unwrap();
        let course = temp.path().join("01 - Física");
        std::fs::create_dir(&course).unwrap();

        assert_eq!(course_title(&course), "Física");
    }

    #[test]
    fn title_falls_back_when_the_folder_has_no_usable_name() {
        assert_eq!(course_title(Path::new("/")), "Curso");
        assert_eq!(course_title(Path::new("42")), "Curso");
    }
}
