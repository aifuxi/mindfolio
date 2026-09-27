use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    FromRow,
    types::chrono::{DateTime, NaiveDate, Utc},
};
use utoipa::ToSchema;

use crate::{
    ApiError,
    auth::PrivateState,
    checkins::date,
    habits::{HabitFailure, conflict, invalid, unavailable},
    tasks::TodayClock,
};

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new().route("/journal/{date}", get(day).put(save))
}

#[derive(FromRow)]
struct JournalRow {
    business_date: NaiveDate,
    body: String,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct JournalResponse {
    business_date: String,
    body: String,
    version: i64,
    created_at: String,
    updated_at: String,
}

impl From<JournalRow> for JournalResponse {
    fn from(row: JournalRow) -> Self {
        Self {
            business_date: row.business_date.to_string(),
            body: row.body,
            version: row.version,
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

#[derive(FromRow)]
struct CompletionRow {
    id: i64,
    task_id: i64,
    task_title: String,
    project_name: Option<String>,
    completed_at: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct DayCompletion {
    id: String,
    task_id: String,
    task_title: String,
    project_name: Option<String>,
    completed_at: String,
}

impl From<CompletionRow> for DayCompletion {
    fn from(row: CompletionRow) -> Self {
        Self {
            id: row.id.to_string(),
            task_id: row.task_id.to_string(),
            task_title: row.task_title,
            project_name: row.project_name,
            completed_at: row.completed_at.to_rfc3339(),
        }
    }
}

#[derive(FromRow)]
struct CheckinRow {
    id: i64,
    habit_id: i64,
    name: String,
    completed: bool,
    note: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct DayCheckin {
    id: String,
    habit_id: String,
    name: String,
    completed: bool,
    note: String,
}

impl From<CheckinRow> for DayCheckin {
    fn from(row: CheckinRow) -> Self {
        Self {
            id: row.id.to_string(),
            habit_id: row.habit_id.to_string(),
            name: row.name,
            completed: row.completed,
            note: row.note,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct JournalDay {
    business_date: String,
    journal: Option<JournalResponse>,
    completions: Vec<DayCompletion>,
    checkins: Vec<DayCheckin>,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct SaveJournal {
    body: String,
    expected_version: Option<i64>,
}

#[utoipa::path(get, path = "/journal/{date}", operation_id = "get_journal_day", security(("admin_session" = [])), params(("date" = String, Path)), responses((status = 200, body = JournalDay), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn day(
    State(state): State<PrivateState>,
    Path(raw_date): Path<String>,
) -> Result<Json<JournalDay>, HabitFailure> {
    let business_date = date(&raw_date)?;
    let journal: Option<JournalRow> = sqlx::query_as("SELECT business_date, body, version, created_at, updated_at FROM daily_journal WHERE business_date = $1").bind(business_date).fetch_optional(&state.pool).await.map_err(|_| unavailable())?;
    let completions: Vec<CompletionRow> = sqlx::query_as("SELECT id, task_id, task_title, project_name, completed_at FROM task_completion WHERE (completed_at AT TIME ZONE 'Asia/Shanghai')::date = $1 ORDER BY completed_at DESC, id DESC").bind(business_date).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let checkins: Vec<CheckinRow> = sqlx::query_as("SELECT c.id, c.habit_id, s.name, c.completed, c.note FROM habit_checkin c JOIN LATERAL (SELECT name FROM habit_setting WHERE habit_id = c.habit_id AND effective_on <= c.business_date ORDER BY effective_on DESC LIMIT 1) s ON true WHERE c.business_date = $1 ORDER BY c.id").bind(business_date).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    Ok(Json(JournalDay {
        business_date: raw_date,
        journal: journal.map(Into::into),
        completions: completions.into_iter().map(Into::into).collect(),
        checkins: checkins.into_iter().map(Into::into).collect(),
    }))
}

#[utoipa::path(put, path = "/journal/{date}", operation_id = "save_journal", security(("admin_session" = [])), params(("date" = String, Path), ("x-csrf-token" = String, Header)), request_body = SaveJournal, responses((status = 200, body = JournalResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn save(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(raw_date): Path<String>,
    input: Result<Json<SaveJournal>, JsonRejection>,
) -> Result<Json<JournalResponse>, HabitFailure> {
    let business_date = date(&raw_date)?;
    let Json(input) = input.map_err(|_| invalid())?;
    if business_date > clock.business_date()
        || input.body.trim().is_empty()
        || input.body.chars().count() > 100000
        || input.expected_version.is_some_and(|v| v < 1)
    {
        return Err(invalid());
    }
    let row: Option<JournalRow> = if let Some(version) = input.expected_version {
        sqlx::query_as("UPDATE daily_journal SET body = $1, version = version + 1, updated_at = now() WHERE business_date = $2 AND version = $3 RETURNING business_date, body, version, created_at, updated_at").bind(input.body).bind(business_date).bind(version).fetch_optional(&state.pool).await.map_err(|_| unavailable())?
    } else {
        sqlx::query_as("INSERT INTO daily_journal (business_date, body) VALUES ($1, $2) ON CONFLICT DO NOTHING RETURNING business_date, body, version, created_at, updated_at").bind(business_date).bind(input.body).fetch_optional(&state.pool).await.map_err(|_| unavailable())?
    };
    row.map(|row| Json(row.into())).ok_or_else(conflict)
}
