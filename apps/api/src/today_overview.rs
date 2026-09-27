use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};
use chrono::{Datelike, Duration};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;

use crate::{
    ApiError,
    auth::PrivateState,
    habits::{HabitFailure, invalid, unavailable},
    tasks::TodayClock,
};

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new().route("/today/overview", get(overview))
}

#[derive(Deserialize)]
pub(crate) struct TodayOverviewQuery {
    page: Option<i64>,
}

#[derive(FromRow)]
struct TodayHabitRow {
    id: i64,
    version: i64,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
    checkin_completed: Option<bool>,
    checkin_note: Option<String>,
    checkin_version: Option<i64>,
    completed_this_week: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TodayHabit {
    id: String,
    version: i64,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
    checkin_completed: Option<bool>,
    checkin_note: Option<String>,
    checkin_version: Option<i64>,
    needs_checkin: bool,
}

impl From<TodayHabitRow> for TodayHabit {
    fn from(row: TodayHabitRow) -> Self {
        let needs_checkin = row.checkin_completed != Some(true)
            && (row.cadence == "daily"
                || row.completed_this_week < i64::from(row.weekly_target.unwrap_or(0)));
        Self {
            id: row.id.to_string(),
            version: row.version,
            name: row.name,
            cadence: row.cadence,
            weekly_target: row.weekly_target,
            checkin_completed: row.checkin_completed,
            checkin_note: row.checkin_note,
            checkin_version: row.checkin_version,
            needs_checkin,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TodayOverview {
    business_date: String,
    habits: Vec<TodayHabit>,
    page: i64,
    has_more: bool,
    journal_exists: bool,
}

#[utoipa::path(get, path = "/today/overview", operation_id = "get_today_overview", security(("admin_session" = [])), params(("page" = Option<i64>, Query)), responses((status = 200, body = TodayOverview), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn overview(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    query: Result<Query<TodayOverviewQuery>, QueryRejection>,
) -> Result<Json<TodayOverview>, HabitFailure> {
    let Query(query) = query.map_err(|_| invalid())?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid());
    }
    let today = clock.business_date();
    let monday = today - Duration::days(today.weekday().num_days_from_monday().into());
    let mut rows: Vec<TodayHabitRow> = sqlx::query_as("SELECT h.id, h.version, s.name, s.cadence, s.weekly_target, c.completed AS checkin_completed, c.note AS checkin_note, c.version AS checkin_version, (SELECT count(*) FROM habit_checkin w WHERE w.habit_id = h.id AND w.business_date BETWEEN $2 AND $1 AND w.completed) AS completed_this_week FROM habit h JOIN LATERAL (SELECT name, cadence, weekly_target FROM habit_setting WHERE habit_id = h.id AND effective_on <= $1 ORDER BY effective_on DESC LIMIT 1) s ON true LEFT JOIN habit_checkin c ON c.habit_id = h.id AND c.business_date = $1 WHERE h.deleted_at IS NULL AND h.created_on <= $1 AND NOT EXISTS (SELECT 1 FROM habit_pause p WHERE p.habit_id = h.id AND p.start_on <= $1 AND (p.end_on IS NULL OR p.end_on > $1)) ORDER BY h.created_at DESC, h.id DESC LIMIT 51 OFFSET $3").bind(today).bind(monday).bind((page - 1) * 50).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    let journal_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM daily_journal WHERE business_date = $1)")
            .bind(today)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| unavailable())?;
    Ok(Json(TodayOverview {
        business_date: today.to_string(),
        habits: rows.into_iter().map(Into::into).collect(),
        page,
        has_more,
        journal_exists,
    }))
}
