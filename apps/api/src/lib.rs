use std::time::Duration;

use axum::{Json, Router, http::StatusCode, routing::get};
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use utoipa::{OpenApi, ToSchema};

mod auth;
mod checkins;
mod completions;
mod habits;
mod journal;
mod projects;
mod tasks;
pub use auth::{AuthConfig, initialize_admin, reset_admin};
pub use tasks::TodayClock;

#[derive(Serialize, ToSchema)]
struct HealthStatus {
    status: HealthState,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
enum HealthState {
    Ok,
}

#[derive(Serialize, ToSchema)]
struct ApiError {
    code: ErrorCode,
    message: String,
    request_id: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
enum ErrorCode {
    DatabaseUnavailable,
    Unauthorized,
    InvalidCredentials,
    InvalidRequest,
    Forbidden,
    RateLimited,
    AuthUnavailable,
    NotFound,
    VersionConflict,
}

#[derive(OpenApi)]
#[openapi(
    paths(live, ready, auth::login, auth::session, auth::logout, projects::list, projects::create, projects::detail, projects::rename, projects::complete, projects::archive, projects::restore, projects::delete, tasks::list, tasks::list_subtasks, tasks::create, tasks::detail, tasks::update, tasks::delete, tasks::today::today_tasks, tasks::today::plan_today, completions::list, completions::days, habits::list, habits::create, habits::detail, habits::update, habits::pause, habits::resume, habits::delete, checkins::list, checkins::save, checkins::calendar, journal::day, journal::save),
    components(schemas(HealthStatus, HealthState, ApiError, ErrorCode, auth::LoginRequest, auth::SessionResponse, projects::ProjectResponse, projects::ProjectPage, projects::CreateProject, projects::RenameProject, projects::VersionRequest, tasks::TaskStatus, tasks::TaskPriority, tasks::TaskResponse, tasks::TaskPage, tasks::CreateTask, tasks::UpdateTask, tasks::DeleteTask, tasks::today::TodayReason, tasks::today::TodayTaskResponse, tasks::today::TodayTaskPage, tasks::today::PlanTodayRequest, completions::CompletionResponse, completions::CompletionPage, completions::DaySummary, completions::DayPage, habits::HabitResponse, habits::HabitPage, habits::HabitInput, habits::HabitUpdate, habits::VersionRequest, habits::SettingResponse, habits::PauseResponse, habits::HabitDetail, checkins::CheckinResponse, checkins::CheckinList, checkins::SaveCheckin, checkins::DayState, checkins::CalendarDay, checkins::Rate, checkins::WeekRate, checkins::CalendarResponse, journal::JournalResponse, journal::DayCompletion, journal::DayCheckin, journal::JournalDay, journal::SaveJournal)),
    servers((url = "/api"))
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut document = ApiDoc::openapi();
    document.openapi = utoipa::openapi::OpenApiVersion::Version31;
    document.info.description = Some("Mindfolio 管理端 API".to_string());
    document.info.license = None;
    if let Some(components) = document.components.as_mut() {
        components.add_security_scheme(
            "admin_session",
            utoipa::openapi::security::SecurityScheme::ApiKey(
                utoipa::openapi::security::ApiKey::Cookie(
                    utoipa::openapi::security::ApiKeyValue::new("__Host-mf_session"),
                ),
            ),
        );
    }
    document
}

#[utoipa::path(
    get,
    path = "/health/live",
    responses((status = 200, description = "进程存活", body = HealthStatus))
)]
async fn live() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: HealthState::Ok,
    })
}

#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "数据库就绪", body = HealthStatus),
        (status = 503, description = "数据库不可用", body = ApiError)
    )
)]
async fn ready(
    axum::extract::State(pool): axum::extract::State<PgPool>,
) -> Result<Json<HealthStatus>, (StatusCode, Json<ApiError>)> {
    if sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok()
    {
        Ok(Json(HealthStatus {
            status: HealthState::Ok,
        }))
    } else {
        Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiError {
                code: ErrorCode::DatabaseUnavailable,
                message: "数据库不可用".to_string(),
                request_id: uuid::Uuid::new_v4().to_string(),
            }),
        ))
    }
}

pub fn app(pool: PgPool) -> Router {
    app_with_config(pool, AuthConfig::local())
}

pub fn app_with_config(pool: PgPool, config: AuthConfig) -> Router {
    app_with_config_and_today_clock(pool, config, TodayClock::system())
}

pub fn app_with_config_and_today_clock(
    pool: PgPool,
    config: AuthConfig,
    clock: TodayClock,
) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .merge(auth::protect(
            projects::routes()
                .merge(tasks::routes())
                .merge(tasks::today_routes(clock.clone()))
                .merge(completions::routes())
                .merge(habits::routes().layer(axum::Extension(clock.clone())))
                .merge(checkins::routes().layer(axum::Extension(clock.clone())))
                .merge(journal::routes().layer(axum::Extension(clock))),
            pool.clone(),
            config.clone(),
        ))
        .merge(auth::routes(pool.clone(), config))
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
