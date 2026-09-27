#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| std::io::Error::other("缺少 DATABASE_URL 环境变量"))?;
    let pool = mindfolio_api::connect(&database_url)
        .await
        .map_err(|error| std::io::Error::other(format!("数据库连接失败：{error}")))?;
    mindfolio_api::migrate(&pool)
        .await
        .map_err(|error| std::io::Error::other(format!("数据库迁移失败：{error}")))?;
    let address = std::env::var("API_BIND").unwrap_or_else(|_| "127.0.0.1:3001".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("API 正在监听 http://{address}");
    let auth = mindfolio_api::AuthConfig::from_env()?;
    axum::serve(listener, mindfolio_api::app_with_config(pool, auth)).await?;
    Ok(())
}
