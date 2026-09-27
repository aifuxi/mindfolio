use axum::{
    Extension, Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::{DateTime, NaiveDate, Utc};
use utoipa::ToSchema;

use crate::{ErrorCode, auth::PrivateState};

use super::{
    FIELDS, TaskFailure, TaskResponse, TaskRow, failure, invalid, missing, parse_id, unavailable,
};

#[derive(Clone)]
pub struct TodayClock {
    fixed_instant: Option<DateTime<Utc>>,
}

impl TodayClock {
    pub fn system() -> Self {
        Self {
            fixed_instant: None,
        }
    }

    pub fn fixed(instant: DateTime<Utc>) -> Self {
        Self {
            fixed_instant: Some(instant),
        }
    }

    pub(crate) fn instant(&self) -> DateTime<Utc> {
        self.fixed_instant.unwrap_or_else(Utc::now)
    }

    pub(crate) fn business_date(&self) -> NaiveDate {
        self.instant()
            .with_timezone(&sqlx::types::chrono::FixedOffset::east_opt(8 * 3600).unwrap())
            .date_naive()
    }
}

pub(crate) fn today_routes(clock: TodayClock) -> Router<PrivateState> {
    Router::new()
        .route("/today/tasks", get(today_tasks))
        .route("/tasks/{id}/plan-today", post(plan_today))
        .layer(Extension(clock))
}

#[derive(Deserialize)]
pub(crate) struct TodayQuery {
    page: Option<i64>,
}

#[derive(Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TodayReason {
    Overdue,
    DueToday,
    PlannedToday,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TodayTaskResponse {
    task: TaskResponse,
    reasons: Vec<TodayReason>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct TodayTaskPage {
    business_date: String,
    items: Vec<TodayTaskResponse>,
    page: i64,
    has_more: bool,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct PlanTodayRequest {
    expected_version: i64,
}

async fn business_date(state: &PrivateState, clock: &TodayClock) -> Result<NaiveDate, TaskFailure> {
    sqlx::query_scalar("SELECT ($1::timestamptz AT TIME ZONE 'Asia/Shanghai')::date")
        .bind(clock.instant())
        .fetch_one(&state.pool)
        .await
        .map_err(|_| unavailable())
}

fn reasons(row: &TaskRow, today: NaiveDate) -> Vec<TodayReason> {
    let mut reasons = Vec::with_capacity(2);
    if row.due_date.is_some_and(|date| date < today) {
        reasons.push(TodayReason::Overdue);
    } else if row.due_date == Some(today) {
        reasons.push(TodayReason::DueToday);
    }
    if row.planned_date == Some(today) {
        reasons.push(TodayReason::PlannedToday);
    }
    reasons
}

#[utoipa::path(
    get, path = "/today/tasks", operation_id = "list_today_tasks", security(("admin_session" = [])),
    description = "服务端按 Asia/Shanghai 计算今天；返回计划今天、今天到期或已逾期的待办与进行中任务，按逾期、今天到期、计划今天排序。",
    params(("page" = Option<i64>, Query)),
    responses((status = 200, body = TodayTaskPage), (status = 400, body = crate::ApiError), (status = 401, body = crate::ApiError), (status = 503, body = crate::ApiError))
)]
pub(crate) async fn today_tasks(
    State(state): State<PrivateState>,
    Extension(clock): Extension<TodayClock>,
    query: Result<Query<TodayQuery>, QueryRejection>,
) -> Result<Json<TodayTaskPage>, TaskFailure> {
    let Query(query) = query.map_err(|_| invalid("查询参数无效"))?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid("页码无效"));
    }
    let today = business_date(&state, &clock).await?;
    let statement = format!(
        "SELECT {FIELDS} FROM task WHERE status IN ('todo', 'in_progress') \
         AND (planned_date = $1 OR due_date <= $1) \
         ORDER BY CASE WHEN due_date < $1 THEN 0 WHEN due_date = $1 THEN 1 ELSE 2 END, \
         due_date ASC NULLS LAST, planned_date ASC NULLS LAST, created_at DESC, id DESC \
         LIMIT 51 OFFSET $2"
    );
    let mut rows: Vec<TaskRow> = sqlx::query_as(&statement)
        .bind(today)
        .bind((page - 1) * 50)
        .fetch_all(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(Json(TodayTaskPage {
        business_date: today.to_string(),
        items: rows
            .into_iter()
            .map(|row| TodayTaskResponse {
                reasons: reasons(&row, today),
                task: row.into(),
            })
            .collect(),
        page,
        has_more,
    }))
}

#[utoipa::path(
    post, path = "/tasks/{id}/plan-today", operation_id = "plan_task_today", security(("admin_session" = [])),
    description = "按服务端 Asia/Shanghai 今日设置计划日期；截止日期和完成记录不变。",
    params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = PlanTodayRequest,
    responses((status = 200, body = TaskResponse), (status = 400, body = crate::ApiError), (status = 401, body = crate::ApiError), (status = 403, body = crate::ApiError), (status = 404, body = crate::ApiError), (status = 409, body = crate::ApiError), (status = 503, body = crate::ApiError))
)]
pub(crate) async fn plan_today(
    State(state): State<PrivateState>,
    Extension(clock): Extension<TodayClock>,
    Path(id): Path<String>,
    input: Result<Json<PlanTodayRequest>, JsonRejection>,
) -> Result<Json<TaskResponse>, TaskFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid("请求无效"))?;
    if input.expected_version < 1 {
        return Err(invalid("版本无效"));
    }
    let today = business_date(&state, &clock).await?;
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1 FOR UPDATE");
    let row: TaskRow = sqlx::query_as(&statement)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)?;
    if row.version != input.expected_version
        || !matches!(row.status.as_str(), "todo" | "in_progress")
    {
        return Err(failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "任务已变更或不再待处理，请刷新后重试",
        ));
    }
    if row.planned_date == Some(today) {
        return Ok(Json(row.into()));
    }
    let statement = format!(
        "UPDATE task SET planned_date = $1, version = version + 1, updated_at = now() \
         WHERE id = $2 AND version = $3 RETURNING {FIELDS}"
    );
    let saved: TaskRow = sqlx::query_as(&statement)
        .bind(today)
        .bind(id)
        .bind(input.expected_version)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(saved.into()))
}
