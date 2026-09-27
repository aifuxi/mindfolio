use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, Response, header},
    routing::get,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Serialize, Serializer};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    ApiError,
    auth::PrivateState,
    habits::{HabitFailure, unavailable},
    tasks::TodayClock,
};

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new().route("/export", get(download))
}

fn id<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}
fn optional_id<S: Serializer>(value: &Option<i64>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&value.to_string()),
        None => serializer.serialize_none(),
    }
}
fn date<S: Serializer>(value: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}
fn optional_date<S: Serializer>(
    value: &Option<NaiveDate>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&value.to_string()),
        None => serializer.serialize_none(),
    }
}
fn instant<S: Serializer>(value: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_rfc3339())
}
fn optional_instant<S: Serializer>(
    value: &Option<DateTime<Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&value.to_rfc3339()),
        None => serializer.serialize_none(),
    }
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportProject {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    name: String,
    version: i64,
    #[serde(serialize_with = "optional_instant")]
    #[schema(value_type = Option<String>)]
    completed_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "optional_instant")]
    #[schema(value_type = Option<String>)]
    archived_at: Option<DateTime<Utc>>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportTask {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "optional_id")]
    #[schema(value_type = Option<String>)]
    parent_id: Option<i64>,
    #[serde(serialize_with = "optional_id")]
    #[schema(value_type = Option<String>)]
    project_id: Option<i64>,
    title: String,
    description: String,
    status: String,
    priority: Option<String>,
    #[serde(serialize_with = "optional_date")]
    #[schema(value_type = Option<String>)]
    planned_date: Option<NaiveDate>,
    #[serde(serialize_with = "optional_date")]
    #[schema(value_type = Option<String>)]
    due_date: Option<NaiveDate>,
    in_backlog: bool,
    version: i64,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportTag {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    name: String,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportTagLink {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    task_id: i64,
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    tag_id: i64,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportCompletion {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    task_id: i64,
    task_title: String,
    project_name: Option<String>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    completed_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportHabit {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "date")]
    #[schema(value_type = String)]
    created_on: NaiveDate,
    #[serde(serialize_with = "optional_instant")]
    #[schema(value_type = Option<String>)]
    deleted_at: Option<DateTime<Utc>>,
    version: i64,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportHabitSetting {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    habit_id: i64,
    #[serde(serialize_with = "date")]
    #[schema(value_type = String)]
    effective_on: NaiveDate,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportHabitPause {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    habit_id: i64,
    #[serde(serialize_with = "date")]
    #[schema(value_type = String)]
    start_on: NaiveDate,
    #[serde(serialize_with = "optional_date")]
    #[schema(value_type = Option<String>)]
    end_on: Option<NaiveDate>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportHabitCheckin {
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    id: i64,
    #[serde(serialize_with = "id")]
    #[schema(value_type = String)]
    habit_id: i64,
    #[serde(serialize_with = "date")]
    #[schema(value_type = String)]
    business_date: NaiveDate,
    completed: bool,
    note: String,
    version: i64,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, ToSchema)]
pub(crate) struct ExportJournal {
    #[serde(serialize_with = "date")]
    #[schema(value_type = String)]
    business_date: NaiveDate,
    body: String,
    version: i64,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    created_at: DateTime<Utc>,
    #[serde(serialize_with = "instant")]
    #[schema(value_type = String)]
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ExportData {
    format_version: u32,
    generated_at: String,
    projects: Vec<ExportProject>,
    tasks: Vec<ExportTask>,
    task_tags: Vec<ExportTag>,
    task_tag_links: Vec<ExportTagLink>,
    task_completions: Vec<ExportCompletion>,
    habits: Vec<ExportHabit>,
    habit_settings: Vec<ExportHabitSetting>,
    habit_pauses: Vec<ExportHabitPause>,
    habit_checkins: Vec<ExportHabitCheckin>,
    daily_journals: Vec<ExportJournal>,
}

#[utoipa::path(get, path = "/export", operation_id = "download_private_export", security(("admin_session" = [])), responses((status = 200, body = ExportData, content_type = "application/json"), (status = 401, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn download(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
) -> Result<Response<Body>, HabitFailure> {
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    let projects: Vec<ExportProject> = sqlx::query_as("SELECT id, name, version, completed_at, archived_at, created_at, updated_at FROM project ORDER BY id").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let tasks: Vec<ExportTask> = sqlx::query_as("SELECT id, parent_id, project_id, title, description, status, priority, planned_date, due_date, in_backlog, version, created_at, updated_at FROM task ORDER BY id").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let task_tags: Vec<ExportTag> = sqlx::query_as("SELECT id, name FROM task_tag ORDER BY id")
        .fetch_all(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    let task_tag_links: Vec<ExportTagLink> =
        sqlx::query_as("SELECT task_id, tag_id FROM task_tag_link ORDER BY task_id, tag_id")
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
    let task_completions: Vec<ExportCompletion> = sqlx::query_as("SELECT id, task_id, task_title, project_name, completed_at FROM task_completion ORDER BY id").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let habits: Vec<ExportHabit> = sqlx::query_as(
        "SELECT id, created_on, deleted_at, version, created_at, updated_at FROM habit ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| unavailable())?;
    let habit_settings: Vec<ExportHabitSetting> = sqlx::query_as("SELECT id, habit_id, effective_on, name, cadence, weekly_target, created_at FROM habit_setting ORDER BY id").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let habit_pauses: Vec<ExportHabitPause> = sqlx::query_as(
        "SELECT id, habit_id, start_on, end_on, created_at FROM habit_pause ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| unavailable())?;
    let habit_checkins: Vec<ExportHabitCheckin> = sqlx::query_as("SELECT id, habit_id, business_date, completed, note, version, created_at, updated_at FROM habit_checkin ORDER BY id").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let daily_journals: Vec<ExportJournal> = sqlx::query_as("SELECT business_date, body, version, created_at, updated_at FROM daily_journal ORDER BY business_date").fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    let data = ExportData {
        format_version: 1,
        generated_at: clock.instant().to_rfc3339(),
        projects,
        tasks,
        task_tags,
        task_tag_links,
        task_completions,
        habits,
        habit_settings,
        habit_pauses,
        habit_checkins,
        daily_journals,
    };
    let bytes = serde_json::to_vec(&data).map_err(|_| unavailable())?;
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"mindfolio-{}.json\"",
            clock.business_date()
        ))
        .map_err(|_| unavailable())?,
    );
    Ok(response)
}
