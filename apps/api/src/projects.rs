use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;

use crate::{ApiError, ErrorCode, auth::PrivateState};

type ProjectFailure = (StatusCode, Json<ApiError>);

pub(super) fn routes() -> Router<PrivateState> {
    Router::new()
        .route("/projects", get(list).post(create))
        .route("/projects/{id}", get(detail).patch(rename).delete(delete))
        .route("/projects/{id}/complete", post(complete))
        .route("/projects/{id}/archive", post(archive))
        .route("/projects/{id}/restore", post(restore))
}

#[derive(FromRow)]
struct ProjectRow {
    id: i64,
    name: String,
    version: i64,
    completed_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    archived_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProjectResponse {
    id: String,
    name: String,
    version: i64,
    completed_at: Option<String>,
    archived_at: Option<String>,
}

impl From<ProjectRow> for ProjectResponse {
    fn from(value: ProjectRow) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            version: value.version,
            completed_at: value.completed_at.map(|time| time.to_rfc3339()),
            archived_at: value.archived_at.map(|time| time.to_rfc3339()),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProjectPage {
    items: Vec<ProjectResponse>,
    page: i64,
    has_more: bool,
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    page: Option<i64>,
    include_archived: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct CreateProject {
    name: String,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct RenameProject {
    name: String,
    expected_version: i64,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct VersionRequest {
    expected_version: i64,
}

fn failure(status: StatusCode, code: ErrorCode, message: &str) -> ProjectFailure {
    (
        status,
        Json(ApiError {
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    )
}

fn invalid() -> ProjectFailure {
    failure(
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidRequest,
        "请求无效",
    )
}

fn unavailable() -> ProjectFailure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::DatabaseUnavailable,
        "数据库不可用",
    )
}

fn delete_lock_error(error: sqlx::Error) -> ProjectFailure {
    if error
        .as_database_error()
        .is_some_and(|error| matches!(error.code().as_deref(), Some("55P03" | "40P01")))
    {
        failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "项目正在变更，请刷新后重试",
        )
    } else {
        unavailable()
    }
}

fn valid_name(raw: &str) -> Result<String, ProjectFailure> {
    let name = raw.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(name.into())
}

fn parse_id(raw: &str) -> Result<i64, ProjectFailure> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "项目不存在"))
}

async fn conflict_or_missing(pool: &PgPool, id: i64) -> ProjectFailure {
    match sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM project WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
    {
        Ok(true) => failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "项目已变更，请刷新后重试",
        ),
        Ok(false) => failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "项目不存在"),
        Err(_) => unavailable(),
    }
}

const FIELDS: &str = "id, name, version, completed_at, archived_at";

#[utoipa::path(
    get, path = "/projects", security(("admin_session" = [])),
    params(("page" = Option<i64>, Query, description = "从 1 开始的页码"), ("include_archived" = Option<bool>, Query, description = "是否包含归档项目")),
    responses((status = 200, body = ProjectPage), (status = 401, body = ApiError), (status = 400, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn list(
    State(state): State<PrivateState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<ProjectPage>, ProjectFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid());
    }
    let offset = (page - 1) * 50;
    let statement = format!(
        "SELECT {FIELDS} FROM project WHERE ($1 OR archived_at IS NULL) ORDER BY created_at DESC, id DESC LIMIT 51 OFFSET $2"
    );
    let mut rows: Vec<ProjectRow> = sqlx::query_as(&statement)
        .bind(query.include_archived.unwrap_or(false))
        .bind(offset)
        .fetch_all(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(Json(ProjectPage {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        has_more,
    }))
}

#[utoipa::path(
    post, path = "/projects", security(("admin_session" = [])),
    params(("x-csrf-token" = String, Header)), request_body = CreateProject,
    responses((status = 201, body = ProjectResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn create(
    State(state): State<PrivateState>,
    input: Result<Json<CreateProject>, JsonRejection>,
) -> Result<(StatusCode, Json<ProjectResponse>), ProjectFailure> {
    let Json(input) = input.map_err(|_| invalid())?;
    let name = valid_name(&input.name)?;
    let statement = format!("INSERT INTO project (name) VALUES ($1) RETURNING {FIELDS}");
    let row: ProjectRow = sqlx::query_as(&statement)
        .bind(name)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

#[utoipa::path(
    get, path = "/projects/{id}", security(("admin_session" = [])),
    params(("id" = String, Path)),
    responses((status = 200, body = ProjectResponse), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn detail(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    let id = parse_id(&id)?;
    let statement = format!("SELECT {FIELDS} FROM project WHERE id = $1");
    let row: Option<ProjectRow> = sqlx::query_as(&statement)
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    row.map(|row| Json(row.into()))
        .ok_or_else(|| failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "项目不存在"))
}

#[utoipa::path(
    patch, path = "/projects/{id}", security(("admin_session" = [])),
    params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = RenameProject,
    responses((status = 200, body = ProjectResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn rename(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<RenameProject>, JsonRejection>,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid())?;
    if input.expected_version < 1 {
        return Err(invalid());
    }
    let name = valid_name(&input.name)?;
    let statement = format!(
        "UPDATE project SET name = $1, version = version + 1, updated_at = now() WHERE id = $2 AND version = $3 RETURNING {FIELDS}"
    );
    let row: Option<ProjectRow> = sqlx::query_as(&statement)
        .bind(name)
        .bind(id)
        .bind(input.expected_version)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    match row {
        Some(row) => Ok(Json(row.into())),
        None => Err(conflict_or_missing(&state.pool, id).await),
    }
}

async fn transition(
    state: &PrivateState,
    id: String,
    input: Result<Json<VersionRequest>, JsonRejection>,
    action: &str,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid())?;
    if input.expected_version < 1 {
        return Err(invalid());
    }
    let change = match action {
        "complete" => "completed_at = COALESCE(completed_at, now())",
        "archive" => "archived_at = now()",
        "restore" => "archived_at = NULL",
        _ => unreachable!(),
    };
    let condition = match action {
        "complete" => "completed_at IS NULL AND archived_at IS NULL",
        "archive" => "archived_at IS NULL",
        "restore" => "archived_at IS NOT NULL",
        _ => unreachable!(),
    };
    let statement = format!(
        "UPDATE project SET {change}, version = version + 1, updated_at = now() WHERE id = $1 AND version = $2 AND {condition} RETURNING {FIELDS}"
    );
    let row: Option<ProjectRow> = sqlx::query_as(&statement)
        .bind(id)
        .bind(input.expected_version)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    match row {
        Some(row) => Ok(Json(row.into())),
        None => Err(conflict_or_missing(&state.pool, id).await),
    }
}

#[utoipa::path(post, path = "/projects/{id}/complete", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = ProjectResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(super) async fn complete(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    transition(&state, id, input, "complete").await
}

#[utoipa::path(post, path = "/projects/{id}/archive", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = ProjectResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(super) async fn archive(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    transition(&state, id, input, "archive").await
}

#[utoipa::path(post, path = "/projects/{id}/restore", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = ProjectResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(super) async fn restore(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<ProjectResponse>, ProjectFailure> {
    transition(&state, id, input, "restore").await
}

#[utoipa::path(
    delete, path = "/projects/{id}", operation_id = "delete_project", security(("admin_session" = [])),
    description = "删除项目及所属任务和子任务；既有任务完成记录保留。",
    params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest,
    responses((status = 204), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn delete(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<StatusCode, ProjectFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid())?;
    if input.expected_version < 1 {
        return Err(invalid());
    }
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    let version: Option<i64> =
        sqlx::query_scalar("SELECT version FROM project WHERE id = $1 FOR UPDATE NOWAIT")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(delete_lock_error)?;
    let version =
        version.ok_or_else(|| failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "项目不存在"))?;
    if version != input.expected_version {
        return Err(failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "项目已变更，请刷新后重试",
        ));
    }
    let _: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM task WHERE project_id = $1 ORDER BY id FOR UPDATE NOWAIT",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(delete_lock_error)?;
    sqlx::query("DELETE FROM task WHERE project_id = $1 AND parent_id IS NOT NULL")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(delete_lock_error)?;
    sqlx::query("DELETE FROM task WHERE project_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(delete_lock_error)?;
    sqlx::query("DELETE FROM project WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(delete_lock_error)?;
    tx.commit().await.map_err(delete_lock_error)?;
    Ok(StatusCode::NO_CONTENT)
}
