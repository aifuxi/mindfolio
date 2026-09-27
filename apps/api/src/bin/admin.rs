#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let action = std::env::args().nth(1).ok_or("用法：admin {init|reset}")?;
    let database_url = std::env::var("DATABASE_URL").map_err(|_| "缺少 DATABASE_URL")?;
    let config = mindfolio_api::AuthConfig::from_env()?;
    let pool = mindfolio_api::connect(&database_url).await?;
    mindfolio_api::migrate(&pool).await?;
    let password = rpassword::prompt_password("新密码：")?;
    let repeated = rpassword::prompt_password("再次输入新密码：")?;
    if password != repeated {
        return Err("两次密码不一致".into());
    }
    match action.as_str() {
        "init" => {
            let username = std::env::var("ADMIN_USERNAME").map_err(|_| "缺少 ADMIN_USERNAME")?;
            mindfolio_api::initialize_admin(&pool, &username, password, config).await?;
            println!("管理者初始化完成");
        }
        "reset" => {
            mindfolio_api::reset_admin(&pool, password, config).await?;
            println!("管理者密码已重置，全部旧会话已撤销");
        }
        _ => return Err("用法：admin {init|reset}".into()),
    }
    Ok(())
}
