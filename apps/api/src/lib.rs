use std::time::Duration;

use axum::{Json, Router, http::StatusCode, routing::get};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Serialize)]
struct HealthStatus {
    status: &'static str,
}

async fn live() -> Json<HealthStatus> {
    Json(HealthStatus { status: "ok" })
}

async fn ready(
    axum::extract::State(pool): axum::extract::State<PgPool>,
) -> (StatusCode, Json<HealthStatus>) {
    if sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok()
    {
        (StatusCode::OK, Json(HealthStatus { status: "ok" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthStatus {
                status: "unavailable",
            }),
        )
    }
}

pub fn app(pool: PgPool) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(pool)
}

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await
}

pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::app;

    #[tokio::test]
    async fn 进程存活接口返回成功状态() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .unwrap();
        let response = app(pool)
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "application/json"
        );
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(body, r#"{"status":"ok"}"#);
    }
}
