#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "127.0.0.1:3001";
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("API 正在监听 http://{address}");
    axum::serve(listener, mindfolio_api::app()).await?;
    Ok(())
}
