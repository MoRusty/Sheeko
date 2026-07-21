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
    /// `as_audio_source` exists so the gateway's REST API can create a device
    /// that `SwitchAudioDevice` has something to switch to/from without
    /// needing a real voice_node connection — see PDR M5.
    CreateDevice {
        owner: Entity,
        as_audio_source: bool,
        reply: oneshot::Sender<Option<Entity>>,
    },
    GetUser {
        user: Entity,
        reply: oneshot::Sender<Option<UserView>>,
    },
    /// Registers a voice peer as a Device entity owned by `owner`, in `room`,
    /// tagged `AudioSink` and — only if `as_source` — `AudioSource` too (the
    /// first device registered for a user starts as the active source; later
    /// devices join muted until switched to, see `SwitchAudioDevice`). Fails
    /// (sends `None`) if `owner` isn't a live User entity.
    RegisterVoicePeer {
        owner: Entity,
        room: RoomId,
        as_source: bool,
        outbound: mpsc::UnboundedSender<Bytes>,
        reply: oneshot::Sender<Option<Entity>>,
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
    /// Moves the active `AudioSource` for `user` onto `to_device`. Fails if
    /// `to_device` isn't owned by `user` — see `ecs::systems::device_switch`.
    SwitchAudioDevice {
        user: Entity,
        to_device: Entity,
        reply: oneshot::Sender<Result<(), super::systems::device_switch::DeviceSwitchError>>,
    },
}

/// Snapshot of a User entity and the Device entities currently attached to it.
pub struct UserView {
    pub username: String,
    pub devices: Vec<Entity>,
    pub online: bool,
}
