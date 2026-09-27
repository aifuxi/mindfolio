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

fn app(pool: &PgPool, instant: &str) -> axum::Router {
    mindfolio_api::app_with_config_and_today_clock(
        pool.clone(),
        mindfolio_api::AuthConfig::local(),
        mindfolio_api::TodayClock::fixed(
            DateTime::parse_from_rfc3339(instant)
                .unwrap()
                .with_timezone(&Utc),
        ),
    )
}

async fn send(
    app: &axum::Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Option<Value>,
) -> Response {
    let mut request = Request::builder().method(method).uri(path);
    if method != "GET" {
        request = request.header(header::ORIGIN, ORIGIN);
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

async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap()
}

async fn session(pool: &PgPool, app: &axum::Router) -> (String, String) {
    mindfolio_api::initialize_admin(
        pool,
        "owner",
        "correct horse battery staple".into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    let response = send(
        app,
        "POST",
        "/auth/login",
        None,
        None,
        Some(json!({"username":"owner","password":"correct horse battery staple"})),
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
    let csrf = body(response).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    (cookie, csrf)
}

#[sqlx::test]
async fn 习惯设定暂停版本和历史(pool: PgPool) {
    let day1 = app(&pool, "2026-09-27T15:59:00Z");
    let day2 = app(&pool, "2026-09-27T16:01:00Z");
    let (cookie, csrf) = session(&pool, &day1).await;
    assert_eq!(
        send(&day1, "GET", "/habits", None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &day1,
            "POST",
            "/habits",
            Some(&cookie),
            None,
            Some(json!({"name":"阅读","cadence":"daily"}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &day1,
            "POST",
            "/habits",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"阅读","cadence":"weekly","weekly_target":8}))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let created = send(
        &day1,
        "POST",
        "/habits",
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"阅读","cadence":"daily"})),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let habit = body(created).await;
    assert_eq!(habit["created_on"], "2026-09-27");
    assert_eq!(habit["version"], 1);
    let id = habit["id"].as_str().unwrap();
    let path = format!("/habits/{id}");
    let updated = send(
        &day2,
        "PATCH",
        &path,
        Some(&cookie),
        Some(&csrf),
        Some(json!({"name":"每周阅读","cadence":"weekly","weekly_target":3,"expected_version":1})),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(body(updated).await["version"], 2);
    assert_eq!(
        send(
            &day2,
            "PATCH",
            &path,
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"过期","cadence":"daily","expected_version":1}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let pause = send(
        &day2,
        "POST",
        &format!("{path}/pause"),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":2})),
    )
    .await;
    assert_eq!(pause.status(), StatusCode::OK);
    assert_eq!(body(pause).await["paused"], true);
    assert_eq!(
        send(
            &day2,
            "POST",
            &format!("{path}/pause"),
            Some(&cookie),
            Some(&csrf),
            Some(json!({"expected_version":3}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let resumed = send(
        &day2,
        "POST",
        &format!("{path}/resume"),
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":3})),
    )
    .await;
    assert_eq!(resumed.status(), StatusCode::OK);
    assert_eq!(body(resumed).await["paused"], false);
    let history = body(send(&day2, "GET", &path, Some(&cookie), None, None).await).await;
    assert_eq!(history["settings"].as_array().unwrap().len(), 2);
    assert_eq!(history["settings"][0]["name"], "阅读");
    assert_eq!(history["settings"][1]["effective_on"], "2026-09-28");
    assert_eq!(history["pauses"][0]["start_on"], "2026-09-28");
    assert_eq!(history["pauses"][0]["end_on"], "2026-09-28");
    let deleted = send(
        &day2,
        "DELETE",
        &path,
        Some(&cookie),
        Some(&csrf),
        Some(json!({"expected_version":4})),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert!(body(deleted).await["deleted_at"].is_string());
    assert!(
        body(send(&day2, "GET", "/habits", Some(&cookie), None, None).await).await["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        body(
            send(
                &day2,
                "GET",
                "/habits?include_deleted=true",
                Some(&cookie),
                None,
                None
            )
            .await
        )
        .await["items"][0]["id"],
        id
    );
}
