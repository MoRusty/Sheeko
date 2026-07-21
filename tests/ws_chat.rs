use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

use sheeko::common::message::{ClientMessage, ServerMessage};
use sheeko::ecs::{self, Command};

/// Spins up the real gateway `Router` (the same one `src/bin/gateway.rs`
/// serves) on an ephemeral port and returns its base URL plus a `Command`
/// sender for creating fixture Users directly, without an HTTP round trip.
async fn spawn_gateway() -> (String, sheeko::ecs::DriverHandle) {
    let driver = ecs::spawn_driver();
    let app = sheeko::gateway::app(driver.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("ws://{addr}/ws"), driver)
}

async fn create_user(driver: &sheeko::ecs::DriverHandle, username: &str) -> String {
    let (reply, rx) = tokio::sync::oneshot::channel();
    driver.send(Command::CreateUser {
        username: username.to_string(),
        reply,
    });
    ecs::entity_to_id(rx.await.unwrap())
}

async fn join(url: &str, user_id: &str, room: u64) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let join = ClientMessage::Join {
        user_id: user_id.to_string(),
        room,
    };
    ws.send(Message::text(serde_json::to_string(&join).unwrap()))
        .await
        .unwrap();
    ws
}

async fn send_chat(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    text: &str,
) {
    let chat = ClientMessage::Chat {
        text: text.to_string(),
    };
    ws.send(Message::text(serde_json::to_string(&chat).unwrap()))
        .await
        .unwrap();
}

async fn recv_chat(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> ServerMessage {
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), ws.next())
        .await
        .expect("timed out waiting for a chat message")
        .expect("stream ended")
        .expect("websocket error");
    match msg {
        Message::Text(text) => serde_json::from_str(&text).expect("malformed ServerMessage"),
        other => panic!("expected a text message, got {other:?}"),
    }
}

/// The PDR's DoD item: two separate terminal clients exchange "Hello".
#[tokio::test]
async fn two_different_users_exchange_hello() {
    let (url, driver) = spawn_gateway().await;
    let alice = create_user(&driver, "alice").await;
    let bob = create_user(&driver, "bob").await;

    let mut alice_ws = join(&url, &alice, 1).await;
    let mut bob_ws = join(&url, &bob, 1).await;

    send_chat(&mut alice_ws, "Hello").await;

    let received = recv_chat(&mut bob_ws).await;
    match received {
        ServerMessage::Chat { from, text } => {
            assert_eq!(from, alice);
            assert_eq!(text, "Hello");
        }
        other => panic!("expected a Chat message, got {other:?}"),
    }
}

/// Stretch goal from the plan: one User with two Devices (two WS
/// connections) both in the same room both receive a message from a third
/// party — the multi-device-per-identity thesis, proven over text chat.
#[tokio::test]
async fn same_user_two_devices_both_receive_fan_out() {
    let (url, driver) = spawn_gateway().await;
    let carol = create_user(&driver, "carol").await;
    let dave = create_user(&driver, "dave").await;

    // Carol connects from two "devices" (phone + laptop) into the same room.
    let mut carol_phone = join(&url, &carol, 42).await;
    let mut carol_laptop = join(&url, &carol, 42).await;
    let mut dave_ws = join(&url, &dave, 42).await;

    send_chat(&mut dave_ws, "hi both of you").await;

    let on_phone = recv_chat(&mut carol_phone).await;
    let on_laptop = recv_chat(&mut carol_laptop).await;

    for received in [on_phone, on_laptop] {
        match received {
            ServerMessage::Chat { from, text } => {
                assert_eq!(from, dave);
                assert_eq!(text, "hi both of you");
            }
            other => panic!("expected a Chat message, got {other:?}"),
        }
    }
}
