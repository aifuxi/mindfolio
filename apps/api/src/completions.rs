use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    FromRow,
    types::chrono::{DateTime, NaiveDate, Utc},
};
use utoipa::ToSchema;

use crate::{ApiError, ErrorCode, auth::PrivateState};

type CompletionFailure = (StatusCode, Json<ApiError>);

pub(super) fn routes() -> Router<PrivateState> {
    Router::new()
        .route("/task-completions", get(list))
        .route("/task-completions/days", get(days))
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    date: Option<String>,
    page: Option<i64>,
}

#[derive(Deserialize)]
pub(super) struct DaysQuery {
    page: Option<i64>,
}

#[derive(FromRow)]
struct CompletionRow {
    id: i64,
    task_id: i64,
    task_title: String,
    project_name: Option<String>,
    completed_at: DateTime<Utc>,
    business_date: NaiveDate,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CompletionResponse {
    id: String,
    task_id: String,
    task_title: String,
    project_name: Option<String>,
    completed_at: String,
    business_date: String,
}

impl From<CompletionRow> for CompletionResponse {
    fn from(row: CompletionRow) -> Self {
        Self {
            id: row.id.to_string(),
            task_id: row.task_id.to_string(),
            task_title: row.task_title,
            project_name: row.project_name,
            completed_at: row.completed_at.to_rfc3339(),
            business_date: row.business_date.to_string(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(super) struct CompletionPage {
    items: Vec<CompletionResponse>,
    page: i64,
    has_more: bool,
}

#[derive(FromRow)]
struct DayRow {
    business_date: NaiveDate,
    completed_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DaySummary {
    business_date: String,
    completed_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DayPage {
    items: Vec<DaySummary>,
    page: i64,
    has_more: bool,
}

fn failure(status: StatusCode, code: ErrorCode, message: &str) -> CompletionFailure {
    (
        status,
        Json(ApiError {
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    )
}

fn invalid() -> CompletionFailure {
    failure(
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidRequest,
        "查询参数无效",
    )
}

fn unavailable() -> CompletionFailure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::DatabaseUnavailable,
        "数据库不可用",
    )
}

fn page(value: Option<i64>) -> Result<i64, CompletionFailure> {
    let page = value.unwrap_or(1);
    (1..=100_000)
        .contains(&page)
        .then_some(page)
        .ok_or_else(invalid)
}

fn date(value: Option<String>) -> Result<Option<NaiveDate>, CompletionFailure> {
    value
        .map(|raw| {
            if raw.len() != 10 || raw.as_bytes()[4] != b'-' || raw.as_bytes()[7] != b'-' {
                return Err(invalid());
            }
            NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|_| invalid())
        })
        .transpose()
}

#[utoipa::path(
    get, path = "/task-completions", operation_id = "list_task_completions", security(("admin_session" = [])),
    description = "完成事实按完成时刻倒序；date 按 Asia/Shanghai 业务日期筛选。",
    params(("date" = Option<String>, Query), ("page" = Option<i64>, Query)),
    responses((status = 200, body = CompletionPage), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn list(
    State(state): State<PrivateState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<CompletionPage>, CompletionFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = page(query.page)?;
    let date = date(query.date)?;
    let mut rows: Vec<CompletionRow> = sqlx::query_as(
        "SELECT id, task_id, task_title, project_name, completed_at, \
         (completed_at AT TIME ZONE 'Asia/Shanghai')::date AS business_date \
         FROM task_completion \
         WHERE ($1::date IS NULL OR (completed_at AT TIME ZONE 'Asia/Shanghai')::date = $1) \
         ORDER BY completed_at DESC, id DESC LIMIT 51 OFFSET $2",
    )
    .bind(date)
    .bind((page - 1) * 50)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(Json(CompletionPage {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        has_more,
    }))
}

#[utoipa::path(
    get, path = "/task-completions/days", operation_id = "list_task_completion_days", security(("admin_session" = [])),
    description = "按 Asia/Shanghai 业务日期汇总完成次数，每页最多 50 天。",
    params(("page" = Option<i64>, Query)),
    responses((status = 200, body = DayPage), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn days(
    State(state): State<PrivateState>,
    query: Result<Query<DaysQuery>, QueryRejection>,
) -> Result<Json<DayPage>, CompletionFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = page(query.page)?;
    let mut rows: Vec<DayRow> = sqlx::query_as(
        "SELECT (completed_at AT TIME ZONE 'Asia/Shanghai')::date AS business_date, \
         count(*) AS completed_count FROM task_completion \
         GROUP BY business_date ORDER BY business_date DESC LIMIT 51 OFFSET $1",
    )
    .bind((page - 1) * 50)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(Json(DayPage {
        items: rows
            .into_iter()
            .map(|row| DaySummary {
                business_date: row.business_date.to_string(),
                completed_count: row.completed_count,
            })
            .collect(),
        page,
        has_more,
    }))
}
