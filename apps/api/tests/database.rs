use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tower::ServiceExt;

async fn response(pool: PgPool, path: &str) -> (StatusCode, String) {
    let response = mindfolio_api::app(pool)
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
async fn 数据库就绪与不可用时的健康响应() {
    let pool = mindfolio_api::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    assert_eq!(
        response(pool, "/health/ready").await,
        (StatusCode::OK, r#"{"status":"ok"}"#.into())
    );

    let unavailable = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(200))
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let (status, body) = response(unavailable.clone(), "/health/ready").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let error: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(error["code"], "database_unavailable");
    assert_eq!(error["message"], "数据库不可用");
    assert!(uuid::Uuid::parse_str(error["request_id"].as_str().unwrap()).is_ok());
    assert_eq!(
        response(unavailable, "/health/live").await,
        (StatusCode::OK, r#"{"status":"ok"}"#.into())
    );
}

#[tokio::test]
async fn 唯一管理者和会话约束() {
    let pool = mindfolio_api::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO admin_account (id, username, password_hash) OVERRIDING SYSTEM VALUE VALUES (1, 'admin', 'hash-only')")
        .execute(&mut *tx).await.unwrap();

    sqlx::query("SAVEPOINT singleton_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let duplicate = sqlx::query("INSERT INTO admin_account (id, username, password_hash) OVERRIDING SYSTEM VALUE VALUES (2, 'other', 'hash-only')")
        .execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT singleton_check")
        .execute(&mut *tx)
        .await
        .unwrap();

    sqlx::query("SAVEPOINT fk_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let foreign_key = sqlx::query("INSERT INTO admin_session (admin_id, token_hash, csrf_token_hash, idle_expires_at, absolute_expires_at) VALUES (2, decode(repeat('01', 32), 'hex'), decode(repeat('02', 32), 'hex'), now() + interval '1 hour', now() + interval '1 day')")
        .execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        foreign_key.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT fk_check")
        .execute(&mut *tx)
        .await
        .unwrap();

    sqlx::query("INSERT INTO admin_session (admin_id, token_hash, csrf_token_hash, idle_expires_at, absolute_expires_at) VALUES (1, decode(repeat('01', 32), 'hex'), decode(repeat('02', 32), 'hex'), now() + interval '1 hour', now() + interval '1 day')")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("SAVEPOINT token_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    let token = sqlx::query("INSERT INTO admin_session (admin_id, token_hash, csrf_token_hash, idle_expires_at, absolute_expires_at) VALUES (1, decode(repeat('01', 32), 'hex'), decode(repeat('03', 32), 'hex'), now() + interval '1 hour', now() + interval '1 day')")
        .execute(&mut *tx).await.unwrap_err();
    assert_eq!(
        token.as_database_error().unwrap().code().as_deref(),
        Some("23505")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT token_check")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    let index_count: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_indexes WHERE tablename = 'admin_session' AND indexname = 'admin_session_admin_active_idx'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(index_count, 1);
}
