//! Endpoints HTTP do catálogo, do rescan administrativo e do registro de cursos.

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use learnvault_core::{Availability, CatalogFile, CatalogNode, MediaKind, NodeRole, RescanSummary};
use serde::Serialize;

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    courses::RegisteredCourse,
    ordering::CourseOrders,
};

/// Limite de corpo da requisição de ordenação, acima do limite padrão do roteador (8 KiB).
///
/// Reordenar um módulo com muitas aulas (ou um curso com muitos módulos) gera um corpo com
/// um caminho relativo por irmão; o limite generoso evita um `413` surpresa numa biblioteca
/// grande, sem afetar as demais rotas.
pub(crate) const ORDER_BODY_LIMIT: usize = 512 * 1024;

enum RescanFailure {
    Busy,
    Failed,
    NoCourses,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Serialize)]
struct CatalogResponse {
    complete: bool,
    loose_files: Vec<CatalogFileResponse>,
    /// Árvore recursiva canônica, o contrato preferido pela SPA.
    nodes: Vec<CatalogNodeResponse>,
    /// Cursos, na ordem de importação. Cada nó de topo tem papel `course` e a
    /// identidade opaca do curso em `course_id`.
    categories: Vec<CatalogNodeResponse>,
}

#[derive(Serialize, Clone)]
struct CatalogNodeResponse {
    title: String,
    path: String,
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    course_id: Option<String>,
    /// Segundos de modificação da capa personalizada do curso, quando existir. É a
    /// versão usada pela SPA para invalidar a URL (`?v=`); ausente quando não há capa.
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_version: Option<i64>,
    files: Vec<CatalogFileResponse>,
    loose_files: Vec<CatalogFileResponse>,
    children: Vec<CatalogNodeResponse>,
}

#[derive(Serialize, Clone)]
struct CatalogFileResponse {
    media_id: String,
    name: String,
    path: String,
    size: u64,
    mtime_seconds: i64,
    kind: &'static str,
    availability: &'static str,
    unverified: bool,
}

#[derive(Serialize)]
struct RescanResponse {
    scan_id: i64,
    complete: bool,
    courses: usize,
    failed_courses: usize,
    inserted: usize,
    updated: usize,
    marked_missing: usize,
    preserved: usize,
    /// Quantidade de chaves relativas ambíguas recusadas nesta varredura.
    ///
    /// É apenas a contagem: as chaves em si são caminhos relativos e nunca cruzam a API.
    ambiguous_count: usize,
}

pub(crate) async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => {}
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    }

    // Evita segurar o mutex do banco só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let courses = state.courses_snapshot();
    let orders = state.orders_snapshot();
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let covers = crate::covers::covers_dir(&state.config.database_path);
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        build_course_catalog(&database, &courses, &orders, &covers)
    })
    .await
    {
        Ok(Ok(catalog)) => private_no_store(Json(catalog).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("catalog unavailable"),
    }
}

/// Agrega o catálogo dos cursos registrados.
///
/// Cada curso vira um nó de topo com papel `course`, identidade própria e a subárvore
/// derivada dos dados persistidos. Os arquivos soltos da pasta do curso ficam no próprio
/// nó; não existe mais uma raiz global de arquivos soltos. Um curso sem varredura aparece
/// vazio, e a resposta é marcada como incompleta se qualquer curso estiver incompleto.
///
/// Cada subárvore recebe a preferência de ordenação manual do curso ([`CourseOrders`]),
/// aplicada por sobreposição sobre a ordem natural; cursos continuam na ordem automática.
/// A mesma lista de nós de topo alimenta `nodes` (árvore recursiva canônica) e `categories`
/// (lista de cursos de topo): nenhum dos dois contratos perde aula, arquivo ou identidade.
fn build_course_catalog(
    database: &learnvault_core::Database,
    courses: &[RegisteredCourse],
    orders: &CourseOrders,
    covers_dir: &Path,
) -> Result<CatalogResponse, learnvault_core::Error> {
    let empty = learnvault_core::CatalogOrder::new();
    let mut complete = true;
    let mut nodes = Vec::with_capacity(courses.len());
    let mut courses: Vec<_> = courses.iter().collect();
    courses.sort_by(|a, b| {
        let a_name = a
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&a.title);
        let b_name = b
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&b.title);
        learnvault_core::natural_cmp(a_name, b_name).then_with(|| a.id.cmp(&b.id))
    });
    for course in courses {
        let catalog = learnvault_core::build_catalog(database, &course.id)?;
        complete = complete && catalog.complete;
        let (mut children, loose_files) = learnvault_core::shape_course_catalog(catalog);
        learnvault_core::apply_order(&mut children, orders.get(&course.id).unwrap_or(&empty));
        nodes.push(CatalogNodeResponse {
            title: course.title.clone(),
            path: String::new(),
            role: "course",
            course_id: Some(course.id.clone()),
            cover_version: crate::covers::cover_version(covers_dir, &course.id),
            files: Vec::new(),
            loose_files: loose_files.into_iter().map(Into::into).collect(),
            children: children.into_iter().map(Into::into).collect(),
        });
    }
    Ok(CatalogResponse {
        complete,
        loose_files: Vec::new(),
        // O mesmo nó de topo de curso alimenta os dois contratos: `nodes` é a árvore
        // canônica e `categories` é a lista de cursos de topo do contrato HTTP.
        nodes: nodes.clone(),
        categories: nodes,
    })
}

pub(crate) async fn rescan(State(state): State<AppState>, headers: HeaderMap) -> Response {
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
    // Evita segurar locks só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let Ok(local_lock) = state.rescan_lock.clone().try_lock_owned() else {
        return status_private_no_store(StatusCode::CONFLICT);
    };

    let database = state.database.clone();
    let courses = state.courses_snapshot();
    let covers_dir = crate::covers::covers_dir(&state.config.database_path);
    let lock_path = rescan_lock_path(&state.config.database_path);
    let mut database = database.lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    match tokio::task::spawn_blocking(move || {
        // Os guards e o permit pertencem deliberadamente à tarefa blocking. Tokio continua
        // executando-a depois que o JoinHandle é descartado, então um timeout HTTP não libera
        // a capacidade nem os locks enquanto o scan ainda usa o banco.
        let _blocking_task = blocking_task;
        let _local_lock = local_lock;
        let _file_lock = try_rescan_file_lock(&lock_path)?;
        if courses.is_empty() {
            return Err(RescanFailure::NoCourses);
        }
        let mut aggregate = RescanSummary {
            complete: true,
            ..RescanSummary::default()
        };
        let mut failed_courses = 0usize;
        for course in &courses {
            // A raiz é revalidada antes de cada varredura: um curso removido, inacessível
            // ou substituído por link/junction conta como falha sem seguir o alvo.
            let Ok(root) = learnvault_core::CourseRoot::resolve(course.root.as_path()) else {
                failed_courses += 1;
                aggregate.complete = false;
                continue;
            };
            match learnvault_core::rescan(&mut database, &course.id, root.as_path()) {
                Ok(summary) => {
                    let _ = crate::covers::auto_detect_course_cover(
                        &covers_dir,
                        &course.id,
                        root.as_path(),
                    );
                    aggregate.scan_id = aggregate.scan_id.max(summary.scan_id);
                    aggregate.complete = aggregate.complete && summary.complete;
                    aggregate.inserted += summary.inserted;
                    aggregate.updated += summary.updated;
                    aggregate.marked_missing += summary.marked_missing;
                    aggregate.preserved += summary.preserved;
                    aggregate.ambiguous.extend(summary.ambiguous);
                }
                Err(_) => {
                    failed_courses += 1;
                    aggregate.complete = false;
                }
            }
        }
        // A resposta é a projeção sanitizada do resumo agregado: só as contagens cruzam a
        // API, inclusive a quantidade de chaves ambíguas, nunca as chaves em si.
        let mut response = RescanResponse::from(aggregate);
        response.courses = courses.len();
        response.failed_courses = failed_courses;
        Ok(response)
    })
    .await
    {
        Ok(Ok(summary)) => private_no_store(Json(summary).into_response()),
        Ok(Err(RescanFailure::Busy)) => status_private_no_store(StatusCode::CONFLICT),
        Ok(Err(RescanFailure::NoCourses)) => private_no_store(
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "no_courses_registered",
                }),
            )
                .into_response(),
        ),
        Ok(Err(RescanFailure::Failed)) | Err(_) => internal_error("rescan failed"),
    }
}

pub(crate) async fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<learnvault_core::AuthSession>, StateError> {
    let Some(token) = request_token(headers) else {
        return Ok(None);
    };
    state.session_user(hash_session_token(&token)).await
}

pub(crate) fn internal_error(message: &'static str) -> Response {
    private_no_store(
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: message }),
        )
            .into_response(),
    )
}

pub(crate) fn service_unavailable() -> Response {
    private_no_store(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "server busy",
            }),
        )
            .into_response(),
    )
}

pub(crate) fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

pub(crate) fn status_private_no_store(status: StatusCode) -> Response {
    private_no_store(status.into_response())
}

pub(crate) fn credentials_required() -> Response {
    private_no_store(
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "credentials_change_required",
            }),
        )
            .into_response(),
    )
}

pub(crate) fn csrf_failed() -> Response {
    private_no_store(
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "csrf_failed",
            }),
        )
            .into_response(),
    )
}

fn rescan_lock_path(database_path: &Path) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(".rescan.lock");
    path.into()
}

fn try_rescan_file_lock(path: &Path) -> Result<File, RescanFailure> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| RescanFailure::Failed)?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => RescanFailure::Busy,
        std::fs::TryLockError::Error(_) => RescanFailure::Failed,
    })?;
    Ok(file)
}

impl From<CatalogNode> for CatalogNodeResponse {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
            course_id: None,
            cover_version: None,
            files: node.files.into_iter().map(Into::into).collect(),
            loose_files: node.loose_files.into_iter().map(Into::into).collect(),
            children: node.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CatalogFile> for CatalogFileResponse {
    fn from(file: CatalogFile) -> Self {
        Self {
            media_id: file.media_id,
            name: file.name,
            path: file.path,
            size: file.size,
            mtime_seconds: file.mtime_seconds,
            kind: media_kind(file.kind),
            availability: availability(file.availability),
            unverified: file.unverified,
        }
    }
}

impl From<RescanSummary> for RescanResponse {
    fn from(summary: RescanSummary) -> Self {
        Self {
            scan_id: summary.scan_id,
            complete: summary.complete,
            // Um resumo descreve um único curso; a agregação de vários cursos substitui as
            // duas contagens depois da conversão, como o DTO do desktop.
            courses: 1,
            failed_courses: 0,
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            // Nunca serializa as chaves: a resposta expõe só a contagem, como o desktop.
            ambiguous_count: summary.ambiguous.len(),
        }
    }
}

const fn node_role(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Category => "category",
        NodeRole::Course => "course",
        NodeRole::Module => "module",
        NodeRole::Lesson => "lesson",
    }
}

const fn media_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Media => "media",
        MediaKind::Subtitle => "subtitle",
        MediaKind::Material => "material",
    }
}

const fn availability(value: Availability) -> &'static str {
    match value {
        Availability::Available => "available",
        Availability::Missing => "missing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_response_exposes_only_the_ambiguous_count() {
        // O resumo carrega caminhos relativos nas chaves ambíguas; a resposta nunca pode
        // serializá-los, expondo apenas a contagem (como o DTO do desktop).
        let summary = RescanSummary {
            ambiguous: vec!["Cursos secretos/aula.mp4".to_owned()],
            ..RescanSummary::default()
        };

        let json = serde_json::to_value(RescanResponse::from(summary)).unwrap();

        assert_eq!(json["ambiguous_count"], 1);
        assert!(
            json.get("ambiguous").is_none(),
            "a resposta não pode expor a lista de caminhos: {json}"
        );
        let rendered = json.to_string();
        assert!(
            !rendered.contains("Cursos secretos") && !rendered.contains("aula.mp4"),
            "nenhum caminho relativo pode cruzar a API: {rendered}"
        );
    }
}
