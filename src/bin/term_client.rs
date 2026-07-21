use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

use sheeko::common::message::{ClientMessage, ServerMessage};

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let mut args = std::env::args().skip(1);
    let user_id = args
        .next()
        .context("usage: term_client <user_id> <room>")?;
    let room: u64 = args
        .next()
        .context("usage: term_client <user_id> <room>")?
        .parse()
        .context("room must be a number")?;

    let (ws_stream, _) = tokio_tungstenite::connect_async("ws://127.0.0.1:3030/ws").await?;
    let (mut write, mut read) = ws_stream.split();

    let join = ClientMessage::Join { user_id, room };
    write.send(Message::text(serde_json::to_string(&join)?)).await?;
    info!(room, "joined — type a message and press enter (Ctrl+C to quit)");

    let mut lines = BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line? {
                    Some(text) => {
                        let chat = ClientMessage::Chat { text };
                        write.send(Message::text(serde_json::to_string(&chat)?)).await?;
                    }
                    None => break, // stdin closed
                }
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ServerMessage>(&text) {
                            Ok(ServerMessage::Chat { from, text }) => println!("[{from}] {text}"),
                            Ok(ServerMessage::Error { message }) => warn!(%message, "server error"),
                            Err(err) => warn!(%err, "malformed server message"),
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(err)) => {
                        warn!(%err, "websocket error");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
