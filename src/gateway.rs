//! The gateway's HTTP/WS application, factored out of `src/bin/gateway.rs` so
//! `tests/ws_chat.rs` can build and run the exact same `Router` in-process
//! instead of duplicating the WS handshake/chat logic.

use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::common::message::{ClientMessage, ServerMessage};
use crate::ecs::components::RoomId;
use crate::ecs::{self, Command, DriverHandle};

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
}

#[derive(Serialize)]
struct CreateUserResponse {
    user_id: String,
}

#[derive(Deserialize, Default)]
struct CreateDeviceRequest {
    #[serde(default)]
    as_audio_source: bool,
}

#[derive(Serialize)]
struct CreateDeviceResponse {
    device_id: String,
}

#[derive(Serialize)]
struct UserResponse {
    user_id: String,
    username: String,
    devices: Vec<String>,
    online: bool,
}

pub fn app(driver: DriverHandle) -> Router {
    Router::new()
        .route("/users", post(create_user))
        .route("/users/{user_id}/devices", post(create_device))
        .route("/users/{user_id}", get(get_user))
        .route(
            "/users/{user_id}/audio-device/{device_id}",
            put(switch_audio_device),
        )
        .route("/ws", get(ws_upgrade))
        .with_state(driver)
}

async fn create_user(
    State(driver): State<DriverHandle>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), StatusCode> {
    let (reply, rx) = oneshot::channel();
    driver.send(Command::CreateUser {
        username: req.username,
        reply,
    });
    let entity = rx.await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user_id: ecs::entity_to_id(entity),
        }),
    ))
}

async fn create_device(
    State(driver): State<DriverHandle>,
    Path(user_id): Path<String>,
    body: Option<Json<CreateDeviceRequest>>,
) -> Result<(StatusCode, Json<CreateDeviceResponse>), StatusCode> {
    let owner = ecs::id_to_entity(&user_id).ok_or(StatusCode::NOT_FOUND)?;
    let as_audio_source = body.map(|Json(req)| req.as_audio_source).unwrap_or(false);

    let (reply, rx) = oneshot::channel();
    driver.send(Command::CreateDevice {
        owner,
        as_audio_source,
        reply,
    });
    let device = rx
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDeviceResponse {
            device_id: ecs::entity_to_id(device),
        }),
    ))
}

async fn get_user(
    State(driver): State<DriverHandle>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, StatusCode> {
    let user = ecs::id_to_entity(&user_id).ok_or(StatusCode::NOT_FOUND)?;

    let (reply, rx) = oneshot::channel();
    driver.send(Command::GetUser { user, reply });
    let view = rx
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(UserResponse {
        user_id,
        username: view.username,
        devices: view.devices.into_iter().map(ecs::entity_to_id).collect(),
        online: view.online,
    }))
}

/// Moves the active audio source for `user_id` onto `device_id` — the
/// project-defining feature made concrete over HTTP. Returns 400 if
/// `device_id` isn't owned by `user_id`, 404 if either id is unknown.
async fn switch_audio_device(
    State(driver): State<DriverHandle>,
    Path((user_id, device_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let user = ecs::id_to_entity(&user_id).ok_or(StatusCode::NOT_FOUND)?;
    let to_device = ecs::id_to_entity(&device_id).ok_or(StatusCode::NOT_FOUND)?;

    let (reply, rx) = oneshot::channel();
    driver.send(Command::SwitchAudioDevice {
        user,
        to_device,
        reply,
    });
    rx.await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn ws_upgrade(State(driver): State<DriverHandle>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, driver))
}

/// One task per WS connection. The first message must be `ClientMessage::Join`;
/// everything before that is dropped. After joining, `tokio::select!` between
/// the socket (incoming chat) and this Device's `OutboundTx` receiver
/// (messages other Devices in the room sent) keeps read and write concurrent
/// without needing to split the socket.
async fn handle_socket(mut socket: WebSocket, driver: DriverHandle) {
    let Some((device, mut outbound_rx)) = join_room(&mut socket, &driver).await else {
        return;
    };

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(ClientMessage::Chat { text }) = serde_json::from_str(&text) {
                            driver.send(Command::ChatMessage { from: device, text });
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            Some(bytes) = outbound_rx.recv() => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                if socket.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Waits for the connection's `Join` message and registers it as a Device
/// entity. Returns `None` (dropping the connection) on any malformed
/// message, unknown user, or early close.
async fn join_room(
    socket: &mut WebSocket,
    driver: &DriverHandle,
) -> Option<(hecs::Entity, mpsc::UnboundedReceiver<Bytes>)> {
    loop {
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => {
                let Ok(ClientMessage::Join { user_id, room }) = serde_json::from_str(&text) else {
                    continue;
                };
                let Some(user) = ecs::id_to_entity(&user_id) else {
                    let _ = socket
                        .send(Message::text(
                            serde_json::to_string(&ServerMessage::Error {
                                message: "unknown user_id".into(),
                            })
                            .unwrap(),
                        ))
                        .await;
                    return None;
                };

                let (tx, rx) = mpsc::unbounded_channel();
                let (reply, reply_rx) = oneshot::channel();
                driver.send(Command::JoinTextRoom {
                    user,
                    room: RoomId(room),
                    outbound: tx,
                    reply,
                });

                return reply_rx.await.ok().flatten().map(|device| (device, rx));
            }
            Some(Ok(Message::Close(_))) | None => return None,
            Some(Err(_)) => return None,
            _ => continue,
        }
    }
}
