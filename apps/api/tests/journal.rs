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
    serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap()).unwrap()
}

#[sqlx::test]
async fn 每日记录版本边界与独立事实(pool: PgPool) {
    let early = app(&pool, "2026-09-27T15:59:00Z");
    let late = app(&pool, "2026-09-27T16:01:00Z");
    mindfolio_api::initialize_admin(
        &pool,
        "owner",
        "correct horse battery staple".into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    let login = send(
        &early,
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
        send(&early, "GET", "/journal/2026-09-27", None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let empty = body(
        send(
            &early,
            "GET",
            "/journal/2026-09-27",
            Some(&cookie),
            None,
            None,
        )
        .await,
    )
    .await;
    assert!(empty["journal"].is_null());
    assert!(empty["completions"].as_array().unwrap().is_empty());
    assert_eq!(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            None,
            Some(json!({"body":"内容","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &early,
            "PUT",
            "/journal/2026-09-28",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"提前","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"   ","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    let first = body(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"# 今天","expected_version":null})),
        )
        .await,
    )
    .await;
    assert_eq!(first["version"], 1);
    assert_eq!(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"重复","expected_version":null}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    let revised = body(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"# 修改后","expected_version":1})),
        )
        .await,
    )
    .await;
    assert_eq!(revised["version"], 2);
    assert_eq!(
        send(
            &early,
            "PUT",
            "/journal/2026-09-27",
            Some(&cookie),
            Some(&csrf),
            Some(json!({"body":"过期","expected_version":1}))
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    sqlx::query("INSERT INTO task_completion (task_id, task_title, project_name, completed_at) VALUES (12345, '已删除任务', '已删除项目', '2026-09-27T15:58:00Z'), (12346, '次日任务', NULL, '2026-09-27T16:00:00Z')").execute(&pool).await.unwrap();
    let habit: i64 =
        sqlx::query_scalar("INSERT INTO habit (created_on) VALUES ('2026-09-27') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence) VALUES ($1, '2026-09-27', '阅读', 'daily')").bind(habit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO habit_checkin (habit_id, business_date, completed, note) VALUES ($1, '2026-09-27', true, '睡前')").bind(habit).execute(&pool).await.unwrap();
    let day = body(
        send(
            &late,
            "GET",
            "/journal/2026-09-27",
            Some(&cookie),
            None,
            None,
        )
        .await,
    )
    .await;
    assert_eq!(day["journal"]["body"], "# 修改后");
    assert_eq!(day["completions"].as_array().unwrap().len(), 1);
    assert_eq!(day["completions"][0]["task_title"], "已删除任务");
    assert_eq!(day["completions"][0]["project_name"], "已删除项目");
    assert_eq!(day["checkins"][0]["name"], "阅读");
    assert_eq!(day["checkins"][0]["note"], "睡前");
    let next = body(
        send(
            &late,
            "GET",
            "/journal/2026-09-28",
            Some(&cookie),
            None,
            None,
        )
        .await,
    )
    .await;
    assert!(next["journal"].is_null());
    assert_eq!(next["completions"][0]["task_title"], "次日任务");
}
