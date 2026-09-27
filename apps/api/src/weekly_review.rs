use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    ApiError,
    auth::PrivateState,
    checkins::{WeekSnapshot, date, week_snapshot},
    habits::{HabitFailure, invalid, unavailable},
    tasks::TodayClock,
};

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new().route("/weekly-review", get(review))
}

#[derive(Deserialize)]
pub(crate) struct ReviewQuery {
    date: Option<String>,
    page: Option<i64>,
}

#[derive(FromRow)]
struct CompletionRow {
    id: i64,
    task_id: i64,
    task_title: String,
    project_name: Option<String>,
    completed_at: DateTime<Utc>,
    task_exists: bool,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ReviewCompletion {
    id: String,
    task_id: String,
    task_title: String,
    project_name: Option<String>,
    completed_at: String,
    business_date: String,
    task_exists: bool,
}

impl From<CompletionRow> for ReviewCompletion {
    fn from(row: CompletionRow) -> Self {
        Self {
            id: row.id.to_string(),
            task_id: row.task_id.to_string(),
            task_title: row.task_title,
            project_name: row.project_name,
            business_date: row
                .completed_at
                .with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap())
                .date_naive()
                .to_string(),
            completed_at: row.completed_at.to_rfc3339(),
            task_exists: row.task_exists,
        }
    }
}

#[derive(FromRow)]
struct JournalRow {
    business_date: NaiveDate,
    body: String,
    version: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ReviewJournal {
    business_date: String,
    body: String,
    version: i64,
}

impl From<JournalRow> for ReviewJournal {
    fn from(row: JournalRow) -> Self {
        Self {
            business_date: row.business_date.to_string(),
            body: row.body,
            version: row.version,
        }
    }
}

#[derive(FromRow)]
struct HabitRow {
    id: i64,
    name: String,
    current_exists: bool,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ReviewHabit {
    id: String,
    name: String,
    current_exists: bool,
    snapshot: WeekSnapshot,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct WeeklyReview {
    starts_on: String,
    ends_on: String,
    dates: Vec<String>,
    page: i64,
    completions: Vec<ReviewCompletion>,
    has_more_completions: bool,
    journals: Vec<ReviewJournal>,
    habits: Vec<ReviewHabit>,
    has_more_habits: bool,
}

#[utoipa::path(get, path = "/weekly-review", operation_id = "get_weekly_review", security(("admin_session" = [])), params(("date" = Option<String>, Query), ("page" = Option<i64>, Query)), responses((status = 200, body = WeeklyReview), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn review(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    query: Result<Query<ReviewQuery>, QueryRejection>,
) -> Result<Json<WeeklyReview>, HabitFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid());
    }
    let today = clock.business_date();
    let selected = query
        .date
        .as_deref()
        .map(date)
        .transpose()?
        .unwrap_or(today);
    let start = selected - Duration::days(selected.weekday().num_days_from_monday().into());
    if start > today {
        return Err(invalid());
    }
    let end = start + Duration::days(6);
    let next = end + Duration::days(1);
    let mut completion_rows: Vec<CompletionRow> = sqlx::query_as("SELECT c.id, c.task_id, c.task_title, c.project_name, c.completed_at, EXISTS(SELECT 1 FROM task t WHERE t.id = c.task_id) AS task_exists FROM task_completion c WHERE c.completed_at >= ($1::date::timestamp AT TIME ZONE 'Asia/Shanghai') AND c.completed_at < ($2::date::timestamp AT TIME ZONE 'Asia/Shanghai') ORDER BY c.completed_at DESC, c.id DESC LIMIT 51 OFFSET $3").bind(start).bind(next).bind((page - 1) * 50).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let has_more_completions = completion_rows.len() > 50;
    completion_rows.truncate(50);
    let journal_rows: Vec<JournalRow> = sqlx::query_as("SELECT business_date, body, version FROM daily_journal WHERE business_date BETWEEN $1 AND $2 ORDER BY business_date").bind(start).bind(end).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let mut habit_rows: Vec<HabitRow> = sqlx::query_as("SELECT h.id, s.name, h.deleted_at IS NULL AS current_exists FROM habit h JOIN LATERAL (SELECT name FROM habit_setting WHERE habit_id = h.id AND effective_on <= LEAST($2::date, $3::date) ORDER BY effective_on DESC LIMIT 1) s ON true WHERE h.created_on <= $2 AND (h.deleted_at IS NULL OR (h.deleted_at AT TIME ZONE 'Asia/Shanghai')::date >= $1) ORDER BY h.created_on, h.id LIMIT 51 OFFSET $4").bind(start).bind(end).bind(today).bind((page - 1) * 50).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let has_more_habits = habit_rows.len() > 50;
    habit_rows.truncate(50);
    let mut habits = Vec::with_capacity(habit_rows.len());
    for row in habit_rows {
        habits.push(ReviewHabit {
            id: row.id.to_string(),
            name: row.name,
            current_exists: row.current_exists,
            snapshot: week_snapshot(&state.pool, row.id, start, today).await?,
        });
    }
    Ok(Json(WeeklyReview {
        starts_on: start.to_string(),
        ends_on: end.to_string(),
        dates: (0..7)
            .map(|day| (start + Duration::days(day)).to_string())
            .collect(),
        page,
        completions: completion_rows.into_iter().map(Into::into).collect(),
        has_more_completions,
        journals: journal_rows.into_iter().map(Into::into).collect(),
        habits,
        has_more_habits,
    }))
}
