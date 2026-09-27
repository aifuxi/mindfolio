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
use sqlx::{FromRow, PgPool, types::chrono::NaiveDate};
use utoipa::ToSchema;

use crate::{ApiError, ErrorCode, auth::PrivateState, tasks::TodayClock};

pub(crate) type HabitFailure = (StatusCode, Json<ApiError>);

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new()
        .route("/habits", get(list).post(create))
        .route("/habits/{id}", get(detail).patch(update).delete(delete))
        .route("/habits/{id}/pause", post(pause))
        .route("/habits/{id}/resume", post(resume))
}

#[derive(FromRow)]
pub(crate) struct HabitRow {
    pub id: i64,
    pub created_on: NaiveDate,
    pub deleted_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    pub version: i64,
    pub name: String,
    pub cadence: String,
    pub weekly_target: Option<i16>,
    pub paused: bool,
}

pub(crate) const FIELDS: &str = "h.id, h.created_on, h.deleted_at, h.version, s.name, s.cadence, s.weekly_target, EXISTS(SELECT 1 FROM habit_pause p WHERE p.habit_id = h.id AND p.start_on <= $1 AND (p.end_on IS NULL OR p.end_on > $1)) AS paused";

#[derive(Serialize, ToSchema)]
pub(crate) struct HabitResponse {
    id: String,
    created_on: String,
    deleted_at: Option<String>,
    version: i64,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
    paused: bool,
}

impl From<HabitRow> for HabitResponse {
    fn from(row: HabitRow) -> Self {
        Self {
            id: row.id.to_string(),
            created_on: row.created_on.to_string(),
            deleted_at: row.deleted_at.map(|time| time.to_rfc3339()),
            version: row.version,
            name: row.name,
            cadence: row.cadence,
            weekly_target: row.weekly_target,
            paused: row.paused,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct HabitPage {
    items: Vec<HabitResponse>,
    page: i64,
    has_more: bool,
}

#[derive(Deserialize)]
pub(crate) struct ListQuery {
    page: Option<i64>,
    include_deleted: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct HabitInput {
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct HabitUpdate {
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
    expected_version: i64,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct VersionRequest {
    expected_version: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SettingResponse {
    effective_on: String,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct PauseResponse {
    start_on: String,
    end_on: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct HabitDetail {
    habit: HabitResponse,
    settings: Vec<SettingResponse>,
    pauses: Vec<PauseResponse>,
}

#[derive(FromRow)]
struct SettingRow {
    effective_on: NaiveDate,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
}

#[derive(FromRow)]
struct PauseRow {
    start_on: NaiveDate,
    end_on: Option<NaiveDate>,
}

pub(crate) fn failure(status: StatusCode, code: ErrorCode, message: &str) -> HabitFailure {
    (
        status,
        Json(ApiError {
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    )
}
pub(crate) fn invalid() -> HabitFailure {
    failure(
        StatusCode::BAD_REQUEST,
        ErrorCode::InvalidRequest,
        "习惯请求无效",
    )
}
pub(crate) fn missing() -> HabitFailure {
    failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "习惯不存在")
}
pub(crate) fn conflict() -> HabitFailure {
    failure(
        StatusCode::CONFLICT,
        ErrorCode::VersionConflict,
        "习惯已变更，请刷新后重试",
    )
}
pub(crate) fn unavailable() -> HabitFailure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::DatabaseUnavailable,
        "数据库不可用",
    )
}
pub(crate) fn parse_id(raw: &str) -> Result<i64, HabitFailure> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(missing)
}

fn validate(input: &HabitInput) -> Result<String, HabitFailure> {
    let name = input.name.trim();
    if name.is_empty()
        || name.chars().count() > 120
        || name.chars().any(char::is_control)
        || !matches!(
            (input.cadence.as_str(), input.weekly_target),
            ("daily", None) | ("weekly", Some(1..=7))
        )
    {
        return Err(invalid());
    }
    Ok(name.into())
}

async fn row(pool: &PgPool, id: i64, today: NaiveDate) -> Result<HabitRow, HabitFailure> {
    let statement = format!(
        "SELECT {FIELDS} FROM habit h JOIN LATERAL (SELECT name, cadence, weekly_target FROM habit_setting WHERE habit_id = h.id AND effective_on <= $1 ORDER BY effective_on DESC LIMIT 1) s ON true WHERE h.id = $2"
    );
    sqlx::query_as(&statement)
        .bind(today)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)
}

async fn history(pool: &PgPool, id: i64, today: NaiveDate) -> Result<HabitDetail, HabitFailure> {
    let habit = row(pool, id, today).await?.into();
    let settings: Vec<SettingRow> = sqlx::query_as("SELECT effective_on, name, cadence, weekly_target FROM habit_setting WHERE habit_id = $1 ORDER BY effective_on, id").bind(id).fetch_all(pool).await.map_err(|_| unavailable())?;
    let pauses: Vec<PauseRow> = sqlx::query_as(
        "SELECT start_on, end_on FROM habit_pause WHERE habit_id = $1 ORDER BY start_on, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|_| unavailable())?;
    Ok(HabitDetail {
        habit,
        settings: settings
            .into_iter()
            .map(|r| SettingResponse {
                effective_on: r.effective_on.to_string(),
                name: r.name,
                cadence: r.cadence,
                weekly_target: r.weekly_target,
            })
            .collect(),
        pauses: pauses
            .into_iter()
            .map(|r| PauseResponse {
                start_on: r.start_on.to_string(),
                end_on: r.end_on.map(|d| d.to_string()),
            })
            .collect(),
    })
}

#[utoipa::path(get, path = "/habits", operation_id = "list_habits", security(("admin_session" = [])), params(("page" = Option<i64>, Query), ("include_deleted" = Option<bool>, Query)), responses((status = 200, body = HabitPage), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn list(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<HabitPage>, HabitFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid());
    }
    let today = clock.business_date();
    let statement = format!(
        "SELECT {FIELDS} FROM habit h JOIN LATERAL (SELECT name, cadence, weekly_target FROM habit_setting WHERE habit_id = h.id AND effective_on <= $1 ORDER BY effective_on DESC LIMIT 1) s ON true WHERE ($2 OR h.deleted_at IS NULL) ORDER BY h.created_at DESC, h.id DESC LIMIT 51 OFFSET $3"
    );
    let mut rows: Vec<HabitRow> = sqlx::query_as(&statement)
        .bind(today)
        .bind(query.include_deleted.unwrap_or(false))
        .bind((page - 1) * 50)
        .fetch_all(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(Json(HabitPage {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        has_more,
    }))
}

#[utoipa::path(post, path = "/habits", operation_id = "create_habit", security(("admin_session" = [])), params(("x-csrf-token" = String, Header)), request_body = HabitInput, responses((status = 201, body = HabitResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn create(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    input: Result<Json<HabitInput>, JsonRejection>,
) -> Result<(StatusCode, Json<HabitResponse>), HabitFailure> {
    let Json(input) = input.map_err(|_| invalid())?;
    let name = validate(&input)?;
    let today = clock.business_date();
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    let id: i64 = sqlx::query_scalar("INSERT INTO habit (created_on) VALUES ($1) RETURNING id")
        .bind(today)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence, weekly_target) VALUES ($1, $2, $3, $4, $5)").bind(id).bind(today).bind(name).bind(input.cadence).bind(input.weekly_target).execute(&mut *tx).await.map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok((
        StatusCode::CREATED,
        Json(row(&state.pool, id, today).await?.into()),
    ))
}

#[utoipa::path(get, path = "/habits/{id}", operation_id = "get_habit", security(("admin_session" = [])), params(("id" = String, Path)), responses((status = 200, body = HabitDetail), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn detail(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
) -> Result<Json<HabitDetail>, HabitFailure> {
    Ok(Json(
        history(&state.pool, parse_id(&id)?, clock.business_date()).await?,
    ))
}

async fn locked_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: i64,
    expected: i64,
) -> Result<(), HabitFailure> {
    if expected < 1 {
        return Err(invalid());
    }
    let value: Option<(
        i64,
        Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
    )> = sqlx::query_as("SELECT version, deleted_at FROM habit WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| unavailable())?;
    match value {
        None => Err(missing()),
        Some((_, deleted)) if deleted.is_some() => Err(missing()),
        Some((version, _)) if version != expected => Err(conflict()),
        _ => Ok(()),
    }
}

#[utoipa::path(patch, path = "/habits/{id}", operation_id = "update_habit", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = HabitUpdate, responses((status = 200, body = HabitResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn update(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
    input: Result<Json<HabitUpdate>, JsonRejection>,
) -> Result<Json<HabitResponse>, HabitFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid())?;
    let name = validate(&HabitInput {
        name: input.name.clone(),
        cadence: input.cadence.clone(),
        weekly_target: input.weekly_target,
    })?;
    let today = clock.business_date();
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    locked_version(&mut tx, id, input.expected_version).await?;
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence, weekly_target) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (habit_id, effective_on) DO UPDATE SET name = EXCLUDED.name, cadence = EXCLUDED.cadence, weekly_target = EXCLUDED.weekly_target").bind(id).bind(today).bind(name).bind(input.cadence).bind(input.weekly_target).execute(&mut *tx).await.map_err(|_| unavailable())?;
    sqlx::query("UPDATE habit SET version = version + 1, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(row(&state.pool, id, today).await?.into()))
}

async fn transition(
    state: &PrivateState,
    clock: &TodayClock,
    id: String,
    input: Result<Json<VersionRequest>, JsonRejection>,
    action: &str,
) -> Result<Json<HabitResponse>, HabitFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid())?;
    let today = clock.business_date();
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    locked_version(&mut tx, id, input.expected_version).await?;
    match action {
        "pause" => {
            let open: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM habit_pause WHERE habit_id = $1 AND end_on IS NULL)",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
            if open {
                return Err(conflict());
            }
            sqlx::query("INSERT INTO habit_pause (habit_id, start_on) VALUES ($1, $2)")
                .bind(id)
                .bind(today)
                .execute(&mut *tx)
                .await
                .map_err(|_| unavailable())?;
        }
        "resume" => {
            let count = sqlx::query("UPDATE habit_pause SET end_on = $1 WHERE habit_id = $2 AND end_on IS NULL AND start_on <= $1").bind(today).bind(id).execute(&mut *tx).await.map_err(|_| unavailable())?.rows_affected();
            if count != 1 {
                return Err(conflict());
            }
        }
        "delete" => {
            sqlx::query("UPDATE habit SET deleted_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|_| unavailable())?;
        }
        _ => unreachable!(),
    }
    sqlx::query("UPDATE habit SET version = version + 1, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(row(&state.pool, id, today).await?.into()))
}

#[utoipa::path(post, path = "/habits/{id}/pause", operation_id = "pause_habit", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = HabitResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn pause(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<HabitResponse>, HabitFailure> {
    transition(&state, &clock, id, input, "pause").await
}

#[utoipa::path(post, path = "/habits/{id}/resume", operation_id = "resume_habit", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = HabitResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn resume(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<HabitResponse>, HabitFailure> {
    transition(&state, &clock, id, input, "resume").await
}

#[utoipa::path(delete, path = "/habits/{id}", operation_id = "delete_habit", security(("admin_session" = [])), params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = VersionRequest, responses((status = 200, body = HabitResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn delete(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
    input: Result<Json<VersionRequest>, JsonRejection>,
) -> Result<Json<HabitResponse>, HabitFailure> {
    transition(&state, &clock, id, input, "delete").await
}
