use bytes::Bytes;
use hecs::Entity;
use tokio::sync::{mpsc, oneshot};

use super::components::RoomId;

/// Messages sent into the ECS driver task. The driver is the only task that
/// ever holds `&mut World`; every mutation and query goes through here.
pub enum Command {
    CreateUser {
        username: String,
        reply: oneshot::Sender<Entity>,
    },
    /// Fails (sends `None`) if `owner` does not refer to a live User entity.
    CreateDevice {
        owner: Entity,
        reply: oneshot::Sender<Option<Entity>>,
    },
    GetUser {
        user: Entity,
        reply: oneshot::Sender<Option<UserView>>,
    },
    /// Registers a standalone voice peer (not tied to any User/gateway
    /// session — see PDR M3) as a Device entity in `room`, tagged as both an
    /// audio source and sink, with `outbound` as its `OutboundTx`.
    RegisterVoicePeer {
        room: RoomId,
        outbound: mpsc::UnboundedSender<Bytes>,
        reply: oneshot::Sender<Entity>,
    },
    /// A raw packet arrived from `from`'s transport. The driver updates
    /// `from`'s jitter tracking and forwards the packet, undecoded, to every
    /// other Device in the same room with an `AudioSink`. Fire-and-forget —
    /// no reply, to keep the UDP hot loop free of per-packet round trips.
    PacketReceived { from: Entity, packet: Bytes },
    /// Registers a WS text session as a new Device entity owned by `user`, in
    /// `room`, tagged `TextChannel`. Fails (sends `None`) if `user` doesn't
    /// refer to a live User entity.
    JoinTextRoom {
        user: Entity,
        room: RoomId,
        outbound: mpsc::UnboundedSender<Bytes>,
        reply: oneshot::Sender<Option<Entity>>,
    },
    /// A chat message from `from`, fanned out to every other `TextChannel`
    /// Device in `from`'s room. Fire-and-forget, same reasoning as
    /// `PacketReceived`.
    ChatMessage { from: Entity, text: String },
}

/// Snapshot of a User entity and the Device entities currently attached to it.
pub struct UserView {
    pub username: String,
    pub devices: Vec<Entity>,
}
