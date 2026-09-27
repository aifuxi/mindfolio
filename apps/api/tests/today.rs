use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use sqlx::{
    PgPool,
    types::chrono::{DateTime, Utc},
};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:5173";
const PASSWORD: &str = "correct horse battery staple";

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    origin: Option<&str>,
    body: Option<Value>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let body = if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    app.clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap()
}

fn fixed_app(pool: &PgPool, instant: &str) -> axum::Router {
    let instant = DateTime::parse_from_rfc3339(instant)
        .unwrap()
        .with_timezone(&Utc);
    mindfolio_api::app_with_config_and_today_clock(
        pool.clone(),
        mindfolio_api::AuthConfig::local(),
        mindfolio_api::TodayClock::fixed(instant),
    )
}

async fn session(pool: &PgPool, app: &axum::Router) -> (String, String) {
    mindfolio_api::initialize_admin(
        pool,
        "owner",
        PASSWORD.into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    let login = send(
        app,
        "POST",
        "/auth/login",
        None,
        None,
        Some(ORIGIN),
        Some(json!({"username":"owner", "password":PASSWORD})),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let csrf = json_body(login).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    (cookie, csrf)
}

async fn write(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> Response {
    send(
        app,
        method,
        path,
        Some(cookie),
        Some(csrf),
        Some(ORIGIN),
        Some(body),
    )
    .await
}

async fn insert_task(
    pool: &PgPool,
    title: &str,
    status: &str,
    planned_date: Option<&str>,
    due_date: Option<&str>,
    project_id: Option<i64>,
    parent_id: Option<i64>,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO task (title, status, planned_date, due_date, project_id, parent_id) \
         VALUES ($1, $2, $3::date, $4::date, $5, $6) RETURNING id",
    )
    .bind(title)
    .bind(status)
    .bind(planned_date)
    .bind(due_date)
    .bind(project_id)
    .bind(parent_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn find<'a>(items: &'a [Value], title: &str) -> Option<&'a Value> {
    items.iter().find(|item| item["task"]["title"] == title)
}

#[sqlx::test]
async fn 今日筛选按上海跨日且多条件任务只出现一次(pool: PgPool) {
    let before = fixed_app(&pool, "2026-09-27T15:59:59Z");
    let after = fixed_app(&pool, "2026-09-27T16:00:00Z");
    let (cookie, _) = session(&pool, &before).await;
    let project_id: i64 =
        sqlx::query_scalar("INSERT INTO project (name) VALUES ('项目甲') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let parent_id = insert_task(&pool, "父任务", "todo", None, None, Some(project_id), None).await;
    insert_task(
        &pool,
        "计划昨天",
        "todo",
        Some("2026-09-27"),
        None,
        None,
        None,
    )
    .await;
    insert_task(
        &pool,
        "昨天到期",
        "todo",
        None,
        Some("2026-09-27"),
        None,
        None,
    )
    .await;
    insert_task(
        &pool,
        "计划昨天且逾期",
        "in_progress",
        Some("2026-09-27"),
        Some("2026-09-26"),
        Some(project_id),
        None,
    )
    .await;
    insert_task(
        &pool,
        "今天到期",
        "todo",
        None,
        Some("2026-09-28"),
        Some(project_id),
        None,
    )
    .await;
    insert_task(
        &pool,
        "计划今天",
        "todo",
        Some("2026-09-28"),
        None,
        None,
        None,
    )
    .await;
    insert_task(
        &pool,
        "子任务昨天",
        "todo",
        Some("2026-09-27"),
        None,
        Some(project_id),
        Some(parent_id),
    )
    .await;
    insert_task(
        &pool,
        "已完成",
        "completed",
        Some("2026-09-27"),
        Some("2026-09-26"),
        None,
        None,
    )
    .await;
    insert_task(
        &pool,
        "已取消",
        "canceled",
        None,
        Some("2026-09-26"),
        None,
        None,
    )
    .await;
    insert_task(
        &pool,
        "未来任务",
        "todo",
        Some("2026-09-30"),
        Some("2026-10-01"),
        None,
        None,
    )
    .await;

    assert_eq!(
        send(&before, "GET", "/today/tasks", None, None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let day27 = json_body(
        send(
            &before,
            "GET",
            "/today/tasks",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(day27["business_date"], "2026-09-27");
    let items27 = day27["items"].as_array().unwrap();
    assert_eq!(items27.len(), 4);
    assert_eq!(items27[0]["task"]["title"], "计划昨天且逾期");
    assert_eq!(
        find(items27, "计划昨天且逾期").unwrap()["reasons"],
        json!(["overdue", "planned_today"])
    );
    assert_eq!(
        find(items27, "昨天到期").unwrap()["reasons"],
        json!(["due_today"])
    );
    assert_eq!(
        find(items27, "子任务昨天").unwrap()["task"]["parent_id"],
        parent_id.to_string()
    );
    assert!(find(items27, "已完成").is_none());
    assert!(find(items27, "已取消").is_none());

    let day28 = json_body(
        send(
            &after,
            "GET",
            "/today/tasks",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(day28["business_date"], "2026-09-28");
    let items28 = day28["items"].as_array().unwrap();
    assert_eq!(items28.len(), 4);
    assert_eq!(
        find(items28, "昨天到期").unwrap()["reasons"],
        json!(["overdue"])
    );
    assert_eq!(
        find(items28, "今天到期").unwrap()["reasons"],
        json!(["due_today"])
    );
    assert_eq!(
        find(items28, "计划今天").unwrap()["reasons"],
        json!(["planned_today"])
    );
    assert!(find(items28, "计划昨天").is_none());
    assert!(find(items28, "子任务昨天").is_none());
    assert_eq!(
        send(
            &after,
            "GET",
            "/today/tasks?page=0",
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test]
async fn 主动安排到今天保持截止日期并验证私人写入与版本(pool: PgPool) {
    let app = fixed_app(&pool, "2026-09-27T16:00:00Z");
    let (cookie, csrf) = session(&pool, &app).await;
    let project = json_body(
        write(
            &app,
            "POST",
            "/projects",
            &cookie,
            &csrf,
            json!({"name":"项目甲"}),
        )
        .await,
    )
    .await;
    let project_id = project["id"].as_str().unwrap();
    let inbox = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"收件箱事项", "due_date":"2026-10-01"}),
        )
        .await,
    )
    .await;
    let inbox_id = inbox["id"].as_str().unwrap();
    let project_task = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"项目事项", "project_id":project_id, "due_date":"2026-10-02"}),
        )
        .await,
    )
    .await;
    let project_task_id = project_task["id"].as_str().unwrap();
    let path = format!("/tasks/{inbox_id}/plan-today");
    assert_eq!(
        send(
            &app,
            "POST",
            &path,
            None,
            None,
            Some(ORIGIN),
            Some(json!({"expected_version":1}))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &path,
            Some(&cookie),
            None,
            Some(ORIGIN),
            Some(json!({"expected_version":1}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &path,
            Some(&cookie),
            Some(&csrf),
            Some("https://other.example"),
            Some(json!({"expected_version":1}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &path,
            Some(&cookie),
            Some(&csrf),
            Some(ORIGIN),
            Some(json!({"expected_version":0}))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let saved = json_body(
        write(
            &app,
            "POST",
            &path,
            &cookie,
            &csrf,
            json!({"expected_version":1}),
        )
        .await,
    )
    .await;
    assert_eq!(saved["planned_date"], "2026-09-28");
    assert_eq!(saved["due_date"], "2026-10-01");
    assert!(saved["project_id"].is_null());
    assert_eq!(saved["version"], 2);
    let repeated = json_body(
        write(
            &app,
            "POST",
            &path,
            &cookie,
            &csrf,
            json!({"expected_version":2}),
        )
        .await,
    )
    .await;
    assert_eq!(repeated["version"], 2);
    assert_eq!(
        write(
            &app,
            "POST",
            &path,
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        write(
            &app,
            "POST",
            &format!("/tasks/{project_task_id}/plan-today"),
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::OK
    );
    let project_saved = json_body(
        send(
            &app,
            "GET",
            &format!("/tasks/{project_task_id}"),
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(project_saved["project_id"], project_id);
    assert_eq!(project_saved["due_date"], "2026-10-02");
    assert_eq!(project_saved["planned_date"], "2026-09-28");
    let today =
        json_body(send(&app, "GET", "/today/tasks", Some(&cookie), None, None, None).await).await;
    assert_eq!(today["items"].as_array().unwrap().len(), 2);
    let completion_count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(completion_count, 0);
    assert_eq!(
        write(
            &app,
            "POST",
            "/tasks/999999/plan-today",
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let completed = json_body(
        write(
            &app,
            "PATCH",
            &format!("/tasks/{project_task_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":2,"status":"completed"}),
        )
        .await,
    )
    .await;
    assert_eq!(completed["version"], 3);
    assert_eq!(
        write(
            &app,
            "POST",
            &format!("/tasks/{project_task_id}/plan-today"),
            &cookie,
            &csrf,
            json!({"expected_version":3})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
}

#[sqlx::test]
async fn 今日任务分页稳定且无重复(pool: PgPool) {
    let app = fixed_app(&pool, "2026-09-27T16:00:00Z");
    let (cookie, _) = session(&pool, &app).await;
    for index in 0..55 {
        insert_task(
            &pool,
            &format!("任务 {index}"),
            "todo",
            Some("2026-09-28"),
            None,
            None,
            None,
        )
        .await;
    }
    let first = json_body(
        send(
            &app,
            "GET",
            "/today/tasks?page=1",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    let second = json_body(
        send(
            &app,
            "GET",
            "/today/tasks?page=2",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["has_more"], true);
    assert_eq!(second["items"].as_array().unwrap().len(), 5);
    assert_eq!(second["has_more"], false);
    let ids: std::collections::HashSet<&str> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .map(|item| item["task"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 55);
    let repeated = json_body(
        send(
            &app,
            "GET",
            "/today/tasks?page=1",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(repeated["items"], first["items"]);
}
