use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let listener = TcpListener::bind("127.0.0.1:7000").await?;
    info!("tcp_echo listening on 127.0.0.1:7000");

    loop {
        let (mut socket, addr) = listener.accept().await?;
        info!(%addr, "accepted connection");

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if let Err(err) = socket.write_all(&buf[..n]).await {
                            warn!(%addr, %err, "write failed");
                            break;
                        }
                    }
                    Err(err) => {
                        warn!(%addr, %err, "read failed");
                        break;
                    }
                }
            }
            info!(%addr, "connection closed");
        });
    }
}
