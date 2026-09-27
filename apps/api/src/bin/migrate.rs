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
    println!("数据库迁移完成");
    Ok(())
}
