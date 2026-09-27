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
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
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

#[sqlx::test]
async fn 一级子任务限制层级归属和移动且保持原子性(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    let project = write(
        &app,
        "POST",
        "/projects",
        &cookie,
        &csrf,
        json!({"name":"项目甲"}),
    )
    .await;
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();
    let another = write(
        &app,
        "POST",
        "/projects",
        &cookie,
        &csrf,
        json!({"name":"项目乙"}),
    )
    .await;
    let another_id = json_body(another).await["id"].as_str().unwrap().to_string();
    let parent = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"父任务", "project_id":project_id}),
    )
    .await;
    assert_eq!(parent.status(), StatusCode::CREATED);
    let parent_id = json_body(parent).await["id"].as_str().unwrap().to_string();

    let forbidden = send(
        &app,
        "GET",
        &format!("/tasks/{parent_id}/subtasks"),
        None,
        None,
        None,
        None,
    )
    .await;
    assert_eq!(forbidden.status(), StatusCode::UNAUTHORIZED);
    let wrong_csrf = send(
        &app,
        "POST",
        "/tasks",
        Some(ORIGIN),
        Some(&cookie),
        Some("wrong"),
        Some(json!({"title":"拒绝创建", "parent_id":parent_id, "project_id":project_id})),
    )
    .await;
    assert_eq!(wrong_csrf.status(), StatusCode::FORBIDDEN);
    let wrong_origin = send(
        &app,
        "POST",
        "/tasks",
        Some("https://other.example"),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"title":"拒绝创建", "parent_id":parent_id, "project_id":project_id})),
    )
    .await;
    assert_eq!(wrong_origin.status(), StatusCode::FORBIDDEN);

    let child = write(
        &app, "POST", "/tasks", &cookie, &csrf,
        json!({"title":"步骤一", "parent_id":parent_id, "project_id":project_id, "planned_date":"2026-09-28", "due_date":"2026-09-30"}),
    ).await;
    assert_eq!(child.status(), StatusCode::CREATED);
    let child = json_body(child).await;
    let child_id = child["id"].as_str().unwrap().to_string();
    assert_eq!(child["parent_id"], parent_id);
    assert_eq!(child["planned_date"], "2026-09-28");
    assert_eq!(child["due_date"], "2026-09-30");
    let top_level = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(
        json_body(top_level).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let children = send(
        &app,
        "GET",
        &format!("/tasks/{parent_id}/subtasks"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(children).await["items"][0]["id"], child_id);

    let grandchild = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"不允许的第二级", "parent_id":child_id, "project_id":project_id}),
    )
    .await;
    assert_eq!(grandchild.status(), StatusCode::BAD_REQUEST);
    let mismatch = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"不同项目", "parent_id":parent_id, "project_id":another_id}),
    )
    .await;
    assert_eq!(mismatch.status(), StatusCode::BAD_REQUEST);
    let cycle = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "parent_id":child_id, "status":"completed"}),
    )
    .await;
    assert_eq!(cycle.status(), StatusCode::BAD_REQUEST);
    let move_parent = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "project_id":another_id}),
    )
    .await;
    assert_eq!(move_parent.status(), StatusCode::BAD_REQUEST);
    let move_child = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "project_id":another_id}),
    )
    .await;
    assert_eq!(move_child.status(), StatusCode::BAD_REQUEST);
    let stale = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2, "status":"in_progress"}),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let unchanged_parent = send(
        &app,
        "GET",
        &format!("/tasks/{parent_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    let unchanged_child = send(
        &app,
        "GET",
        &format!("/tasks/{child_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(unchanged_parent).await["project_id"], project_id);
    let unchanged_child = json_body(unchanged_child).await;
    assert_eq!(unchanged_child["project_id"], project_id);
    assert_eq!(unchanged_child["version"], 1);
    let completion_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
            .bind(parent_id.parse::<i64>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(completion_count, 0);

    let detach = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "parent_id":null}),
    )
    .await;
    assert_eq!(detach.status(), StatusCode::OK);
    let move_parent = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "project_id":another_id}),
    )
    .await;
    assert_eq!(move_parent.status(), StatusCode::OK);
    let reattach = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2, "parent_id":parent_id, "project_id":another_id}),
    )
    .await;
    assert_eq!(reattach.status(), StatusCode::OK);
}

#[sqlx::test]
async fn 子任务完成事实独立于父任务(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    let parent = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"父任务"}),
    )
    .await;
    let parent_id = json_body(parent).await["id"].as_str().unwrap().to_string();
    let child = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"子任务", "parent_id":parent_id, "planned_date":"2026-09-29"}),
    )
    .await;
    let child_id = json_body(child).await["id"].as_str().unwrap().to_string();
    let complete_parent = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "status":"completed"}),
    )
    .await;
    assert_eq!(complete_parent.status(), StatusCode::OK);
    let child_current = send(
        &app,
        "GET",
        &format!("/tasks/{child_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(child_current).await["status"], "todo");
    let complete = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "status":"completed", "due_date":"2026-09-30"}),
    )
    .await;
    assert_eq!(complete.status(), StatusCode::OK);
    assert_eq!(json_body(complete).await["due_date"], "2026-09-30");
    let repeat = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2, "status":"completed"}),
    )
    .await;
    assert_eq!(repeat.status(), StatusCode::OK);
    let reopened = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2, "status":"todo"}),
    )
    .await;
    assert_eq!(reopened.status(), StatusCode::OK);
    let again = write(
        &app,
        "PATCH",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":3, "status":"completed"}),
    )
    .await;
    assert_eq!(again.status(), StatusCode::OK);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM task_completion WHERE task_id = $1")
        .bind(child_id.parse::<i64>().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test]
async fn 子任务数据库约束保护一级层级和项目归属(pool: PgPool) {
    let parent: i64 = sqlx::query_scalar("INSERT INTO task (title) VALUES ('父任务') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let child: i64 = sqlx::query_scalar(
        "INSERT INTO task (title, parent_id) VALUES ('子任务', $1) RETURNING id",
    )
    .bind(parent)
    .fetch_one(&pool)
    .await
    .unwrap();
    let project: i64 =
        sqlx::query_scalar("INSERT INTO project (name) VALUES ('项目') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut tx = pool.begin().await.unwrap();
    for (savepoint, statement) in [
        (
            "grandchild",
            format!("INSERT INTO task (title, parent_id) VALUES ('第二级', {child})"),
        ),
        (
            "cycle",
            format!("UPDATE task SET parent_id = {child} WHERE id = {parent}"),
        ),
        (
            "self_parent",
            format!("UPDATE task SET parent_id = {parent} WHERE id = {parent}"),
        ),
        (
            "child_project",
            format!("UPDATE task SET project_id = {project} WHERE id = {child}"),
        ),
        (
            "parent_project",
            format!("UPDATE task SET project_id = {project} WHERE id = {parent}"),
        ),
    ] {
        sqlx::query(&format!("SAVEPOINT {savepoint}"))
            .execute(&mut *tx)
            .await
            .unwrap();
        let error = sqlx::query(&statement).execute(&mut *tx).await.unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
        sqlx::query(&format!("ROLLBACK TO SAVEPOINT {savepoint}"))
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.rollback().await.unwrap();
    let index_count: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE tablename = 'task' AND indexname = 'task_parent_order_idx'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(index_count, 1);
}

#[sqlx::test]
async fn 任务标签和组合筛选覆盖项目收件箱及子任务(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    let project = write(
        &app,
        "POST",
        "/projects",
        &cookie,
        &csrf,
        json!({"name":"检索项目"}),
    )
    .await;
    let project_id = json_body(project).await["id"].as_str().unwrap().to_string();
    let parent = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"撰写方案 100%", "description":"包含技术路线", "project_id":project_id,
               "status":"in_progress", "priority":"high", "planned_date":"2026-09-28",
               "due_date":"2026-09-30", "in_backlog":true, "tags":["研究", "紧急"]}),
    )
    .await;
    assert_eq!(parent.status(), StatusCode::CREATED);
    let parent = json_body(parent).await;
    let parent_id = parent["id"].as_str().unwrap().to_string();
    assert_eq!(parent["tags"], json!(["研究", "紧急"]));
    let child = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"查找资料", "parent_id":parent_id, "project_id":project_id,
               "status":"todo", "planned_date":"2026-09-28", "tags":["研究"]}),
    )
    .await;
    assert_eq!(child.status(), StatusCode::CREATED);
    let child_id = json_body(child).await["id"].as_str().unwrap().to_string();
    let inbox = write(
        &app,
        "POST",
        "/tasks",
        &cookie,
        &csrf,
        json!({"title":"收件箱资料", "tags":["研究"]}),
    )
    .await;
    assert_eq!(inbox.status(), StatusCode::CREATED);

    let query = format!(
        "/tasks?scope=project&project_id={project_id}&include_subtasks=true&keyword=%E8%B5%84%E6%96%99&planned_date=2026-09-28&tag=%E7%A0%94%E7%A9%B6"
    );
    let matching = send(&app, "GET", &query, None, Some(&cookie), None, None).await;
    assert_eq!(matching.status(), StatusCode::OK);
    let matching = json_body(matching).await;
    assert_eq!(matching["items"].as_array().unwrap().len(), 1);
    assert_eq!(matching["items"][0]["id"], child_id);
    let all_project = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}&include_subtasks=true"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(
        json_body(all_project).await["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let top_only = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(
        json_body(top_only).await["items"].as_array().unwrap().len(),
        1
    );
    let inbox = send(
        &app,
        "GET",
        "/tasks?scope=inbox&tag=%E7%A0%94%E7%A9%B6",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(inbox).await["items"].as_array().unwrap().len(), 1);
    let child_only = send(
        &app,
        "GET",
        &format!("/tasks/{parent_id}/subtasks?status=todo&tag=%E7%A0%94%E7%A9%B6"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(child_only).await["items"][0]["id"], child_id);
    let combined = send(&app, "GET", &format!("/tasks?scope=project&project_id={project_id}&status=in_progress&priority=high&planned_date=2026-09-28&due_date=2026-09-30&tag=%E7%A0%94%E7%A9%B6"), None, Some(&cookie), None, None).await;
    assert_eq!(json_body(combined).await["items"][0]["id"], parent_id);
    let literal_percent = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}&keyword=%25"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(
        json_body(literal_percent).await["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let forbidden = send(&app, "GET", &query, None, None, None, None).await;
    assert_eq!(forbidden.status(), StatusCode::UNAUTHORIZED);

    let changed = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "tags":["资料"]}),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    assert_eq!(json_body(changed).await["tags"], json!(["资料"]));
    let old_tag = send(
        &app,
        "GET",
        &format!("/tasks?scope=project&project_id={project_id}&tag=%E7%A0%94%E7%A9%B6"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert!(
        json_body(old_tag).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let duplicate = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2, "title":"不会保存", "tags":["资料", "资料"]}),
    )
    .await;
    assert_eq!(duplicate.status(), StatusCode::BAD_REQUEST);
    let current = send(
        &app,
        "GET",
        &format!("/tasks/{parent_id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    let current = json_body(current).await;
    assert_eq!(current["title"], "撰写方案 100%");
    assert_eq!(current["version"], 2);
    let stale = write(
        &app,
        "PATCH",
        &format!("/tasks/{parent_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":1, "tags":[]}),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
}

#[sqlx::test]
async fn 任务筛选分页限制和稳定顺序(pool: PgPool) {
    let (app, cookie, _csrf) = session(&pool).await;
    sqlx::query("INSERT INTO task (title, created_at) SELECT '分页任务 ' || n, TIMESTAMPTZ '2026-09-27 00:00:00+00' FROM generate_series(1, 55) AS n")
        .execute(&pool).await.unwrap();
    let first = send(
        &app,
        "GET",
        "/tasks?scope=inbox&page=1",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    let first = json_body(first).await;
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["has_more"], true);
    let second = send(
        &app,
        "GET",
        "/tasks?scope=inbox&page=2",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    let second = json_body(second).await;
    assert_eq!(second["items"].as_array().unwrap().len(), 5);
    assert_eq!(second["has_more"], false);
    let first_last: i64 = first["items"][49]["id"].as_str().unwrap().parse().unwrap();
    let second_first: i64 = second["items"][0]["id"].as_str().unwrap().parse().unwrap();
    assert!(first_last > second_first);
    for query in [
        "page=0",
        "page=100001",
        "planned_date=2026-99-99",
        "status=unknown",
    ] {
        let invalid = send(
            &app,
            "GET",
            &format!("/tasks?scope=inbox&{query}"),
            None,
            Some(&cookie),
            None,
            None,
        )
        .await;
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    }
}
