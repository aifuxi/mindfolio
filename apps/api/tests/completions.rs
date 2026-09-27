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

async fn session(pool: &PgPool) -> (axum::Router, String, String) {
    let config = mindfolio_api::AuthConfig::local();
    mindfolio_api::initialize_admin(pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    let app = mindfolio_api::app_with_config(pool.clone(), config);
    let login = send(
        &app,
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
        Some(cookie),
        Some(csrf),
        Some(ORIGIN),
        Some(body),
    )
    .await
}

#[sqlx::test]
async fn 完成历史跨日期且删除后保留快照(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    for path in ["/task-completions", "/task-completions/days"] {
        assert_eq!(
            send(&app, "GET", path, None, None, None, None)
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    for path in ["/tasks/1", "/projects/1"] {
        assert_eq!(
            send(
                &app,
                "DELETE",
                path,
                None,
                None,
                Some(ORIGIN),
                Some(json!({"expected_version":1})),
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let project = json_body(
        write(
            &app,
            "POST",
            "/projects",
            &cookie,
            &csrf,
            json!({"name":"旧项目"}),
        )
        .await,
    )
    .await;
    let project_id = project["id"].as_str().unwrap();
    let parent = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"父任务", "project_id":project_id}),
        )
        .await,
    )
    .await;
    let parent_id = parent["id"].as_str().unwrap();
    let child = json_body(write(&app, "POST", "/tasks", &cookie, &csrf, json!({"title":"旧子任务", "project_id":project_id, "parent_id":parent_id, "status":"completed"})).await).await;
    let child_id = child["id"].as_str().unwrap();
    let first = json_body(
        send(
            &app,
            "GET",
            "/task-completions",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(first["items"].as_array().unwrap().len(), 1);
    let first_id = first["items"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    sqlx::query("UPDATE task_completion SET completed_at = '2026-09-27 15:59:59+00' WHERE id = $1")
        .bind(first_id)
        .execute(&pool)
        .await
        .unwrap();

    let reopened = json_body(
        write(
            &app,
            "PATCH",
            &format!("/tasks/{child_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":1,"status":"todo","title":"新子任务"}),
        )
        .await,
    )
    .await;
    assert_eq!(reopened["version"], 2);
    let completed = json_body(
        write(
            &app,
            "PATCH",
            &format!("/tasks/{child_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":2,"status":"completed"}),
        )
        .await,
    )
    .await;
    assert_eq!(completed["version"], 3);
    let repeated = json_body(
        write(
            &app,
            "PATCH",
            &format!("/tasks/{child_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":3,"status":"completed"}),
        )
        .await,
    )
    .await;
    assert_eq!(repeated["version"], 3);
    let second_id: i64 =
        sqlx::query_scalar::<_, Option<i64>>("SELECT max(id) FROM task_completion")
            .fetch_one(&pool)
            .await
            .unwrap()
            .unwrap();
    sqlx::query("UPDATE task_completion SET completed_at = '2026-09-27 16:00:00+00' WHERE id = $1")
        .bind(second_id)
        .execute(&pool)
        .await
        .unwrap();

    let renamed = json_body(
        write(
            &app,
            "PATCH",
            &format!("/projects/{project_id}"),
            &cookie,
            &csrf,
            json!({"name":"新项目", "expected_version":1}),
        )
        .await,
    )
    .await;
    assert_eq!(renamed["version"], 2);
    let parent_done = json_body(
        write(
            &app,
            "PATCH",
            &format!("/tasks/{parent_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":1,"status":"completed"}),
        )
        .await,
    )
    .await;
    assert_eq!(parent_done["version"], 2);
    sqlx::query(
        "UPDATE task_completion SET completed_at = '2026-09-28 16:00:00+00' WHERE task_id = $1",
    )
    .bind(parent_id.parse::<i64>().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let day27 = json_body(
        send(
            &app,
            "GET",
            "/task-completions?date=2026-09-27",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(day27["items"].as_array().unwrap().len(), 1);
    assert_eq!(day27["items"][0]["task_title"], "旧子任务");
    assert_eq!(day27["items"][0]["project_name"], "旧项目");
    let day28 = json_body(
        send(
            &app,
            "GET",
            "/task-completions?date=2026-09-28",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(day28["items"].as_array().unwrap().len(), 1);
    assert_eq!(day28["items"][0]["task_title"], "新子任务");
    assert_eq!(day28["items"][0]["business_date"], "2026-09-28");
    let days = json_body(
        send(
            &app,
            "GET",
            "/task-completions/days",
            Some(&cookie),
            None,
            None,
            None,
        )
        .await,
    )
    .await;
    assert!(
        days["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|day| day["business_date"] == "2026-09-28" && day["completed_count"] == 1)
    );
    assert!(
        days["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|day| day["business_date"] == "2026-09-27" && day["completed_count"] == 1)
    );
    assert_eq!(
        send(
            &app,
            "GET",
            "/task-completions?date=2026-02-30",
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );

    let bad_delete = send(
        &app,
        "DELETE",
        &format!("/tasks/{child_id}"),
        Some(&cookie),
        None,
        Some(ORIGIN),
        Some(json!({"expected_version":3})),
    )
    .await;
    assert_eq!(bad_delete.status(), StatusCode::FORBIDDEN);
    let stale_delete = write(
        &app,
        "DELETE",
        &format!("/tasks/{child_id}"),
        &cookie,
        &csrf,
        json!({"expected_version":2}),
    )
    .await;
    assert_eq!(stale_delete.status(), StatusCode::CONFLICT);
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/tasks/{child_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":3})
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{child_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_body(
            send(
                &app,
                "GET",
                "/task-completions?date=2026-09-28",
                Some(&cookie),
                None,
                None,
                None
            )
            .await
        )
        .await["items"][0]["task_title"],
        "新子任务"
    );

    let other_child = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"待删除子任务", "project_id":project_id, "parent_id":parent_id}),
        )
        .await,
    )
    .await;
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/projects/{project_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{}", other_child["id"].as_str().unwrap()),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/projects/{project_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":2})
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/projects/{project_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{parent_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{}", other_child["id"].as_str().unwrap()),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_body(
            send(
                &app,
                "GET",
                "/task-completions?date=2026-09-27",
                Some(&cookie),
                None,
                None,
                None
            )
            .await
        )
        .await["items"][0]["project_name"],
        "旧项目"
    );
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/projects/{project_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":2})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test]
async fn 锁冲突时删除返回冲突且不产生部分删除(pool: PgPool) {
    let (app, cookie, csrf) = session(&pool).await;
    let project = json_body(
        write(
            &app,
            "POST",
            "/projects",
            &cookie,
            &csrf,
            json!({"name":"并发项目"}),
        )
        .await,
    )
    .await;
    let project_id = project["id"].as_str().unwrap();
    let parent = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"父任务", "project_id":project_id}),
        )
        .await,
    )
    .await;
    let parent_id = parent["id"].as_str().unwrap();
    let child = json_body(
        write(
            &app,
            "POST",
            "/tasks",
            &cookie,
            &csrf,
            json!({"title":"子任务", "project_id":project_id, "parent_id":parent_id}),
        )
        .await,
    )
    .await;
    let child_id = child["id"].as_str().unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM task WHERE id = $1 FOR UPDATE")
        .bind(child_id.parse::<i64>().unwrap())
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/tasks/{parent_id}"),
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
            "DELETE",
            &format!("/projects/{project_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{parent_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{child_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        write(
            &app,
            "DELETE",
            &format!("/tasks/{parent_id}"),
            &cookie,
            &csrf,
            json!({"expected_version":1})
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/tasks/{child_id}"),
            Some(&cookie),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}
