use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:5173";
const PASSWORD: &str = "correct horse battery staple";

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    origin: Option<&str>,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<Value>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
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
    let body = to_bytes(response.into_body(), 8192).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn session(pool: &PgPool) -> (axum::Router, String, String) {
    let config = mindfolio_api::AuthConfig::local();
    mindfolio_api::initialize_admin(pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    let app = mindfolio_api::app_with_config(pool.clone(), config);
    let response = send(
        &app,
        "POST",
        "/auth/login",
        Some(ORIGIN),
        None,
        None,
        Some(json!({"username":"owner", "password":PASSWORD})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let csrf = json_body(response).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    (app, cookie, csrf)
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
        Some(ORIGIN),
        Some(cookie),
        Some(csrf),
        Some(body),
    )
    .await
}

#[sqlx::test]
async fn 收件箱移动字段清空及完成重试(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    assert_eq!(
        send(&app, "GET", "/tasks?scope=inbox", None, None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/tasks",
            Some(ORIGIN),
            None,
            None,
            Some(json!({"title":"未授权"}))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let project = write(
        &app,
        "POST",
        "/projects",
        &cookie,
        &csrf,
        json!({"name":"原项目"}),
    )
    .await;
    assert_eq!(project.status(), StatusCode::CREATED);
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();

    for (origin, token) in [
        (Some(ORIGIN), None),
        (Some(ORIGIN), Some("invalid")),
        (Some("https://other.example"), Some(csrf.as_str())),
    ] {
        let rejected = send(
            &app,
            "POST",
            "/tasks",
            origin,
            Some(&cookie),
            token,
            Some(json!({"title":"未授权"})),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);
        assert_eq!(json_body(rejected).await["code"], "forbidden");
    }
    let invalid_backlog = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"无项目", "in_backlog":true}),
    )
    .await;
    assert_eq!(invalid_backlog.status(), StatusCode::BAD_REQUEST);
    let invalid_date = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"无效日期", "planned_date":"2026-02-30"}),
    )
    .await;
    assert_eq!(invalid_date.status(), StatusCode::BAD_REQUEST);

    let created = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({
            "title":"  写总结  ", "description":"**正文**", "status":"todo", "priority":"high",
            "planned_date":"2026-09-27", "due_date":"2026-10-01"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let task = json_body(created).await;
    let id = task["id"].as_str().unwrap().to_string();
    assert!(id.parse::<i64>().unwrap() > 0);
    assert!(task["project_id"].is_null());
    assert_eq!(task["title"], "写总结");
    assert_eq!(task["description"], "**正文**");
    assert_eq!(task["planned_date"], "2026-09-27");
    let detail = send(
        &app,
        "GET",
        &format!("/tasks/{id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(detail).await["id"], id);
    let inbox = send(
        &app,
        "GET",
        "/tasks?scope=inbox",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(inbox).await["items"][0]["id"], id);

    let moved = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({
            "expected_version":1, "project_id":project_id, "in_backlog":true
        }),
    )
    .await;
    assert_eq!(moved.status(), StatusCode::OK);
    let moved = json_body(moved).await;
    assert_eq!(moved["project_id"], project_id);
    assert_eq!(moved["in_backlog"], true);
    assert_eq!(moved["status"], "todo");
    let inbox = send(
        &app,
        "GET",
        "/tasks?scope=inbox",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert!(
        json_body(inbox).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let project_list = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(project_list).await["items"][0]["id"], id);

    let cleared = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({
            "expected_version":2, "description":null, "priority":null,
            "planned_date":null, "due_date":null, "status":"in_progress"
        }),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared = json_body(cleared).await;
    assert_eq!(cleared["description"], "");
    assert!(cleared["priority"].is_null());
    assert!(cleared["planned_date"].is_null());
    assert!(cleared["due_date"].is_null());
    assert_eq!(cleared["project_id"], project_id);
    assert_eq!(cleared["in_backlog"], true);
    assert_eq!(cleared["status"], "in_progress");

    let canceled = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":3, "status":"canceled"}),
    )
    .await;
    assert_eq!(json_body(canceled).await["status"], "canceled");
    let todo = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":4, "status":"todo"}),
    )
    .await;
    assert_eq!(json_body(todo).await["status"], "todo");
    let completed = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":5, "status":"completed"}),
    )
    .await;
    assert_eq!(completed.status(), StatusCode::OK);
    assert_eq!(json_body(completed).await["version"], 6);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
        .bind(id.parse::<i64>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let repeated = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":6, "status":"completed"}),
    )
    .await;
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(json_body(repeated).await["version"], 6);
    let stale = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":5, "status":"completed"}),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(stale).await["code"], "version_conflict");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
        .bind(id.parse::<i64>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let renamed = write(
        &app,
        "PATCH",
        &format!("/projects/{project_id}"),
        &cookie,
        &csrf,
        json!({"name":"新项目", "expected_version":1}),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    let reopened = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":6, "status":"todo"}),
    )
    .await;
    assert_eq!(json_body(reopened).await["version"], 7);
    let completed_again = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":7, "status":"completed"}),
    )
    .await;
    assert_eq!(json_body(completed_again).await["version"], 8);
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT project_name FROM task_completion WHERE task_id = $1 ORDER BY id",
    )
    .bind(id.parse::<i64>().unwrap())
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(names, vec!["原项目", "新项目"]);

    let second_project = write(
        &app,
        "POST",
        "/projects",
        &cookie,
        &csrf,
        json!({"name":"第二项目"}),
    )
    .await;
    let second_project_id = json_body(second_project).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let moved_between_projects = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":8, "project_id":second_project_id, "in_backlog":true}),
    )
    .await;
    assert_eq!(moved_between_projects.status(), StatusCode::OK);
    assert_eq!(
        json_body(moved_between_projects).await["project_id"],
        second_project_id
    );
    let second_project_tasks = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={second_project_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(second_project_tasks).await["items"][0]["id"], id);

    let move_to_inbox = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":9, "project_id":null}),
    )
    .await;
    assert_eq!(move_to_inbox.status(), StatusCode::OK);
    let moved = json_body(move_to_inbox).await;
    assert!(moved["project_id"].is_null());
    assert_eq!(moved["in_backlog"], false);
    let missing = send(
        &app,
        "GET",
        "/tasks/999999999",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let invalid_query = send(
        &app,
        "GET",
        "/tasks?scope=project",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(invalid_query.status(), StatusCode::BAD_REQUEST);

    let initially_completed = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"直接完成", "status":"completed"}),
    )
    .await;
    assert_eq!(initially_completed.status(), StatusCode::CREATED);
    let completed_id = json_body(initially_completed).await["id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
        .bind(completed_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test]
async fn 完成记录失败时回滚任务状态(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    let created = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"事务任务"}),
    )
    .await;
    let id = json_body(created).await["id"].as_str().unwrap().to_string();
    sqlx::query("CREATE FUNCTION reject_task_completion() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION '测试完成记录失败'; END $$")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_task_completion BEFORE INSERT ON task_completion FOR EACH ROW EXECUTE FUNCTION reject_task_completion()")
        .execute(&pool).await.unwrap();
    let failed = write(
        &app,
        "PATCH",
        &format!("/tasks/{id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "status":"completed"}),
    )
    .await;
    assert_eq!(failed.status(), StatusCode::SERVICE_UNAVAILABLE);
    let current = send(
        &app,
        "GET",
        &format!("/tasks/{id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    let current = json_body(current).await;
    assert_eq!(current["status"], "todo");
    assert_eq!(current["version"], 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
        .bind(id.parse::<i64>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test]
async fn 任务数据库约束保护收件箱和状态(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SAVEPOINT backlog_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invalid = sqlx::query("INSERT INTO task (title, in_backlog) VALUES ('任务', true)")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT backlog_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SAVEPOINT status_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invalid = sqlx::query("INSERT INTO task (title, status) VALUES ('任务', 'unknown')")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT status_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SAVEPOINT project_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invalid = sqlx::query("INSERT INTO task (title, project_id) VALUES ('任务', 999999999)")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT project_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
}
