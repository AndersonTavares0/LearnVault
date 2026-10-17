//! Registro de cursos do servidor e os endpoints administrativos que o mantêm.
//!
//! Cada curso é uma pasta externa registrada individualmente e permanece **somente
//! leitura**: registrar não copia, move nem altera nada dentro dela. O registro vive em um
//! arquivo de configuração versionado ao lado do banco, nunca no banco nem dentro dos
//! cursos — a raiz física é configuração da plataforma, não dado do domínio.
//!
//! A identidade de cada curso é derivada do caminho canônico ([`course_id_from_path`]),
//! de modo que reimportar a mesma pasta reencontra as mídias e o progresso de estudo já
//! persistidos. O caminho nunca cruza a fronteira HTTP: a interface conhece apenas `id` e
//! `title`.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{CourseRoot, LEGACY_LIBRARY_ID, course_id_from_path, course_title};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
};

/// Nome do arquivo de registro dentro do diretório de dados do servidor.
pub const REGISTRY_FILE_NAME: &str = "courses.json";

/// Versão do formato do arquivo de registro.
pub const REGISTRY_VERSION: u32 = 1;

/// Curso registrado: identidade estável, título apresentado e raiz já validada.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredCourse {
    /// Identidade estável usada como `library_id` no banco.
    pub id: String,
    /// Título apresentado nos cards.
    pub title: String,
    /// Raiz canônica validada do curso. Nunca serializada para o cliente.
    pub root: CourseRoot,
}

/// Formato serializado do arquivo de registro.
#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    version: u32,
    courses: Vec<StoredCourse>,
}

/// Entrada persistida de um curso. O caminho é a única informação durável.
#[derive(Debug, Serialize, Deserialize)]
struct StoredCourse {
    id: String,
    path: PathBuf,
}

/// Caminho do arquivo de registro para um banco. Fica ao lado do banco, no diretório de
/// dados; um banco sem diretório pai usa o diretório atual.
pub(crate) fn registry_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(REGISTRY_FILE_NAME)
}

/// Carrega o registro persistido.
///
/// Arquivo ausente, corrompido ou em versão desconhecida resulta em registro vazio; cada
/// entrada é revalidada com [`CourseRoot::resolve`] e uma raiz removida, inacessível ou
/// substituída por link é descartada em silêncio (a lista persistida não é reescrita por
/// uma leitura). Nenhum caminho aparece em mensagens.
pub(crate) fn load_registry(path: &Path) -> Vec<RegisteredCourse> {
    let Ok(raw) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(stored) = serde_json::from_slice::<RegistryFile>(&raw) else {
        return Vec::new();
    };
    if stored.version != REGISTRY_VERSION {
        return Vec::new();
    }
    let mut courses: Vec<RegisteredCourse> = Vec::new();
    for entry in stored.courses {
        let Ok(root) = CourseRoot::resolve(&entry.path) else {
            continue;
        };
        if courses.iter().any(|course| course.id == entry.id) {
            continue;
        }
        let title = course_title(root.as_path());
        courses.push(RegisteredCourse {
            id: entry.id,
            title,
            root,
        });
    }
    courses
}

/// Curso semente do modo legado: a raiz configurada por `--library-root`/`LIBRARY_DIR`
/// usa a identidade histórica para preservar mídias, varreduras e progresso já gravados.
pub(crate) fn legacy_seed(root: &Path) -> Option<RegisteredCourse> {
    let root = CourseRoot::resolve(root).ok()?;
    Some(RegisteredCourse {
        id: LEGACY_LIBRARY_ID.to_owned(),
        title: course_title(root.as_path()),
        root,
    })
}

/// Grava o registro de forma serializada e atômica.
///
/// O temporário é criado exclusivamente (`create_new`) e trocado por `rename`; no Windows,
/// como a API padrão não substitui um destino existente, o destino é removido imediatamente
/// antes da troca. A serialização é responsabilidade do cadeado de escrita do estado.
pub(crate) fn store_registry(
    path: &Path,
    courses: &[RegisteredCourse],
) -> Result<(), RegistryError> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).map_err(|_| RegistryError::WriteFailed)?;

    let stored = RegistryFile {
        version: REGISTRY_VERSION,
        courses: courses
            .iter()
            .map(|course| StoredCourse {
                id: course.id.clone(),
                path: course.root.as_path().to_path_buf(),
            })
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| RegistryError::WriteFailed)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("courses");
    let temporary = directory.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&payload)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(RegistryError::WriteFailed);
    }

    if replace_file(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(RegistryError::WriteFailed);
    }
    Ok(())
}

/// Troca o temporário pelo destino usando apenas APIs padrão.
fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        match fs::remove_file(dest) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::rename(source, dest)
}

/// Falha estável do registro. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryError {
    /// O registro não pôde ser gravado com segurança.
    WriteFailed,
}

/// Resposta de um curso registrado, sem o caminho.
#[derive(Serialize)]
struct CourseResponse {
    id: String,
    title: String,
}

impl From<&RegisteredCourse> for CourseResponse {
    fn from(course: &RegisteredCourse) -> Self {
        Self {
            id: course.id.clone(),
            title: course.title.clone(),
        }
    }
}

/// Corpo do pedido de importação: o caminho da pasta do curso no servidor.
#[derive(Deserialize)]
pub(crate) struct ImportRequest {
    path: String,
}

#[derive(Serialize)]
struct ImportResponse {
    course: CourseResponse,
}

#[derive(Serialize)]
struct RemoveResponse {
    removed: bool,
}

/// `POST /api/admin/courses` — registra uma pasta como curso.
///
/// A pasta precisa existir, ser um diretório legível e não ser um link/junction. A
/// validação e a gravação rodam fora das threads Tokio; o curso passa a existir no
/// catálogo após a próxima indexação. Uma pasta já registrada é recusada com `400`.
pub(crate) async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(crate::StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let courses = Arc::clone(&state.courses);
    let courses_file = state.courses_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        import_course(&courses, &courses_file, &request.path)
    })
    .await;

    match outcome {
        Ok(Ok(course)) => private_no_store(
            Json(ImportResponse {
                course: (&course).into(),
            })
            .into_response(),
        ),
        Ok(Err(ImportFailure::InvalidPath)) => {
            json_error(StatusCode::BAD_REQUEST, "invalid_course_path")
        }
        Ok(Err(ImportFailure::Duplicate)) => {
            json_error(StatusCode::BAD_REQUEST, "course_already_registered")
        }
        Ok(Err(ImportFailure::WriteFailed)) | Err(_) => {
            internal_error("course_registry_unavailable")
        }
    }
}

/// `DELETE /api/admin/courses/{id}` — remove um curso do registro.
///
/// Remove apenas o registro: os arquivos permanecem intactos e o catálogo do curso deixa
/// de ser servido. Remover um curso desconhecido responde `200` com `removed: false`.
pub(crate) async fn remove(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(crate::StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let courses = Arc::clone(&state.courses);
    let courses_file = state.courses_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        remove_course(&courses, &courses_file, &course_id)
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(RemoveResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("course_registry_unavailable"),
    }
}

#[derive(Debug)]
enum ImportFailure {
    InvalidPath,
    Duplicate,
    WriteFailed,
}

/// Valida a pasta, registra o curso e persiste a lista sob o cadeado de escrita.
///
/// A lista em memória só é trocada depois de a gravação concluir: uma falha de escrita não
/// deixa o estado divergente do arquivo.
fn import_course(
    courses: &std::sync::RwLock<Vec<RegisteredCourse>>,
    courses_file: &Path,
    path: &str,
) -> Result<RegisteredCourse, ImportFailure> {
    let candidate = Path::new(path);
    let root = CourseRoot::resolve(candidate).map_err(|_| ImportFailure::InvalidPath)?;
    let course = RegisteredCourse {
        id: course_id_from_path(root.as_path()),
        title: course_title(root.as_path()),
        root,
    };

    let mut guard = courses.write().map_err(|_| ImportFailure::WriteFailed)?;
    if guard.iter().any(|registered| registered.id == course.id) {
        return Err(ImportFailure::Duplicate);
    }
    let mut next = guard.clone();
    next.push(course.clone());
    store_registry(courses_file, &next).map_err(|_| ImportFailure::WriteFailed)?;
    *guard = next;
    Ok(course)
}

/// Remove o curso do registro e persiste a lista restante.
fn remove_course(
    courses: &std::sync::RwLock<Vec<RegisteredCourse>>,
    courses_file: &Path,
    course_id: &str,
) -> Result<bool, RegistryError> {
    let mut guard = courses.write().map_err(|_| RegistryError::WriteFailed)?;
    let next: Vec<RegisteredCourse> = guard
        .iter()
        .filter(|course| course.id != course_id)
        .cloned()
        .collect();
    let removed = next.len() != guard.len();
    if removed {
        store_registry(courses_file, &next)?;
        *guard = next;
    }
    Ok(removed)
}

/// Erro JSON estável, sem política de cache adicional.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

#[cfg(test)]
mod tests {
    use std::sync::RwLock;

    use tempfile::tempdir;

    use super::{
        ImportFailure, RegisteredCourse, RegistryFile, import_course, load_registry, registry_path,
        remove_course, store_registry,
    };

    fn write_registry_file(path: &std::path::Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn registry_round_trips_courses_and_drops_invalid_roots() {
        let temp = tempdir().unwrap();
        let data = temp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let database = data.join("learnvault.sqlite");
        let registry = registry_path(&database);
        assert_eq!(registry, data.join("courses.json"));

        let course_a = temp.path().join("01 - Física");
        let course_b = temp.path().join("Matemática");
        std::fs::create_dir(&course_a).unwrap();
        std::fs::create_dir(&course_b).unwrap();

        let courses = RwLock::new(Vec::new());
        let first = import_course(&courses, &registry, course_a.to_str().unwrap()).unwrap();
        let second = import_course(&courses, &registry, course_b.to_str().unwrap()).unwrap();
        assert_eq!(first.title, "Física");
        assert_eq!(second.title, "Matemática");

        // A pasta removida antes da recarga é descartada em silêncio.
        std::fs::remove_dir(&course_b).unwrap();
        let loaded = load_registry(&registry);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, first.id);
    }

    #[test]
    fn import_rejects_duplicates_and_invalid_paths() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let courses = RwLock::new(Vec::new());

        let missing = import_course(&courses, &registry, "nao-existe");
        assert!(matches!(missing, Err(ImportFailure::InvalidPath)));

        import_course(&courses, &registry, course.to_str().unwrap()).unwrap();
        let duplicate = import_course(&courses, &registry, course.to_str().unwrap());
        assert!(matches!(duplicate, Err(ImportFailure::Duplicate)));
    }

    #[test]
    fn remove_is_idempotent_and_persists_the_remaining_courses() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let courses = RwLock::new(Vec::new());
        let registered = import_course(&courses, &registry, course.to_str().unwrap()).unwrap();

        assert!(remove_course(&courses, &registry, &registered.id).unwrap());
        assert!(!remove_course(&courses, &registry, &registered.id).unwrap());
        assert!(load_registry(&registry).is_empty());
        assert!(courses.read().unwrap().is_empty());
    }

    #[test]
    fn load_discards_unknown_versions_and_corrupt_files() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");

        write_registry_file(&registry, "not json");
        assert!(load_registry(&registry).is_empty());

        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let unknown = serde_json::json!({
            "version": 99,
            "courses": [{ "id": "course-1", "path": course }]
        });
        write_registry_file(&registry, &unknown.to_string());
        assert!(load_registry(&registry).is_empty());
    }

    #[test]
    fn store_writes_a_versioned_file_without_temporaries_left_behind() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("nested").join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let root = learnvault_core::CourseRoot::resolve(&course).unwrap();
        let courses = vec![RegisteredCourse {
            id: "course-1".to_owned(),
            title: "Curso".to_owned(),
            root,
        }];

        store_registry(&registry, &courses).unwrap();
        let raw = std::fs::read_to_string(&registry).unwrap();
        let parsed: RegistryFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.courses.len(), 1);

        let leftovers: Vec<_> = std::fs::read_dir(registry.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
