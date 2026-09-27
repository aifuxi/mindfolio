use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery staple";
const NEW_PASSWORD: &str = "another long private password";

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
    let content = if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    app.clone()
        .oneshot(request.body(content).unwrap())
        .await
        .unwrap()
}

async fn json_body(response: Response) -> Value {
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn login(app: &axum::Router, origin: &str, username: &str, password: &str) -> Response {
    send(
        app,
        "POST",
        "/auth/login",
        Some(origin),
        None,
        None,
        Some(json!({ "username": username, "password": password })),
    )
    .await
}

#[sqlx::test]
async fn 认证会话与密码重置通过真实数据库(pool: PgPool) {
    let config = mindfolio_api::AuthConfig::local();
    let app = mindfolio_api::app_with_config(pool.clone(), config.clone());
    let origin = "http://127.0.0.1:5173";

    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/login",
            None,
            None,
            None,
            Some(json!({ "username": "owner", "password": PASSWORD })),
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let no_account = login(&app, origin, "owner", PASSWORD).await;
    assert_eq!(no_account.status(), StatusCode::UNAUTHORIZED);
    let no_account_error = json_body(no_account).await;
    mindfolio_api::initialize_admin(&pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    assert!(
        mindfolio_api::initialize_admin(&pool, "other", PASSWORD.into(), config.clone())
            .await
            .unwrap_err()
            .contains("拒绝重复初始化")
    );
    let stored: String = sqlx::query_scalar("SELECT password_hash FROM admin_account")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored.starts_with("$argon2id$"));
    assert!(!stored.contains(PASSWORD));
    let malformed = send(
        &app,
        "POST",
        "/auth/login",
        Some(origin),
        None,
        None,
        Some(json!({ "username": "owner" })),
    )
    .await;
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json_body(malformed).await["code"], "invalid_request");

    let wrong = login(&app, origin, "owner", NEW_PASSWORD).await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let wrong_error = json_body(wrong).await;
    assert_eq!(wrong_error["code"], no_account_error["code"]);
    assert_eq!(wrong_error["message"], no_account_error["message"]);

    let success = login(&app, origin, "owner", PASSWORD).await;
    assert_eq!(success.status(), StatusCode::OK);
    let cookie_header = success.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    assert!(!cookie_header.contains("Secure"));
    assert!(!cookie_header.contains("Domain="));
    let cookie = cookie_header.split(';').next().unwrap().to_string();
    let body = json_body(success).await;
    let csrf = body["csrf_token"].as_str().unwrap();
    assert_eq!(body["username"], "owner");
    let token_hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM admin_session")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(token_hash.len(), 32);
    assert!(!cookie.as_bytes().windows(32).any(|part| part == token_hash));

    let session = send(
        &app,
        "GET",
        "/auth/session",
        None,
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(session.status(), StatusCode::OK);
    assert_eq!(json_body(session).await["csrf_token"], csrf);
    assert_eq!(
        send(&app, "GET", "/auth/session", None, None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/logout",
            Some(origin),
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/logout",
            Some(origin),
            Some(&cookie),
            Some("wrong-csrf"),
            None,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/logout",
            Some("http://evil.example"),
            Some(&cookie),
            Some(csrf),
            None,
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &app,
            "POST",
            "/auth/logout",
            Some(origin),
            Some(&cookie),
            Some(csrf),
            None
        )
        .await
        .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            &app,
            "GET",
            "/auth/session",
            None,
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );

    let again = login(&app, origin, "owner", PASSWORD).await;
    let again_cookie = again.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    sqlx::query("UPDATE admin_session SET idle_expires_at = now() - interval '1 second' WHERE revoked_at IS NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "GET",
            "/auth/session",
            None,
            Some(&again_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );

    let third = login(&app, origin, "owner", PASSWORD).await;
    let third_cookie = third.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    mindfolio_api::reset_admin(&pool, NEW_PASSWORD.into(), config)
        .await
        .unwrap();
    assert_eq!(
        send(
            &app,
            "GET",
            "/auth/session",
            None,
            Some(&third_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let restarted =
        mindfolio_api::app_with_config(pool.clone(), mindfolio_api::AuthConfig::local());
    assert_eq!(
        login(&restarted, origin, "owner", PASSWORD).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&restarted, origin, "owner", NEW_PASSWORD)
            .await
            .status(),
        StatusCode::OK
    );
    let last = login(&restarted, origin, "owner", NEW_PASSWORD).await;
    let last_cookie = last.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    sqlx::query("UPDATE admin_session SET absolute_expires_at = now() - interval '1 second', idle_expires_at = now() - interval '1 second' WHERE revoked_at IS NULL")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        send(
            &restarted,
            "GET",
            "/auth/session",
            None,
            Some(&last_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "GET", "/health/live", None, None, None, None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[sqlx::test]
async fn 登录限流和重新登录轮换会话(pool: PgPool) {
    let config = mindfolio_api::AuthConfig::local();
    mindfolio_api::initialize_admin(&pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    let app = mindfolio_api::app_with_config(pool, config);
    let origin = "http://127.0.0.1:5173";
    let first = login(&app, origin, "owner", PASSWORD).await;
    let cookie = first.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let rotated = send(
        &app,
        "POST",
        "/auth/login",
        Some(origin),
        Some(&cookie),
        None,
        Some(json!({ "username": "owner", "password": PASSWORD })),
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    assert_ne!(
        rotated.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap(),
        cookie
    );
    assert_eq!(
        send(
            &app,
            "GET",
            "/auth/session",
            None,
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    for _ in 0..3 {
        assert_eq!(
            login(&app, origin, "owner", NEW_PASSWORD).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        login(&app, origin, "owner", PASSWORD).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[sqlx::test]
async fn 生产会话_cookie_使用安全属性(pool: PgPool) {
    let origin = "https://admin.example.com";
    let config = mindfolio_api::AuthConfig::for_origin(origin).unwrap();
    mindfolio_api::initialize_admin(&pool, "owner", PASSWORD.into(), config.clone())
        .await
        .unwrap();
    let app = mindfolio_api::app_with_config(pool, config);
    let response = login(&app, origin, "owner", PASSWORD).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("__Host-mf_session="));
    assert!(cookie.contains("; Secure"));
    assert!(cookie.contains("; HttpOnly"));
    assert!(cookie.contains("; SameSite=Strict"));
    assert!(cookie.contains("; Path=/"));
    assert!(!cookie.contains("Domain="));
    assert!(mindfolio_api::AuthConfig::for_origin("http://admin.example.com").is_err());
}
