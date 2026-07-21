use anyhow::{Context, Result};
use std::env;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::sleep;
use tracing::info;

const PONG_ADDR: &str = "127.0.0.1:5001";

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let role = env::args()
        .nth(1)
        .context("usage: udp_ping_pong <ping|pong>")?;

    match role.as_str() {
        "pong" => run_pong().await,
        "ping" => run_ping().await,
        other => anyhow::bail!("unknown role {other:?}, expected \"ping\" or \"pong\""),
    }
}

async fn run_pong() -> Result<()> {
    let socket = UdpSocket::bind(PONG_ADDR).await?;
    info!(addr = PONG_ADDR, "pong listening");

    let mut buf = [0u8; 64];
    loop {
        let (n, peer) = socket.recv_from(&mut buf).await?;
        let msg = String::from_utf8_lossy(&buf[..n]);
        info!(%peer, %msg, "received");
        socket.send_to(b"pong", peer).await?;
    }
}

async fn run_ping() -> Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:0").await?;
    socket.connect(PONG_ADDR).await?;
    info!(local = %socket.local_addr()?, "ping ready");

    let mut buf = [0u8; 64];
    loop {
        socket.send(b"ping").await?;
        info!("sent ping");
        let n = socket.recv(&mut buf).await?;
        let msg = String::from_utf8_lossy(&buf[..n]);
        info!(%msg, "received");
        sleep(Duration::from_secs(1)).await;
    }
}
