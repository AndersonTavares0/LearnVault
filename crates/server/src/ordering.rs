//! Ordenação manual de módulos e aulas do catálogo (issue #239).
//!
//! A biblioteca de cursos é **somente leitura**: a ordem escolhida pelo usuário vive em um
//! arquivo versionado ao lado do banco (o mesmo diretório do registro de cursos e das
//! capas), nunca dentro das pastas de curso. Cada curso tem a sua própria preferência,
//! identificada pela identidade opaca do curso; as entradas apontam apenas para os caminhos
//! relativos já catalogados.
//!
//! A ordem automática (natural, determinística) continua sendo o padrão: a preferência é
//! aplicada por sobreposição em [`learnvault_core::apply_order`] antes de a árvore ser
//! servida. Por isso remover a preferência restaura a ordem automática sem reescrever nada.

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    CatalogOrder, build_catalog, shape_course_catalog, sibling_paths, validate_siblings,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    courses::replace_file,
};

/// Nome do arquivo de ordenação dentro do diretório de dados do servidor.
pub const ORDER_FILE_NAME: &str = "catalog-order.json";

/// Versão do formato do arquivo de ordenação.
pub const ORDER_VERSION: u32 = 1;

/// Preferências de ordenação de todos os cursos, por identidade opaca do curso.
pub type CourseOrders = BTreeMap<String, CatalogOrder>;

/// Formato serializado do arquivo de ordenação.
#[derive(Debug, Serialize, Deserialize)]
struct OrderFile {
    version: u32,
    /// Curso -> (caminho do pai -> ordem desejada dos filhos).
    courses: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// Caminho do arquivo de ordenação para um banco. Fica ao lado do banco, no diretório de
/// dados, como o registro de cursos e as capas.
pub(crate) fn order_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(ORDER_FILE_NAME)
}

/// Carrega as preferências persistidas.
///
/// Arquivo ausente, corrompido ou em versão desconhecida resulta em nenhuma preferência
/// (ordem automática); nenhuma mensagem carrega caminho.
pub(crate) fn load_orders(path: &Path) -> CourseOrders {
    let Ok(raw) = std::fs::read(path) else {
        return CourseOrders::new();
    };
    let Ok(stored) = serde_json::from_slice::<OrderFile>(&raw) else {
        return CourseOrders::new();
    };
    if stored.version != ORDER_VERSION {
        return CourseOrders::new();
    }
    stored
        .courses
        .into_iter()
        .map(|(course_id, parents)| (course_id, CatalogOrder::from_map(parents)))
        .filter(|(_, order)| !order.is_empty())
        .collect()
}

/// Grava as preferências de forma serializada e atômica.
///
/// O temporário é criado exclusivamente (`create_new`) e trocado por `rename` via
/// [`replace_file`]. A serialização é responsabilidade do cadeado de escrita do estado.
pub(crate) fn store_orders(path: &Path, orders: &CourseOrders) -> Result<(), OrderError> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(directory).map_err(|_| OrderError::WriteFailed)?;

    let stored = OrderFile {
        version: ORDER_VERSION,
        courses: orders
            .iter()
            .filter(|(_, order)| !order.is_empty())
            .map(|(course_id, order)| (course_id.clone(), order.as_map().clone()))
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| OrderError::WriteFailed)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("catalog-order");
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
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    if replace_file(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    Ok(())
}

/// Falha estável da persistência da ordenação. Nunca carrega caminho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrderError {
    /// A ordenação não pôde ser gravada com segurança.
    WriteFailed,
}

/// Falha da camada de ordenação, antes da tradução para HTTP.
enum OrderFailure {
    /// O caminho do pai não existe no catálogo do curso.
    UnknownParent,
    /// A lista não é uma permutação exata dos irmãos.
    InvalidOrder,
    /// A leitura do catálogo falhou.
    Core,
    /// A gravação da preferência falhou.
    WriteFailed,
}

/// Corpo do pedido de ordenação: o caminho do pai e a ordem desejada dos seus filhos.
#[derive(Debug, Deserialize)]
pub(crate) struct OrderRequest {
    /// Caminho relativo do nó pai; vazio para a lista de topo (módulos do curso).
    parent: String,
    /// Caminhos dos filhos, na ordem desejada. Precisa ser uma permutação exata.
    children: Vec<String>,
}

/// Consulta da restauração: um pai específico ou todos quando ausente.
#[derive(Debug, Deserialize)]
pub(crate) struct ResetQuery {
    parent: Option<String>,
}

#[derive(Serialize)]
struct OrderResponse {
    parent: String,
    children: Vec<String>,
}

#[derive(Serialize)]
struct ResetResponse {
    removed: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

/// `PUT /api/admin/courses/{id}/order` — define a ordem dos irmãos de um nó do curso.
///
/// Somente administrador com credenciais definidas e mesma origem. O curso precisa estar
/// registrado; um caminho de pai desconhecido, uma lista que não seja uma permutação exata
/// dos irmãos ou um curso desconhecido respondem `404`/`422` sem revelar caminhos físicos.
pub(crate) async fn put_order(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<OrderRequest>,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
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
    if !course_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let database = state.database.clone().lock_owned().await;
    let orders = state.orders.clone();
    let orders_file = state.orders_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        set_course_order(
            &database,
            &orders,
            &orders_file,
            &course_id,
            &request.parent,
            &request.children,
        )
    })
    .await;

    match outcome {
        Ok(Ok(response)) => private_no_store(Json(response).into_response()),
        Ok(Err(OrderFailure::UnknownParent)) | Ok(Err(OrderFailure::InvalidOrder)) => {
            json_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_order")
        }
        Ok(Err(OrderFailure::WriteFailed)) | Err(_) => internal_error("course_order_unavailable"),
        Ok(Err(OrderFailure::Core)) => internal_error("catalog unavailable"),
    }
}

/// `DELETE /api/admin/courses/{id}/order` — restaura a ordem automática.
///
/// Sem `parent`, remove toda a preferência do curso; com `parent`, remove a preferência
/// daquele nó. É idempotente: restaurar o que já estava automático responde
/// `removed: false`. As mesmas exigências de autenticação e mesma origem do `PUT`.
pub(crate) async fn reset_order(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    Query(query): Query<ResetQuery>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
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
    if !course_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let orders = state.orders.clone();
    let orders_file = state.orders_file.clone();
    let parent = query.parent;
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        clear_course_order(&orders, &orders_file, &course_id, parent.as_deref())
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(ResetResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("course_order_unavailable"),
    }
}

/// Valida a lista contra os irmãos do pai e persiste a preferência.
fn set_course_order(
    database: &learnvault_core::Database,
    orders: &std::sync::RwLock<CourseOrders>,
    orders_file: &Path,
    course_id: &str,
    parent: &str,
    children: &[String],
) -> Result<OrderResponse, OrderFailure> {
    let catalog = build_catalog(database, course_id).map_err(|_| OrderFailure::Core)?;
    let (shaped, _) = shape_course_catalog(catalog);
    let siblings = sibling_paths(&shaped, parent).ok_or(OrderFailure::UnknownParent)?;
    if siblings.is_empty() || !validate_siblings(&siblings, children) {
        return Err(OrderFailure::InvalidOrder);
    }

    let mut guard = orders.write().map_err(|_| OrderFailure::WriteFailed)?;
    let mut next = guard.clone();
    next.entry(course_id.to_owned())
        .or_default()
        .set_children(parent, children.to_vec());
    store_orders(orders_file, &next).map_err(|_| OrderFailure::WriteFailed)?;
    *guard = next;

    Ok(OrderResponse {
        parent: parent.to_owned(),
        children: children.to_vec(),
    })
}

/// Remove a preferência de um pai (ou de todo o curso) e persiste a mudança.
fn clear_course_order(
    orders: &std::sync::RwLock<CourseOrders>,
    orders_file: &Path,
    course_id: &str,
    parent: Option<&str>,
) -> Result<bool, OrderError> {
    let mut guard = orders.write().map_err(|_| OrderError::WriteFailed)?;
    let mut next = guard.clone();
    let removed = match parent {
        Some(parent) => next
            .get_mut(course_id)
            .is_some_and(|order| order.remove(parent)),
        None => next.remove(course_id).is_some(),
    };
    if !removed {
        return Ok(false);
    }
    if next.get(course_id).is_some_and(CatalogOrder::is_empty) {
        next.remove(course_id);
    }
    store_orders(orders_file, &next)?;
    *guard = next;
    Ok(true)
}

/// Verdadeiro quando a identidade corresponde a um curso registrado.
fn course_registered(state: &AppState, course_id: &str) -> bool {
    state
        .courses_snapshot()
        .iter()
        .any(|course| course.id == course_id)
}

/// Erro JSON estável, sem política de cache adicional.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}
