use serde::{Deserialize, Serialize};

/// Sent by a client over the `/ws` route. `Join` must be the first message on
/// a connection — everything else is dropped until a device is registered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join { user_id: String, room: u64 },
    Chat { text: String },
}

/// Sent by the gateway to a client over `/ws`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    Chat { from: String, text: String },
    Error { message: String },
}
