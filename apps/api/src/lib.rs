use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Serialize)]
struct LiveStatus {
    status: &'static str,
}

async fn live() -> Json<LiveStatus> {
    Json(LiveStatus { status: "ok" })
}

pub fn app() -> Router {
    Router::new().route("/health/live", get(live))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    use super::app;

    #[tokio::test]
    async fn 进程存活接口返回成功状态() {
        let response = app()
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
