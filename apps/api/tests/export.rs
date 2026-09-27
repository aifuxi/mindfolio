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

fn app(pool: &PgPool) -> axum::Router {
    mindfolio_api::app_with_config_and_today_clock(
        pool.clone(),
        mindfolio_api::AuthConfig::local(),
        mindfolio_api::TodayClock::fixed(
            DateTime::parse_from_rfc3339("2026-09-27T16:01:00Z")
                .unwrap()
                .with_timezone(&Utc),
        ),
    )
}

async fn get(app: &axum::Router, cookie: Option<&str>) -> Response {
    let mut request = Request::builder().uri("/export");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap()).unwrap()
}

#[sqlx::test]
async fn 私人导出包含现存数据与删除后历史且不含秘密(pool: PgPool) {
    let app = app(&pool);
    mindfolio_api::initialize_admin(
        &pool,
        "owner",
        "correct horse battery staple".into(),
        mindfolio_api::AuthConfig::local(),
    )
    .await
    .unwrap();
    assert_eq!(get(&app, None).await.status(), StatusCode::UNAUTHORIZED);
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
    let empty = body(get(&app, Some(&cookie)).await).await;
    assert_eq!(empty["format_version"], 1);
    assert!(empty["projects"].as_array().unwrap().is_empty());
    assert!(empty["daily_journals"].as_array().unwrap().is_empty());

    let project: i64 =
        sqlx::query_scalar("INSERT INTO project (name) VALUES ('当前项目') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let parent: i64 = sqlx::query_scalar("INSERT INTO task (project_id, title, planned_date) VALUES ($1, '父任务', '2026-09-28') RETURNING id").bind(project).fetch_one(&pool).await.unwrap();
    let child: i64 = sqlx::query_scalar(
        "INSERT INTO task (project_id, parent_id, title) VALUES ($1, $2, '子任务') RETURNING id",
    )
    .bind(project)
    .bind(parent)
    .fetch_one(&pool)
    .await
    .unwrap();
    let tag: i64 = sqlx::query_scalar("INSERT INTO task_tag (name) VALUES ('重要') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO task_tag_link (task_id, tag_id) VALUES ($1, $2)")
        .bind(parent)
        .bind(tag)
        .execute(&pool)
        .await
        .unwrap();
    let removed: i64 =
        sqlx::query_scalar("INSERT INTO task (title) VALUES ('已删除任务') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO task_completion (task_id, task_title, project_name, completed_at) VALUES ($1, '已删除任务', '旧项目', '2026-09-27T16:00:00Z')").bind(removed).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM task WHERE id = $1")
        .bind(removed)
        .execute(&pool)
        .await
        .unwrap();
    let habit: i64 =
        sqlx::query_scalar("INSERT INTO habit (created_on) VALUES ('2026-09-27') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO habit_setting (habit_id, effective_on, name, cadence) VALUES ($1, '2026-09-27', '阅读', 'daily')").bind(habit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO habit_pause (habit_id, start_on, end_on) VALUES ($1, '2026-09-28', '2026-09-29')").bind(habit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO habit_checkin (habit_id, business_date, completed, note) VALUES ($1, '2026-09-27', true, '完成')").bind(habit).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO daily_journal (business_date, body) VALUES ('2026-09-27', '# 记录')")
        .execute(&pool)
        .await
        .unwrap();
    let response = get(&app, Some(&cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/json; charset=utf-8"
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        "attachment; filename=\"mindfolio-2026-09-28.json\""
    );
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let raw = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(!raw.contains("password_hash"));
    assert!(!raw.contains("csrf_token"));
    assert!(!raw.contains("correct horse battery staple"));
    assert!(!raw.contains("admin_session"));
    let data: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(data["generated_at"], "2026-09-27T16:01:00+00:00");
    assert_eq!(data["projects"][0]["id"], project.to_string());
    assert_eq!(data["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(data["tasks"][0]["id"], parent.to_string());
    assert_eq!(data["tasks"][1]["parent_id"], parent.to_string());
    assert_eq!(data["tasks"][1]["id"], child.to_string());
    assert_eq!(data["tasks"][0]["planned_date"], "2026-09-28");
    assert_eq!(data["task_tags"][0]["id"], tag.to_string());
    assert_eq!(data["task_tag_links"][0]["task_id"], parent.to_string());
    assert_eq!(data["task_completions"][0]["task_id"], removed.to_string());
    assert_eq!(data["task_completions"][0]["task_title"], "已删除任务");
    assert_eq!(data["habits"][0]["id"], habit.to_string());
    assert_eq!(data["habit_settings"][0]["effective_on"], "2026-09-27");
    assert_eq!(data["habit_pauses"][0]["end_on"], "2026-09-29");
    assert_eq!(data["habit_checkins"][0]["note"], "完成");
    assert_eq!(data["daily_journals"][0]["body"], "# 记录");
}

#[sqlx::test]
async fn 并发更新时导出字段来自一致快照(pool: PgPool) {
    let app = app(&pool);
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
    sqlx::query("INSERT INTO project (name) VALUES ('旧版本')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO daily_journal (business_date, body) VALUES ('2026-09-27', '旧版本')")
        .execute(&pool)
        .await
        .unwrap();
    let writer = pool.clone();
    let update = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let mut tx = writer.begin().await.unwrap();
        sqlx::query("UPDATE project SET name = '新版本'")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE daily_journal SET body = '新版本'")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    });
    for _ in 0..12 {
        let data = body(get(&app, Some(&cookie)).await).await;
        assert_eq!(
            data["projects"][0]["name"],
            data["daily_journals"][0]["body"]
        );
    }
    update.await.unwrap();
}
