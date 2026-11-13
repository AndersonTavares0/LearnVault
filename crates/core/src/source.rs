//! Fontes de curso somente leitura.
//!
//! Uma fonte expõe uma árvore de arquivos sem revelar se ela vive no disco local ou em um
//! servidor remoto. A varredura usa a mesma política de nomes e a mesma classificação da
//! biblioteca local, então um curso remoto produz o mesmo [`ScanOutcome`] que um curso
//! local com os mesmos arquivos.
//!
//! O contrato é estritamente de leitura: `list` e `read_range` são as únicas operações.
//! Nenhuma implementação pode criar, alterar, renomear, mover ou remover itens da fonte.

use std::{
    collections::VecDeque,
    future::Future,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    pin::Pin,
};

use crate::error::Error;
use crate::paths::{KeyMode, relative_path_key, resolve_within};
use crate::scanner::{ScanOutcome, ScannedFile, classify, is_ignored_directory};

/// Profundidade máxima de diretórios percorridos em uma fonte.
///
/// Protege a varredura de árvores patológicas (inclusive remotas) sem limitar cursos
/// reais, que raramente passam de poucos níveis.
pub const MAX_SOURCE_DEPTH: usize = 32;

/// Quantidade máxima de arquivos que uma varredura aceita de uma fonte.
pub const MAX_SOURCE_FILES: usize = 200_000;

/// Maior bloco que [`CourseSource::read_range`] devolve de uma vez.
pub const MAX_SOURCE_READ: usize = 8 * 1024 * 1024;

/// Item de uma listagem de diretório de uma fonte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    /// Caminho relativo à raiz da fonte, com separadores `/` e a caixa original.
    pub path: String,
    /// Verdadeiro quando o item é um diretório.
    pub is_dir: bool,
    /// Tamanho em bytes. Zero para diretórios e quando a fonte não informa.
    pub size: u64,
    /// Modificação em segundos desde a época Unix. `0` significa desconhecido.
    pub mtime: i64,
}

/// Falha sanitizada de uma fonte.
///
/// As variantes não carregam URLs, caminhos locais nem credenciais: o adaptador HTTP
/// escolhe a mensagem apresentada ao cliente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    /// A fonte não pôde ser alcançada.
    #[error("a fonte não está acessível")]
    Unreachable,
    /// A fonte recusou a autenticação ou a autorização.
    #[error("a fonte recusou o acesso")]
    Denied,
    /// O item pedido não existe na fonte.
    #[error("o item não existe na fonte")]
    NotFound,
    /// A fonte devolveu uma resposta que não pôde ser interpretada.
    #[error("a resposta da fonte é inválida")]
    InvalidResponse,
    /// A fonte excedeu um limite de segurança da varredura.
    #[error("a fonte excedeu o limite permitido")]
    Limit,
}

/// Resultado de uma operação de fonte.
pub type SourceResult<T> = Result<T, SourceError>;

/// Árvore de curso somente leitura.
///
/// Os métodos devolvem futuros porque fontes remotas fazem rede. O trait continua
/// utilizável como objeto (`&dyn CourseSource`) para que a varredura não conheça o
/// transporte.
pub trait CourseSource: Send + Sync {
    /// Lista os itens diretamente dentro de `directory`.
    ///
    /// A raiz é a string vazia. Cada item devolve o caminho relativo completo, nunca um
    /// caminho absoluto ou fora da raiz.
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>>;

    /// Lê até `length` bytes de `path` a partir de `offset`.
    fn read_range<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: usize,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<u8>>> + Send + 'a>>;
}

/// Fonte local, confinada à raiz informada.
#[derive(Debug, Clone)]
pub struct LocalSource {
    root: PathBuf,
}

impl LocalSource {
    /// Cria uma fonte local enraizada em `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Raiz da fonte.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn directory_path(&self, directory: &str) -> SourceResult<PathBuf> {
        if directory.is_empty() {
            return Ok(self.root.clone());
        }
        resolve_within(&self.root, directory).map_err(|_| SourceError::NotFound)
    }

    fn file_path(&self, path: &str) -> SourceResult<PathBuf> {
        if path.is_empty() {
            return Err(SourceError::NotFound);
        }
        resolve_within(&self.root, path).map_err(|_| SourceError::NotFound)
    }
}

impl CourseSource for LocalSource {
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>> {
        Box::pin(async move {
            let path = self.directory_path(directory)?;
            // A raiz nunca é seguida quando é um link, a mesma regra da varredura local.
            if directory.is_empty() {
                let metadata =
                    std::fs::symlink_metadata(&path).map_err(|_| SourceError::NotFound)?;
                if metadata.file_type().is_symlink() {
                    return Ok(Vec::new());
                }
                if !metadata.is_dir() {
                    return Err(SourceError::NotFound);
                }
            }

            let read_dir = std::fs::read_dir(&path).map_err(|_| SourceError::Denied)?;
            let mut entries = Vec::new();
            for entry in read_dir {
                let entry = entry.map_err(|_| SourceError::Denied)?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let file_type = entry.file_type().map_err(|_| SourceError::Denied)?;
                // Links não são seguidos nem reportados, como na varredura local.
                if file_type.is_symlink() {
                    continue;
                }
                let relative = if directory.is_empty() {
                    name
                } else {
                    format!("{directory}/{name}")
                };
                if file_type.is_dir() {
                    entries.push(SourceEntry {
                        path: relative,
                        is_dir: true,
                        size: 0,
                        mtime: 0,
                    });
                    continue;
                }
                if !file_type.is_file() {
                    continue;
                }
                let metadata = entry.metadata().map_err(|_| SourceError::Denied)?;
                entries.push(SourceEntry {
                    path: relative,
                    is_dir: false,
                    size: metadata.len(),
                    mtime: crate::scanner::mtime_seconds(&metadata),
                });
            }
            Ok(entries)
        })
    }

    fn read_range<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: usize,
    ) -> Pin<Box<dyn Future<Output = SourceResult<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            if length > MAX_SOURCE_READ {
                return Err(SourceError::Limit);
            }
            let path = self.file_path(path)?;
            let mut file = std::fs::File::open(path).map_err(|_| SourceError::NotFound)?;
            file.seek(SeekFrom::Start(offset))
                .map_err(|_| SourceError::InvalidResponse)?;
            let mut buffer = vec![0; length];
            let mut filled = 0;
            while filled < length {
                match file.read(&mut buffer[filled..]) {
                    Ok(0) => break,
                    Ok(read) => filled += read,
                    Err(_) => return Err(SourceError::InvalidResponse),
                }
            }
            buffer.truncate(filled);
            Ok(buffer)
        })
    }
}

/// Percorre `source` e devolve o mesmo resultado que [`crate::scanner::scan`] para uma
/// árvore local equivalente.
///
/// Diretórios ocultos, diretórios temporários e links seguem a política da biblioteca.
/// Uma falha na raiz interrompe a varredura; uma falha em uma subárvore é registrada em
/// [`ScanOutcome::inaccessible`] e a varredura continua.
pub async fn scan_source(source: &dyn CourseSource) -> SourceResult<ScanOutcome> {
    let mode = KeyMode::for_host();
    let mut files = Vec::new();
    let mut inaccessible = Vec::new();
    let mut directories = VecDeque::from([String::new()]);

    while let Some(directory) = directories.pop_front() {
        let entries = match source.list(&directory).await {
            Ok(entries) => entries,
            Err(error) if directory.is_empty() => return Err(error),
            Err(_) => {
                inaccessible.push(if directory.is_empty() {
                    ".".to_owned()
                } else {
                    directory
                });
                continue;
            }
        };

        for entry in entries {
            let Some(name) = entry.path.rsplit('/').next().map(str::to_owned) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            if entry.is_dir {
                if name.starts_with('.') || is_ignored_directory(&name) {
                    continue;
                }
                if entry.path.split('/').count() > MAX_SOURCE_DEPTH {
                    inaccessible.push(entry.path);
                    continue;
                }
                directories.push_back(entry.path);
                continue;
            }
            if name.starts_with('.') {
                continue;
            }
            let Ok(key) = relative_path_key(&entry.path, mode) else {
                inaccessible.push(entry.path);
                continue;
            };
            files.push(ScannedFile {
                relative_path: entry.path,
                relative_path_key: key,
                kind: classify(&name, entry.size),
                size: entry.size,
                mtime: entry.mtime,
            });
            if files.len() > MAX_SOURCE_FILES {
                return Err(SourceError::Limit);
            }
        }
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(ScanOutcome {
        files,
        inaccessible,
    })
}

/// Converte uma falha de fonte em um erro do domínio, para que o histórico de varredura
/// registre a tentativa sem esconder a causa.
impl From<SourceError> for Error {
    fn from(error: SourceError) -> Self {
        Error::SourceUnavailable { source: error }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        future::Future,
        task::{Context, Poll, Waker},
    };

    use super::{CourseSource, LocalSource, SourceEntry, SourceError, scan_source};
    use crate::scanner::{MediaKind, scan};

    /// Executa um futuro que nunca suspende de verdade (testes locais e fixtures).
    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    struct FixtureSource {
        entries: Vec<SourceEntry>,
    }

    impl CourseSource for FixtureSource {
        fn list<'a>(
            &'a self,
            directory: &'a str,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = super::SourceResult<Vec<SourceEntry>>> + Send + 'a>,
        > {
            Box::pin(async move {
                Ok(self
                    .entries
                    .iter()
                    .filter(|entry| parent(&entry.path) == directory)
                    .cloned()
                    .collect())
            })
        }

        fn read_range<'a>(
            &'a self,
            _path: &'a str,
            _offset: u64,
            _length: usize,
        ) -> std::pin::Pin<Box<dyn Future<Output = super::SourceResult<Vec<u8>>> + Send + 'a>>
        {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    fn parent(path: &str) -> &str {
        path.rsplit_once('/').map_or("", |(parent, _)| parent)
    }

    fn entry(path: &str, is_dir: bool, size: u64) -> SourceEntry {
        SourceEntry {
            path: path.to_owned(),
            is_dir,
            size,
            mtime: 0,
        }
    }

    #[test]
    fn local_source_matches_the_local_scanner() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir_all(root.join("Curso/Modulo 1")).unwrap();
        fs::write(root.join("Curso/Modulo 1/aula.mp4"), b"video").unwrap();
        fs::write(root.join("Curso/Modulo 1/aula.srt"), b"legenda").unwrap();
        fs::write(root.join("Curso/Modulo 1/apoio.pdf"), b"material").unwrap();
        fs::create_dir_all(root.join("temp_download")).unwrap();
        fs::write(root.join("temp_download/rascunho.mp4"), b"x").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), b"x").unwrap();

        let outcome = block_on(scan_source(&LocalSource::new(root))).unwrap();
        let expected = scan(root).unwrap();

        let paths = |outcome: &crate::scanner::ScanOutcome| {
            outcome
                .files
                .iter()
                .map(|file| (file.relative_path.clone(), file.kind, file.size))
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(&outcome), paths(&expected));
        assert!(outcome.inaccessible.is_empty());
    }

    #[test]
    fn a_remote_like_source_produces_the_same_tree_and_classification() {
        let source = FixtureSource {
            entries: vec![
                entry("Curso", true, 0),
                entry("Curso/Modulo 1", true, 0),
                entry("Curso/Modulo 1/aula.mp4", false, 10),
                entry("Curso/Modulo 1/aula.srt", false, 4),
                entry("Curso/Modulo 1/apoio.pdf", false, 3),
                entry("Curso/.oculto.mp4", false, 1),
                entry("temp", true, 0),
                entry("temp/ignorado.mp4", false, 1),
            ],
        };

        let outcome = block_on(scan_source(&source)).unwrap();
        let files = outcome
            .files
            .iter()
            .map(|file| (file.relative_path.as_str(), file.kind))
            .collect::<Vec<_>>();

        assert_eq!(
            files,
            vec![
                ("Curso/Modulo 1/apoio.pdf", MediaKind::Material),
                ("Curso/Modulo 1/aula.mp4", MediaKind::Media),
                ("Curso/Modulo 1/aula.srt", MediaKind::Subtitle),
            ]
        );
        assert!(outcome.inaccessible.is_empty());
    }

    #[test]
    fn local_source_reads_a_range_and_refuses_paths_outside_the_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::write(root.join("aula.mp4"), b"0123456789").unwrap();
        let source = LocalSource::new(root);

        let bytes = block_on(source.read_range("aula.mp4", 2, 4)).unwrap();
        assert_eq!(bytes, b"2345");
        let tail = block_on(source.read_range("aula.mp4", 8, 10)).unwrap();
        assert_eq!(tail, b"89");
        assert_eq!(
            block_on(source.read_range("../fora.mp4", 0, 1)).unwrap_err(),
            SourceError::NotFound
        );
        assert_eq!(
            block_on(source.read_range("aula.mp4", 0, super::MAX_SOURCE_READ + 1)).unwrap_err(),
            SourceError::Limit
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_entries_are_not_listed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("biblioteca");
        fs::create_dir_all(&root).unwrap();
        let outside = directory.path().join("fora");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("segredo.mp4"), b"x").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("atalho")).unwrap();

        let entries = block_on(LocalSource::new(&root).list("")).unwrap();
        assert!(entries.is_empty());

        let outcome = block_on(scan_source(&LocalSource::new(root))).unwrap();
        assert!(outcome.files.is_empty());
    }
}
