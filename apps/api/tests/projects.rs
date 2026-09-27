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
    let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test]
async fn 项目访问边界与生命周期(pool: PgPool) {
    let config = mindfolio_api::AuthConfig::local();
    mindfolio_api::initialize_admin(&pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    let app = mindfolio_api::app_with_config(pool.clone(), config);

    for (method, path, body) in [
        ("GET", "/projects", None),
        ("POST", "/projects", Some(json!({"name": "私人项目"}))),
        ("GET", "/projects/1", None),
        (
            "PATCH",
            "/projects/1",
            Some(json!({"name": "修改", "expected_version": 1})),
        ),
    ] {
        let response = send(&app, method, path, Some(ORIGIN), None, None, body).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(json_body(response).await["code"], "unauthorized");
    }
    assert_eq!(
        send(&app, "GET", "/health/live", None, None, None, None)
            .await
            .status(),
        StatusCode::OK
    );

    let login = send(
        &app,
        "POST",
        "/auth/login",
        Some(ORIGIN),
        None,
        None,
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

    for (origin, token) in [
        (Some(ORIGIN), None),
        (Some(ORIGIN), Some("invalid")),
        (Some("https://other.example"), Some(csrf.as_str())),
        (None, Some(csrf.as_str())),
    ] {
        let response = send(
            &app,
            "POST",
            "/projects",
            origin,
            Some(&cookie),
            token,
            Some(json!({"name":"私人项目"})),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(json_body(response).await["code"], "forbidden");
    }

    let created = send(
        &app,
        "POST",
        "/projects",
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"  私人项目  "})),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let project = json_body(created).await;
    let id = project["id"].as_str().unwrap();
    assert!(id.parse::<i64>().unwrap() > 0);
    assert_eq!(project["name"], "私人项目");
    assert_eq!(project["version"], 1);
    assert!(project["completed_at"].is_null());
    let detail = send(
        &app,
        "GET",
        &format!("/projects/{id}"),
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(detail.status(), StatusCode::OK);
    assert_eq!(json_body(detail).await["name"], "私人项目");
    let list = send(&app, "GET", "/projects", None, Some(&cookie), None, None).await;
    let list = json_body(list).await;
    assert_eq!(list["items"][0]["id"], id);
    let invalid_page = send(
        &app,
        "GET",
        "/projects?page=abc",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(invalid_page.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(invalid_page).await["code"], "invalid_request");

    let invalid = send(
        &app,
        "POST",
        "/projects",
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"  "})),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let renamed = send(
        &app,
        "PATCH",
        &format!("/projects/{id}"),
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"改名后", "expected_version":1})),
    )
    .await;
    assert_eq!(renamed.status(), StatusCode::OK);
    assert_eq!(json_body(renamed).await["version"], 2);
    let stale = send(
        &app,
        "PATCH",
        &format!("/projects/{id}"),
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"旧版本", "expected_version":1})),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(json_body(stale).await["code"], "version_conflict");
    let missing = send(
        &app,
        "GET",
        "/projects/999999999",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let completed = send(
        &app,
        "POST",
        &format!("/projects/{id}/complete"),
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":2})),
    )
    .await;
    assert_eq!(completed.status(), StatusCode::OK);
    let completed = json_body(completed).await;
    assert!(completed["completed_at"].is_string());
    assert_eq!(completed["version"], 3);
    let archived = send(
        &app,
        "POST",
        &format!("/projects/{id}/archive"),
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":3})),
    )
    .await;
    assert_eq!(archived.status(), StatusCode::OK);
    assert!(json_body(archived).await["archived_at"].is_string());
    let active = send(&app, "GET", "/projects", None, Some(&cookie), None, None).await;
    assert!(
        json_body(active).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let all = send(
        &app,
        "GET",
        "/projects?include_archived=true",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(json_body(all).await["items"][0]["id"], id);
    let restored = send(
        &app,
        "POST",
        &format!("/projects/{id}/restore"),
        Some(ORIGIN),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":4})),
    )
    .await;
    assert_eq!(restored.status(), StatusCode::OK);
    let restored = json_body(restored).await;
    assert_eq!(restored["completed_at"], completed["completed_at"]);
    assert!(restored["archived_at"].is_null());
    assert_eq!(restored["name"], "改名后");

    sqlx::query("UPDATE admin_session SET revoked_at = now() WHERE revoked_at IS NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        send(&app, "GET", "/projects", None, Some(&cookie), None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/projects",
            Some(ORIGIN),
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"另一个"}))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn 项目迁移建立数据库约束和索引(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SAVEPOINT invalid_name")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invalid = sqlx::query("INSERT INTO project (name) VALUES ('  ')")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert_eq!(
        invalid.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT invalid_name")
        .execute(&mut *tx)
        .await
        .unwrap();
    let id: i64 = sqlx::query_scalar("INSERT INTO project (name) VALUES ('测试') RETURNING id")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(id > 0);
    tx.rollback().await.unwrap();
    let indexes: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE tablename = 'project' AND indexname IN ('project_active_order_idx', 'project_archived_order_idx')").fetch_one(&pool).await.unwrap();
    assert_eq!(indexes, 2);
}
