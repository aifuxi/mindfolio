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

fn fixed_app(pool: &PgPool, instant: &str) -> axum::Router {
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

async fn send(app: &axum::Router, path: &str, cookie: Option<&str>) -> Response {
    let mut request = Request::builder().uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 100_000).await.unwrap()).unwrap()
}

#[sqlx::test]
async fn 跨月周回顾保留快照并分页(pool: PgPool) {
    let app = fixed_app(&pool, "2026-10-04T15:00:00Z");
    mindfolio_api::initialize_admin(
        &pool,
        "owner",
        "correct horse battery staple".into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header(header::ORIGIN, "http://127.0.0.1:5173")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({"username":"owner","password":"correct horse battery staple"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = login.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    assert_eq!(
        send(&app, "/weekly-review?date=2026-10-01", None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&app, "/weekly-review?date=2026-10-12", Some(&cookie))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let empty = body(send(&app, "/weekly-review?date=2026-09-21", Some(&cookie)).await).await;
    assert!(empty["completions"].as_array().unwrap().is_empty());
    assert!(empty["habits"].as_array().unwrap().is_empty());
    assert!(empty["journals"].as_array().unwrap().is_empty());
    let habit: i64 =
        sqlx::query_scalar("INSERT INTO habit (created_on) VALUES ('2026-09-28') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence, weekly_target) VALUES ($1, '2026-09-28', '旧名称', 'weekly', 3)").bind(habit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO habit_pause (habit_id, start_on) VALUES ($1, '2026-10-04')")
        .bind(habit)
        .execute(&pool)
        .await
        .unwrap();
    for (day, completed) in [
        ("2026-09-30", true),
        ("2026-10-01", true),
        ("2026-10-02", false),
    ] {
        sqlx::query("INSERT INTO habit_checkin (habit_id, business_date, completed) VALUES ($1, $2::date, $3)").bind(habit).bind(day).bind(completed).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO daily_journal (business_date, body) VALUES ('2026-10-01', '# 回顾')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO task_completion (task_id, task_title, project_name, completed_at) VALUES (9001, '删除后的任务', '旧项目', '2026-09-27T16:00:00Z'), (9002, '前一周', NULL, '2026-09-27T15:59:00Z'), (9003, '下一周', NULL, '2026-10-04T16:00:00Z')").execute(&pool).await.unwrap();
    let review = body(send(&app, "/weekly-review?date=2026-10-01", Some(&cookie)).await).await;
    assert_eq!(review["starts_on"], "2026-09-28");
    assert_eq!(review["ends_on"], "2026-10-04");
    assert_eq!(review["dates"].as_array().unwrap().len(), 7);
    assert_eq!(review["completions"].as_array().unwrap().len(), 1);
    assert_eq!(review["completions"][0]["task_title"], "删除后的任务");
    assert_eq!(review["completions"][0]["project_name"], "旧项目");
    assert_eq!(review["completions"][0]["task_exists"], false);
    assert_eq!(review["journals"][0]["body"], "# 回顾");
    assert_eq!(review["habits"][0]["name"], "旧名称");
    assert_eq!(review["habits"][0]["snapshot"]["rate"]["earned"], 2.0);
    assert_eq!(review["habits"][0]["snapshot"]["rate"]["possible"], 3.0);
    assert_eq!(
        review["habits"][0]["snapshot"]["days"][4]["state"],
        "incomplete"
    );
    assert_eq!(
        review["habits"][0]["snapshot"]["days"][6]["state"],
        "paused"
    );
    let partial = body(
        send(
            &fixed_app(&pool, "2026-10-01T15:00:00Z"),
            "/weekly-review?date=2026-10-01",
            Some(&cookie),
        )
        .await,
    )
    .await;
    assert_eq!(
        partial["habits"][0]["snapshot"]["days"][4]["state"],
        "future"
    );
    assert_eq!(partial["habits"][0]["snapshot"]["rate"]["possible"], 3.0);
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence, weekly_target) VALUES ($1, '2026-10-05', '新名称', 'weekly', 4)").bind(habit).execute(&pool).await.unwrap();
    let unchanged = body(send(&app, "/weekly-review?date=2026-10-01", Some(&cookie)).await).await;
    assert_eq!(unchanged["habits"][0]["name"], "旧名称");
    for index in 0..55 {
        sqlx::query("INSERT INTO task_completion (task_id, task_title, completed_at) VALUES ($1, $2, '2026-10-01T01:00:00Z')").bind(10000 + index).bind(format!("完成 {index}")).execute(&pool).await.unwrap();
    }
    let first =
        body(send(&app, "/weekly-review?date=2026-10-01&page=1", Some(&cookie)).await).await;
    let second =
        body(send(&app, "/weekly-review?date=2026-10-01&page=2", Some(&cookie)).await).await;
    assert_eq!(first["completions"].as_array().unwrap().len(), 50);
    assert_eq!(first["has_more_completions"], true);
    assert_eq!(second["completions"].as_array().unwrap().len(), 6);
    assert_eq!(second["has_more_completions"], false);
}
