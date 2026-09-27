use axum::{
    Json, Router,
    extract::{
        Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction, types::chrono::NaiveDate};
use utoipa::ToSchema;

use crate::{ApiError, ErrorCode, auth::PrivateState};

type TaskFailure = (StatusCode, Json<ApiError>);

pub(super) fn routes() -> Router<PrivateState> {
    Router::new()
        .route("/tasks", get(list).post(create))
        .route("/tasks/{id}", get(detail).patch(update))
        .route("/tasks/{id}/subtasks", get(list_subtasks))
}

#[derive(Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum TaskStatus {
    Todo,
    InProgress,
    Completed,
    Canceled,
}

impl TaskStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
        }
    }

    fn from_db(raw: &str) -> Self {
        match raw {
            "todo" => Self::Todo,
            "in_progress" => Self::InProgress,
            "completed" => Self::Completed,
            "canceled" => Self::Canceled,
            _ => unreachable!("数据库状态约束保证任务状态有效"),
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

impl TaskPriority {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Urgent => "urgent",
        }
    }

    fn from_db(raw: &str) -> Self {
        match raw {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "urgent" => Self::Urgent,
            _ => unreachable!("数据库约束保证任务优先级有效"),
        }
    }
}

#[derive(Clone, PartialEq, FromRow)]
struct TaskRow {
    id: i64,
    parent_id: Option<i64>,
    project_id: Option<i64>,
    title: String,
    description: String,
    status: String,
    priority: Option<String>,
    planned_date: Option<NaiveDate>,
    due_date: Option<NaiveDate>,
    in_backlog: bool,
    tags: Vec<String>,
    version: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct TaskResponse {
    id: String,
    parent_id: Option<String>,
    project_id: Option<String>,
    title: String,
    description: String,
    status: TaskStatus,
    priority: Option<TaskPriority>,
    planned_date: Option<String>,
    due_date: Option<String>,
    in_backlog: bool,
    tags: Vec<String>,
    version: i64,
}

impl From<TaskRow> for TaskResponse {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id.to_string(),
            parent_id: row.parent_id.map(|id| id.to_string()),
            project_id: row.project_id.map(|id| id.to_string()),
            title: row.title,
            description: row.description,
            status: TaskStatus::from_db(&row.status),
            priority: row.priority.as_deref().map(TaskPriority::from_db),
            planned_date: row.planned_date.map(|date| date.to_string()),
            due_date: row.due_date.map(|date| date.to_string()),
            in_backlog: row.in_backlog,
            tags: row.tags,
            version: row.version,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(super) struct TaskPage {
    items: Vec<TaskResponse>,
    page: i64,
    has_more: bool,
}

#[derive(Deserialize)]
pub(super) struct ListQuery {
    scope: String,
    project_id: Option<String>,
    include_subtasks: Option<bool>,
    page: Option<i64>,
    #[serde(flatten)]
    filters: SearchFilters,
}

#[derive(Default, Deserialize)]
pub(super) struct SearchFilters {
    keyword: Option<String>,
    status: Option<TaskStatus>,
    priority: Option<TaskPriority>,
    planned_date: Option<String>,
    due_date: Option<String>,
    tag: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub(super) struct CreateTask {
    title: String,
    parent_id: Option<String>,
    project_id: Option<String>,
    description: Option<String>,
    status: Option<TaskStatus>,
    priority: Option<TaskPriority>,
    planned_date: Option<String>,
    due_date: Option<String>,
    in_backlog: Option<bool>,
    tags: Option<Vec<String>>,
}

fn present_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Deserialize, ToSchema)]
pub(super) struct UpdateTask {
    expected_version: i64,
    title: Option<String>,
    #[serde(default, deserialize_with = "present_nullable")]
    parent_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_nullable")]
    project_id: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_nullable")]
    description: Option<Option<String>>,
    status: Option<TaskStatus>,
    #[serde(default, deserialize_with = "present_nullable")]
    priority: Option<Option<TaskPriority>>,
    #[serde(default, deserialize_with = "present_nullable")]
    planned_date: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_nullable")]
    due_date: Option<Option<String>>,
    in_backlog: Option<bool>,
    tags: Option<Vec<String>>,
}

fn failure(status: StatusCode, code: ErrorCode, message: &str) -> TaskFailure {
    (
        status,
        Json(ApiError {
            code,
            message: message.into(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    )
}

fn invalid(message: &str) -> TaskFailure {
    failure(StatusCode::BAD_REQUEST, ErrorCode::InvalidRequest, message)
}

fn missing() -> TaskFailure {
    failure(StatusCode::NOT_FOUND, ErrorCode::NotFound, "任务不存在")
}

fn unavailable() -> TaskFailure {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::DatabaseUnavailable,
        "数据库不可用",
    )
}

fn parse_id(raw: &str) -> Result<i64, TaskFailure> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(missing)
}

fn parse_project_id(raw: &str) -> Result<i64, TaskFailure> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("项目 ID 无效"))
}

fn parse_parent_id(raw: &str) -> Result<i64, TaskFailure> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| invalid("父任务 ID 无效"))
}

async fn parent_task(tx: &mut Transaction<'_, Postgres>, id: i64) -> Result<TaskRow, TaskFailure> {
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1 FOR UPDATE");
    sqlx::query_as(&statement)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)
}

async fn validate_hierarchy(
    tx: &mut Transaction<'_, Postgres>,
    row: &TaskRow,
) -> Result<(), TaskFailure> {
    if let Some(parent_id) = row.parent_id {
        if parent_id == row.id {
            return Err(invalid("任务不能成为自己的子任务"));
        }
        let parent = parent_task(tx, parent_id).await?;
        if parent.parent_id.is_some() {
            return Err(invalid("子任务不能再拆分"));
        }
        if parent.project_id != row.project_id {
            return Err(invalid("子任务与父任务的项目必须一致"));
        }
    }
    let incompatible_child: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM task WHERE parent_id = $1 AND ($2::BIGINT IS NOT NULL OR project_id IS DISTINCT FROM $3::BIGINT))",
    )
    .bind(row.id)
    .bind(row.parent_id)
    .bind(row.project_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|_| unavailable())?;
    if incompatible_child {
        return Err(invalid("父任务的层级或项目与子任务不一致"));
    }
    Ok(())
}

fn title(raw: &str) -> Result<String, TaskFailure> {
    let value = raw.trim();
    if value.is_empty() || value.chars().count() > 200 || value.chars().any(char::is_control) {
        return Err(invalid("任务标题无效"));
    }
    Ok(value.into())
}

fn description(raw: String) -> Result<String, TaskFailure> {
    if raw.chars().count() > 20_000 {
        return Err(invalid("任务说明过长"));
    }
    Ok(raw)
}

fn date(raw: Option<String>) -> Result<Option<NaiveDate>, TaskFailure> {
    match raw {
        Some(raw) if raw.len() == 10 && raw.as_bytes()[4] == b'-' && raw.as_bytes()[7] == b'-' => {
            NaiveDate::parse_from_str(&raw, "%Y-%m-%d")
                .map(Some)
                .map_err(|_| invalid("日期必须为 YYYY-MM-DD"))
        }
        Some(_) => Err(invalid("日期必须为 YYYY-MM-DD")),
        None => Ok(None),
    }
}

fn normalize_tags(raw: Vec<String>) -> Result<Vec<String>, TaskFailure> {
    if raw.len() > 20 {
        return Err(invalid("任务标签不能超过 20 个"));
    }
    let mut tags = Vec::with_capacity(raw.len());
    let mut seen = std::collections::HashSet::new();
    for value in raw {
        let name = value.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err(invalid("任务标签无效"));
        }
        if !seen.insert(name.to_lowercase()) {
            return Err(invalid("任务标签不能重复"));
        }
        tags.push(name.to_string());
    }
    tags.sort_by_key(|name| name.to_lowercase());
    Ok(tags)
}

async fn replace_tags(
    tx: &mut Transaction<'_, Postgres>,
    task_id: i64,
    tags: &[String],
) -> Result<(), TaskFailure> {
    sqlx::query("DELETE FROM task_tag_link WHERE task_id = $1")
        .bind(task_id)
        .execute(&mut **tx)
        .await
        .map_err(|_| unavailable())?;
    for name in tags {
        sqlx::query("INSERT INTO task_tag (name) VALUES ($1) ON CONFLICT DO NOTHING")
            .bind(name)
            .execute(&mut **tx)
            .await
            .map_err(|_| unavailable())?;
        sqlx::query("INSERT INTO task_tag_link (task_id, tag_id) SELECT $1, id FROM task_tag WHERE lower(name) = lower($2)")
            .bind(task_id)
            .bind(name)
            .execute(&mut **tx)
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(())
}

struct ValidatedFilters {
    keyword: Option<String>,
    status: Option<&'static str>,
    priority: Option<&'static str>,
    planned_date: Option<NaiveDate>,
    due_date: Option<NaiveDate>,
    tag: Option<String>,
}

fn validate_filters(filters: SearchFilters) -> Result<ValidatedFilters, TaskFailure> {
    let keyword = filters.keyword.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });
    if keyword
        .as_ref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err(invalid("关键词过长"));
    }
    let keyword = keyword.map(|value| {
        format!(
            "%{}%",
            value
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let tag = filters.tag.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });
    if tag.as_ref().is_some_and(|value| value.chars().count() > 40) {
        return Err(invalid("任务标签无效"));
    }
    Ok(ValidatedFilters {
        keyword,
        status: filters.status.map(TaskStatus::as_str),
        priority: filters.priority.map(TaskPriority::as_str),
        planned_date: date(filters.planned_date)?,
        due_date: date(filters.due_date)?,
        tag,
    })
}

async fn project_name(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    require_active: bool,
) -> Result<String, TaskFailure> {
    let found: Option<(String, bool)> =
        sqlx::query_as("SELECT name, archived_at IS NOT NULL FROM project WHERE id = $1 FOR SHARE")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|_| unavailable())?;
    match found {
        Some((_, true)) if require_active => Err(failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "项目已归档",
        )),
        Some((name, _)) => Ok(name),
        None => Err(failure(
            StatusCode::NOT_FOUND,
            ErrorCode::NotFound,
            "项目不存在",
        )),
    }
}

async fn record_completion(
    tx: &mut Transaction<'_, Postgres>,
    row: &TaskRow,
) -> Result<(), TaskFailure> {
    let project_name = match row.project_id {
        Some(id) => Some(project_name(tx, id, false).await?),
        None => None,
    };
    sqlx::query(
        "INSERT INTO task_completion (task_id, task_title, project_name) VALUES ($1, $2, $3)",
    )
    .bind(row.id)
    .bind(&row.title)
    .bind(project_name)
    .execute(&mut **tx)
    .await
    .map_err(|_| unavailable())?;
    Ok(())
}

const FIELDS: &str = "id, parent_id, project_id, title, description, status, priority, planned_date, due_date, in_backlog, ARRAY(SELECT tag.name FROM task_tag AS tag JOIN task_tag_link AS link ON link.tag_id = tag.id WHERE link.task_id = task.id ORDER BY lower(tag.name), tag.name) AS tags, version";

async fn search_tasks(
    pool: &PgPool,
    scope: &str,
    scope_id: Option<i64>,
    include_subtasks: bool,
    page: i64,
    filters: ValidatedFilters,
) -> Result<TaskPage, TaskFailure> {
    let statement = format!(
        "SELECT {FIELDS} FROM task WHERE \
         (($1::TEXT = 'subtasks' AND parent_id = $2) OR \
          ($1 = 'project' AND project_id = $2 AND ($3 OR parent_id IS NULL)) OR \
          ($1 = 'inbox' AND project_id IS NULL AND ($3 OR parent_id IS NULL))) \
         AND ($4::TEXT IS NULL OR title ILIKE $4 ESCAPE '\\' OR description ILIKE $4 ESCAPE '\\') \
         AND ($5::TEXT IS NULL OR status = $5) \
         AND ($6::TEXT IS NULL OR priority = $6) \
         AND ($7::DATE IS NULL OR planned_date = $7) \
         AND ($8::DATE IS NULL OR due_date = $8) \
         AND ($9::TEXT IS NULL OR EXISTS \
              (SELECT 1 FROM task_tag_link AS link JOIN task_tag AS tag ON tag.id = link.tag_id \
               WHERE link.task_id = task.id AND lower(tag.name) = lower($9))) \
         ORDER BY task.created_at DESC, task.id DESC LIMIT 51 OFFSET $10"
    );
    let mut rows: Vec<TaskRow> = sqlx::query_as(&statement)
        .bind(scope)
        .bind(scope_id)
        .bind(include_subtasks)
        .bind(filters.keyword)
        .bind(filters.status)
        .bind(filters.priority)
        .bind(filters.planned_date)
        .bind(filters.due_date)
        .bind(filters.tag)
        .bind((page - 1) * 50)
        .fetch_all(pool)
        .await
        .map_err(|_| unavailable())?;
    let has_more = rows.len() > 50;
    rows.truncate(50);
    Ok(TaskPage {
        items: rows.into_iter().map(Into::into).collect(),
        page,
        has_more,
    })
}

#[utoipa::path(
    get, path = "/tasks", operation_id = "list_tasks", security(("admin_session" = [])),
    params(("scope" = String, Query, description = "inbox 或 project"), ("project_id" = Option<String>, Query), ("include_subtasks" = Option<bool>, Query), ("page" = Option<i64>, Query), ("keyword" = Option<String>, Query), ("status" = Option<TaskStatus>, Query), ("priority" = Option<TaskPriority>, Query), ("planned_date" = Option<String>, Query), ("due_date" = Option<String>, Query), ("tag" = Option<String>, Query)),
    responses((status = 200, body = TaskPage), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn list(
    State(state): State<PrivateState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> Result<Json<TaskPage>, TaskFailure> {
    let Query(query) = query.map_err(|_| invalid("查询参数无效"))?;
    let project_id = match (query.scope.as_str(), query.project_id.as_deref()) {
        ("inbox", None) => None,
        ("project", Some(id)) => Some(parse_project_id(id)?),
        _ => return Err(invalid("任务范围无效")),
    };
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid("页码无效"));
    }
    if let Some(id) = project_id {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM project WHERE id = $1)")
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| unavailable())?;
        if !exists {
            return Err(failure(
                StatusCode::NOT_FOUND,
                ErrorCode::NotFound,
                "项目不存在",
            ));
        }
    }
    let filters = validate_filters(query.filters)?;
    let scope = if project_id.is_some() {
        "project"
    } else {
        "inbox"
    };
    Ok(Json(
        search_tasks(
            &state.pool,
            scope,
            project_id,
            query.include_subtasks.unwrap_or(false),
            page,
            filters,
        )
        .await?,
    ))
}

#[utoipa::path(
    get, path = "/tasks/{id}/subtasks", operation_id = "list_subtasks", security(("admin_session" = [])),
    params(("id" = String, Path), ("page" = Option<i64>, Query), ("keyword" = Option<String>, Query), ("status" = Option<TaskStatus>, Query), ("priority" = Option<TaskPriority>, Query), ("planned_date" = Option<String>, Query), ("due_date" = Option<String>, Query), ("tag" = Option<String>, Query)),
    responses((status = 200, body = TaskPage), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn list_subtasks(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    query: Result<Query<SubtaskQuery>, QueryRejection>,
) -> Result<Json<TaskPage>, TaskFailure> {
    let id = parse_id(&id)?;
    let Query(query) = query.map_err(|_| invalid("查询参数无效"))?;
    let page = query.page.unwrap_or(1);
    if !(1..=100_000).contains(&page) {
        return Err(invalid("页码无效"));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM task WHERE id = $1)")
        .bind(id)
        .fetch_one(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    if !exists {
        return Err(missing());
    }
    let filters = validate_filters(query.filters)?;
    Ok(Json(
        search_tasks(&state.pool, "subtasks", Some(id), false, page, filters).await?,
    ))
}

#[derive(Deserialize)]
pub(super) struct SubtaskQuery {
    page: Option<i64>,
    #[serde(flatten)]
    filters: SearchFilters,
}

#[utoipa::path(
    post, path = "/tasks", operation_id = "create_task", security(("admin_session" = [])),
    params(("x-csrf-token" = String, Header)), request_body = CreateTask,
    responses((status = 201, body = TaskResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn create(
    State(state): State<PrivateState>,
    input: Result<Json<CreateTask>, JsonRejection>,
) -> Result<(StatusCode, Json<TaskResponse>), TaskFailure> {
    let Json(input) = input.map_err(|_| invalid("请求无效"))?;
    let title = title(&input.title)?;
    let description = description(input.description.unwrap_or_default())?;
    let project_id = input
        .project_id
        .as_deref()
        .map(parse_project_id)
        .transpose()?;
    let parent_id = input
        .parent_id
        .as_deref()
        .map(parse_parent_id)
        .transpose()?;
    let in_backlog = input.in_backlog.unwrap_or(false);
    if in_backlog && project_id.is_none() {
        return Err(invalid("收件箱任务不能进入待规划区"));
    }
    let planned_date = date(input.planned_date)?;
    let due_date = date(input.due_date)?;
    let tags = normalize_tags(input.tags.unwrap_or_default())?;
    let status = input.status.unwrap_or(TaskStatus::Todo);
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    if let Some(id) = parent_id {
        let parent = parent_task(&mut tx, id).await?;
        if parent.parent_id.is_some() {
            return Err(invalid("子任务不能再拆分"));
        }
        if parent.project_id != project_id {
            return Err(invalid("子任务与父任务的项目必须一致"));
        }
    }
    if let Some(id) = project_id {
        project_name(&mut tx, id, true).await?;
    }
    let statement = format!(
        "INSERT INTO task (parent_id, project_id, title, description, status, priority, planned_date, due_date, in_backlog) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {FIELDS}"
    );
    let row: TaskRow = sqlx::query_as(&statement)
        .bind(parent_id)
        .bind(project_id)
        .bind(title)
        .bind(description)
        .bind(status.as_str())
        .bind(input.priority.map(TaskPriority::as_str))
        .bind(planned_date)
        .bind(due_date)
        .bind(in_backlog)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    replace_tags(&mut tx, row.id, &tags).await?;
    if status == TaskStatus::Completed {
        record_completion(&mut tx, &row).await?;
    }
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1");
    let saved: TaskRow = sqlx::query_as(&statement)
        .bind(row.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok((StatusCode::CREATED, Json(saved.into())))
}

#[utoipa::path(
    get, path = "/tasks/{id}", operation_id = "get_task", security(("admin_session" = [])), params(("id" = String, Path)),
    responses((status = 200, body = TaskResponse), (status = 401, body = ApiError), (status = 404, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn detail(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
) -> Result<Json<TaskResponse>, TaskFailure> {
    let id = parse_id(&id)?;
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1");
    let row: Option<TaskRow> = sqlx::query_as(&statement)
        .bind(id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| unavailable())?;
    row.map(|row| Json(row.into())).ok_or_else(missing)
}

#[utoipa::path(
    patch, path = "/tasks/{id}", operation_id = "update_task", security(("admin_session" = [])),
    description = "缺省字段保持原值；parent_id、project_id、description、priority、planned_date、due_date 的 null 清空；tags 传数组替换所有任务标签；title、status、in_backlog 的 null 与缺省等效。移动到收件箱时，若未指定 in_backlog，会自动离开待规划区。",
    params(("id" = String, Path), ("x-csrf-token" = String, Header)), request_body = UpdateTask,
    responses((status = 200, body = TaskResponse), (status = 400, body = ApiError), (status = 401, body = ApiError), (status = 403, body = ApiError), (status = 404, body = ApiError), (status = 409, body = ApiError), (status = 503, body = ApiError))
)]
pub(super) async fn update(
    State(state): State<PrivateState>,
    Path(id): Path<String>,
    input: Result<Json<UpdateTask>, JsonRejection>,
) -> Result<Json<TaskResponse>, TaskFailure> {
    let id = parse_id(&id)?;
    let Json(input) = input.map_err(|_| invalid("请求无效"))?;
    let requested_tags = input.tags.map(normalize_tags).transpose()?;
    if input.expected_version < 1 {
        return Err(invalid("版本无效"));
    }
    let mut tx = state.pool.begin().await.map_err(|_| unavailable())?;
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1 FOR UPDATE");
    let mut row: TaskRow = sqlx::query_as(&statement)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(missing)?;
    if row.version != input.expected_version {
        return Err(failure(
            StatusCode::CONFLICT,
            ErrorCode::VersionConflict,
            "任务已变更，请刷新后重试",
        ));
    }
    let previous = row.clone();
    if let Some(value) = input.title {
        row.title = title(&value)?;
    }
    if let Some(value) = input.parent_id {
        row.parent_id = value.as_deref().map(parse_parent_id).transpose()?;
    }
    if let Some(value) = input.description {
        row.description = description(value.unwrap_or_default())?;
    }
    if let Some(value) = input.project_id {
        row.project_id = value.as_deref().map(parse_project_id).transpose()?;
        if let Some(project_id) = row.project_id {
            project_name(&mut tx, project_id, true).await?;
        } else if input.in_backlog.is_none() {
            row.in_backlog = false;
        }
    }
    if let Some(value) = input.status {
        row.status = value.as_str().into();
    }
    if let Some(value) = input.priority {
        row.priority = value.map(|priority| priority.as_str().into());
    }
    if let Some(value) = input.planned_date {
        row.planned_date = date(value)?;
    }
    if let Some(value) = input.due_date {
        row.due_date = date(value)?;
    }
    if let Some(value) = input.in_backlog {
        row.in_backlog = value;
    }
    if let Some(tags) = &requested_tags {
        row.tags = tags.clone();
    }
    if row.project_id.is_none() && row.in_backlog {
        return Err(invalid("收件箱任务不能进入待规划区"));
    }
    if row.parent_id != previous.parent_id || row.project_id != previous.project_id {
        validate_hierarchy(&mut tx, &row).await?;
    }
    if row == previous {
        return Ok(Json(row.into()));
    }
    let newly_completed = previous.status != "completed" && row.status == "completed";
    let statement = format!(
        "UPDATE task SET parent_id = $1, project_id = $2, title = $3, description = $4, status = $5, priority = $6, planned_date = $7, due_date = $8, in_backlog = $9, version = version + 1, updated_at = now() WHERE id = $10 AND version = $11 RETURNING {FIELDS}"
    );
    let saved: TaskRow = sqlx::query_as(&statement)
        .bind(row.parent_id)
        .bind(row.project_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(&row.priority)
        .bind(row.planned_date)
        .bind(row.due_date)
        .bind(row.in_backlog)
        .bind(id)
        .bind(input.expected_version)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    if let Some(tags) = &requested_tags {
        replace_tags(&mut tx, id, tags).await?;
    }
    if newly_completed {
        record_completion(&mut tx, &saved).await?;
    }
    let statement = format!("SELECT {FIELDS} FROM task WHERE id = $1");
    let saved: TaskRow = sqlx::query_as(&statement)
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(saved.into()))
}
