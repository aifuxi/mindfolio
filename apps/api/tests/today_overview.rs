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
        request = request.header(header::ORIGIN, "http://127.0.0.1:5173");
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

#[sqlx::test]
async fn 上海跨日后的今日习惯与记录状态(pool: PgPool) {
    let sunday = app(&pool, "2026-09-27T15:59:00Z");
    let monday = app(&pool, "2026-09-27T16:01:00Z");
    mindfolio_api::initialize_admin(
        &pool,
        "owner",
        "correct horse battery staple".into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    let login = send(
        &sunday,
        "POST",
        "/auth/login",
        None,
        None,
        Some(json!({"username":"owner","password":"correct horse battery staple"})),
    )
    .await;
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let csrf = body(login).await["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        send(&sunday, "GET", "/today/overview", None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &sunday,
            "GET",
            "/today/overview?page=0",
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let empty =
        body(send(&sunday, "GET", "/today/overview", Some(&cookie), None, None).await).await;
    assert_eq!(empty["business_date"], "2026-09-27");
    assert!(empty["habits"].as_array().unwrap().is_empty());
    assert_eq!(empty["journal_exists"], false);
    let daily = body(
        send(
            &sunday,
            "POST",
            "/habits",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"阅读","cadence":"daily"})),
        )
        .await,
    )
    .await;
    let weekly = body(
        send(
            &sunday,
            "POST",
            "/habits",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"运动","cadence":"weekly","weekly_target":1})),
        )
        .await,
    )
    .await;
    let paused = body(
        send(
            &sunday,
            "POST",
            "/habits",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"name":"休息","cadence":"daily"})),
        )
        .await,
    )
    .await;
    for habit in [&daily, &weekly] {
        assert_eq!(
            send(
                &sunday,
                "PUT",
                &format!(
                    "/habits/{}/checkins/2026-09-27",
                    habit["id"].as_str().unwrap()
                ),
                Some(&cookie),
                Some(&csrf),
                Some(json!({"completed":true,"note":"","expected_version":null}))
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        send(
            &sunday,
            "POST",
            &format!("/habits/{}/pause", paused["id"].as_str().unwrap()),
            Some(&cookie),
            Some(&csrf),
            Some(json!({"expected_version":1}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        send(
            &sunday,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"当天记录","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let before =
        body(send(&sunday, "GET", "/today/overview", Some(&cookie), None, None).await).await;
    assert_eq!(before["habits"].as_array().unwrap().len(), 2);
    assert_eq!(before["habits"][0]["needs_checkin"], false);
    assert_eq!(before["habits"][1]["needs_checkin"], false);
    assert_eq!(before["journal_exists"], true);
    let after =
        body(send(&monday, "GET", "/today/overview", Some(&cookie), None, None).await).await;
    assert_eq!(after["business_date"], "2026-09-28");
    assert_eq!(after["habits"].as_array().unwrap().len(), 2);
    assert_eq!(after["habits"][0]["needs_checkin"], true);
    assert_eq!(after["habits"][1]["needs_checkin"], true);
    assert_eq!(after["journal_exists"], false);
    assert_eq!(
        send(
            &monday,
            "PUT",
            &format!(
                "/habits/{}/checkins/2026-09-28",
                weekly["id"].as_str().unwrap()
            ),
            Some(&cookie),
            None,
            Some(json!({"completed":true,"note":"","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &monday,
            "PUT",
            &format!(
                "/habits/{}/checkins/2026-09-28",
                weekly["id"].as_str().unwrap()
            ),
            Some(&cookie),
            Some(&csrf),
            Some(json!({"completed":true,"note":"","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::OK
    );
    let updated =
        body(send(&monday, "GET", "/today/overview", Some(&cookie), None, None).await).await;
    assert_eq!(updated["habits"][0]["needs_checkin"], false);
}
