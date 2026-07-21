use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let driver = sheeko::ecs::spawn_driver();
    let app = sheeko::gateway::app(driver);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3030").await?;
    info!("gateway listening on 127.0.0.1:3030");
    axum::serve(listener, app).await?;
    Ok(())
}
