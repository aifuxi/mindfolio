use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    routing::get,
};
use chrono::{Datelike, Duration, Months, NaiveDate};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;

use crate::habits::{HabitFailure, conflict, invalid, missing, parse_id, unavailable};
use crate::{ApiError, auth::PrivateState, tasks::TodayClock};

pub(crate) fn routes() -> Router<PrivateState> {
    Router::new()
        .route("/habits/{id}/checkins", get(list))
        .route("/habits/{id}/checkins/{date}", axum::routing::put(save))
        .route("/habits/{id}/calendar", get(calendar))
}

#[derive(FromRow)]
struct CheckinRow {
    id: i64,
    habit_id: i64,
    business_date: NaiveDate,
    completed: bool,
    note: String,
    version: i64,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CheckinResponse {
    id: String,
    habit_id: String,
    business_date: String,
    completed: bool,
    note: String,
    version: i64,
}

impl From<CheckinRow> for CheckinResponse {
    fn from(row: CheckinRow) -> Self {
        Self {
            id: row.id.to_string(),
            habit_id: row.habit_id.to_string(),
            business_date: row.business_date.to_string(),
            completed: row.completed,
            note: row.note,
            version: row.version,
        }
    }
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct SaveCheckin {
    completed: bool,
    note: String,
    expected_version: Option<i64>,
}

#[derive(Deserialize)]
pub(crate) struct RangeQuery {
    from: String,
    to: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CheckinList {
    items: Vec<CheckinResponse>,
}

#[derive(Deserialize)]
pub(crate) struct CalendarQuery {
    month: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DayState {
    BeforeCreation,
    Deleted,
    Future,
    Paused,
    Completed,
    Incomplete,
    Missed,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CalendarDay {
    date: String,
    state: DayState,
    name: Option<String>,
    cadence: Option<String>,
    weekly_target: Option<i16>,
    note: Option<String>,
    checkin_version: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct Rate {
    earned: f64,
    possible: f64,
    percent: Option<f64>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CalendarResponse {
    month: String,
    today: String,
    days: Vec<CalendarDay>,
    weeks: Vec<WeekRate>,
    month_rate: Rate,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct WeekRate {
    starts_on: String,
    ends_on: String,
    rate: Rate,
}

#[derive(FromRow)]
struct Setting {
    id: i64,
    effective_on: NaiveDate,
    name: String,
    cadence: String,
    weekly_target: Option<i16>,
}

#[derive(FromRow)]
struct Pause {
    start_on: NaiveDate,
    end_on: Option<NaiveDate>,
}

#[derive(FromRow)]
struct HabitBoundary {
    created_on: NaiveDate,
    deleted_at: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

#[derive(Clone)]
struct DayFact {
    date: NaiveDate,
    setting_id: i64,
    cadence: String,
    target: i16,
    eligible: bool,
    completed: bool,
}

pub(crate) fn date(raw: &str) -> Result<NaiveDate, HabitFailure> {
    if raw.len() != 10 || raw.as_bytes()[4] != b'-' || raw.as_bytes()[7] != b'-' {
        return Err(invalid());
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| invalid())
}

fn month(raw: &str) -> Result<NaiveDate, HabitFailure> {
    if raw.len() != 7 || raw.as_bytes()[4] != b'-' {
        return Err(invalid());
    }
    date(&format!("{raw}-01"))
}

fn monday(day: NaiveDate) -> NaiveDate {
    day - Duration::days(day.weekday().num_days_from_monday().into())
}

async fn boundary(pool: &PgPool, id: i64) -> Result<HabitBoundary, HabitFailure> {
    sqlx::query_as("SELECT created_on, deleted_at FROM habit WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)
}

#[utoipa::path(get, path = "/habits/{id}/checkins", operation_id = "list_habit_checkins", security(("admin_session" = [])), params(("id" = String, Path), ("from" = String, Query), ("to" = String, Query)), responses((status = 200, body = CheckinList), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn list(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    query: Result<Query<RangeQuery>, QueryRejection>,
) -> Result<Json<CheckinList>, HabitFailure> {
    let id = parse_id(&id)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let from = date(&query.from)?;
    let to = date(&query.to)?;
    if to < from || (to - from).num_days() > 366 {
        return Err(invalid());
    }
    boundary(&state.pool, id).await?;
    let rows: Vec<CheckinRow> = sqlx::query_as("SELECT id, habit_id, business_date, completed, note, version FROM habit_checkin WHERE habit_id = $1 AND business_date BETWEEN $2 AND $3 ORDER BY business_date, id").bind(id).bind(from).bind(to).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    Ok(Json(CheckinList {
        items: rows.into_iter().map(Into::into).collect(),
    }))
}

#[utoipa::path(put, path = "/habits/{id}/checkins/{date}", operation_id = "save_habit_checkin", security(("admin_session" = [])), params(("id" = String, Path), ("date" = String, Path), ("x-csrf-token" = String, Header)), request_body = SaveCheckin, responses((status = 200, body = CheckinResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn save(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path((id, raw_date)): Path<(String, String)>,
    input: Result<Json<SaveCheckin>, JsonRejection>,
) -> Result<Json<CheckinResponse>, HabitFailure> {
    let id = parse_id(&id)?;
    let day = date(&raw_date)?;
    let Json(input) = input.map_err(|_| invalid())?;
    if day > clock.business_date()
        || input.note.chars().count() > 2000
        || input.expected_version.is_some_and(|v| v < 1)
    {
        return Err(invalid());
    }
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    let bound: Option<HabitBoundary> =
        sqlx::query_as("SELECT created_on, deleted_at FROM habit WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
    let bound = bound.ok_or_else(missing)?;
    if bound.deleted_at.is_some() || day < bound.created_on {
        return Err(invalid());
    }
    let paused: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM habit_pause WHERE habit_id = $1 AND start_on <= $2 AND (end_on IS NULL OR end_on > $2))").bind(id).bind(day).fetch_one(&mut *tx).await.map_err(|_| unavailable())?;
    if paused {
        return Err(invalid());
    }
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT version FROM habit_checkin WHERE habit_id = $1 AND business_date = $2",
    )
    .bind(id)
    .bind(day)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| unavailable())?;
    if existing != input.expected_version {
        return Err(conflict());
    }
    let saved: CheckinRow = if let Some(version) = existing {
        sqlx::query_as("UPDATE habit_checkin SET completed = $1, note = $2, version = version + 1, updated_at = now() WHERE habit_id = $3 AND business_date = $4 AND version = $5 RETURNING id, habit_id, business_date, completed, note, version").bind(input.completed).bind(input.note).bind(id).bind(day).bind(version).fetch_one(&mut *tx).await.map_err(|_| unavailable())?
    } else {
        sqlx::query_as("INSERT INTO habit_checkin (habit_id, business_date, completed, note) VALUES ($1, $2, $3, $4) RETURNING id, habit_id, business_date, completed, note, version").bind(id).bind(day).bind(input.completed).bind(input.note).fetch_one(&mut *tx).await.map_err(|_| unavailable())?
    };
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(saved.into()))
}

fn rate(earned: f64, possible: f64) -> Rate {
    Rate {
        earned,
        possible,
        percent: if possible > 0.0 {
            Some((earned / possible * 1000.0).round() / 10.0)
        } else {
            None
        },
    }
}

fn week_rate(days: &[DayFact]) -> Rate {
    let mut earned = 0.0;
    let mut possible = 0.0;
    let mut index = 0;
    while index < days.len() {
        let key = days[index].setting_id;
        let cadence = &days[index].cadence;
        let target = days[index].target;
        let mut eligible = 0;
        let mut done = 0;
        while index < days.len() && days[index].setting_id == key {
            if days[index].eligible {
                eligible += 1;
                if days[index].completed {
                    done += 1;
                }
            }
            index += 1;
        }
        let goal = if cadence == "daily" {
            eligible
        } else {
            eligible.min(i32::from(target))
        };
        possible += f64::from(goal);
        earned += f64::from(done.min(goal));
    }
    rate(earned, possible)
}

#[utoipa::path(get, path = "/habits/{id}/calendar", operation_id = "get_habit_calendar", security(("admin_session" = [])), params(("id" = String, Path), ("month" = String, Query)), responses((status = 200, body = CalendarResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError)))]
pub(crate) async fn calendar(
    State(state): State<PrivateState>,
    axum::Extension(clock): axum::Extension<TodayClock>,
    Path(id): Path<String>,
    query: Result<Query<CalendarQuery>, QueryRejection>,
) -> Result<Json<CalendarResponse>, HabitFailure> {
    let id = parse_id(&id)?;
    let Query(query) = query.map_err(|_| invalid())?;
    let first = month(&query.month)?;
    let next = first
        .checked_add_months(Months::new(1))
        .ok_or_else(invalid)?;
    let start = monday(first);
    let end = monday(next - Duration::days(1)) + Duration::days(7);
    let today = clock.business_date();
    let bound = boundary(&state.pool, id).await?;
    let settings: Vec<Setting> = sqlx::query_as("SELECT id, effective_on, name, cadence, weekly_target FROM habit_setting WHERE habit_id = $1 AND effective_on < $2 ORDER BY effective_on, id").bind(id).bind(end).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let pauses: Vec<Pause> = sqlx::query_as("SELECT start_on, end_on FROM habit_pause WHERE habit_id = $1 AND start_on < $2 AND (end_on IS NULL OR end_on > $3) ORDER BY start_on").bind(id).bind(end).bind(start).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let checkins: Vec<CheckinRow> = sqlx::query_as("SELECT id, habit_id, business_date, completed, note, version FROM habit_checkin WHERE habit_id = $1 AND business_date >= $2 AND business_date < $3 ORDER BY business_date").bind(id).bind(start).bind(end).fetch_all(&state.pool).await.map_err(|_| unavailable())?;
    let deleted_on = bound.deleted_at.map(|v| {
        v.with_timezone(&sqlx::types::chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .date_naive()
    });
    let mut facts = Vec::new();
    let mut shown = Vec::new();
    let mut setting_index = 0usize;
    let mut checkin_index = 0usize;
    let mut day = start;
    while day < end {
        while setting_index + 1 < settings.len() && settings[setting_index + 1].effective_on <= day
        {
            setting_index += 1;
        }
        while checkin_index < checkins.len() && checkins[checkin_index].business_date < day {
            checkin_index += 1;
        }
        let setting = settings
            .get(setting_index)
            .filter(|s| s.effective_on <= day);
        let checkin = checkins
            .get(checkin_index)
            .filter(|c| c.business_date == day);
        let paused = pauses
            .iter()
            .any(|p| p.start_on <= day && p.end_on.is_none_or(|end| day < end));
        let inactive = day < bound.created_on || deleted_on.is_some_and(|deleted| day > deleted);
        let eligible = !inactive && day <= today && !paused && setting.is_some();
        let state = if day < bound.created_on {
            DayState::BeforeCreation
        } else if deleted_on.is_some_and(|deleted| day > deleted) {
            DayState::Deleted
        } else if day > today {
            DayState::Future
        } else if paused {
            DayState::Paused
        } else if checkin.is_some_and(|c| c.completed) {
            DayState::Completed
        } else if checkin.is_some() {
            DayState::Incomplete
        } else {
            DayState::Missed
        };
        if let Some(s) = setting {
            facts.push(DayFact {
                date: day,
                setting_id: s.id,
                cadence: s.cadence.clone(),
                target: s.weekly_target.unwrap_or(1),
                eligible,
                completed: checkin.is_some_and(|c| c.completed),
            });
        }
        if day >= first && day < next {
            shown.push(CalendarDay {
                date: day.to_string(),
                state,
                name: setting.map(|s| s.name.clone()),
                cadence: setting.map(|s| s.cadence.clone()),
                weekly_target: setting.and_then(|s| s.weekly_target),
                note: checkin.map(|c| c.note.clone()),
                checkin_version: checkin.map(|c| c.version),
            });
        }
        day += Duration::days(1);
    }
    let mut weeks = Vec::new();
    let mut weighted_earned = 0.0;
    let mut weighted_possible = 0.0;
    let mut week = start;
    while week < end {
        let group: Vec<DayFact> = facts
            .iter()
            .filter(|f| f.date >= week && f.date < week + Duration::days(7))
            .cloned()
            .collect();
        let current = week_rate(&group);
        let weight = group
            .iter()
            .filter(|f| f.eligible && f.date >= first && f.date < next)
            .count() as f64;
        if current.possible > 0.0 {
            weighted_earned += current.earned / current.possible * weight;
            weighted_possible += weight;
        }
        weeks.push(WeekRate {
            starts_on: week.to_string(),
            ends_on: (week + Duration::days(6)).to_string(),
            rate: current,
        });
        week += Duration::days(7);
    }
    Ok(Json(CalendarResponse {
        month: query.month,
        today: today.to_string(),
        days: shown,
        weeks,
        month_rate: rate(weighted_earned, weighted_possible),
    }))
}
