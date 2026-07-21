use hecs::{Entity, World};
use tokio::sync::mpsc;
use tracing::debug;

use super::commands::{Command, UserView};
use super::components::{AudioSink, AudioSource, Identity, JitterState, OutboundTx, OwnedBy, RoomMembership};
use super::systems::audio_forward;
use crate::common::jitter::SequenceTracker;
use crate::common::packet;

/// Handle for sending `Command`s into the single task that owns the `World`.
/// Cloning this is cheap and safe to hand out to every connection task.
#[derive(Clone)]
pub struct DriverHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl DriverHandle {
    /// The driver task runs for the lifetime of the process, so a closed
    /// channel here would mean the driver panicked; nothing left to do but
    /// drop the command.
    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }
}

/// Spawns the driver task and returns a handle to it. There must only ever be
/// one of these per process: the driver task is the sole holder of `&mut World`.
pub fn spawn_driver() -> DriverHandle {
    let (tx, mut rx) = mpsc::unbounded_channel::<Command>();

    tokio::spawn(async move {
        let mut world = World::new();
        while let Some(cmd) = rx.recv().await {
            handle_command(&mut world, cmd);
        }
    });

    DriverHandle { tx }
}

/// Applies one `Command` to `world`. The driver task is the only caller in
/// production; tests call this directly to exercise systems without needing
/// a running driver or real sockets.
pub fn handle_command(world: &mut World, cmd: Command) {
    match cmd {
        Command::CreateUser { username, reply } => {
            let entity = world.spawn((Identity { username },));
            debug!(?entity, "created user");
            let _ = reply.send(entity);
        }
        Command::CreateDevice { owner, reply } => {
            if world.contains(owner) {
                let entity = world.spawn((OwnedBy(owner),));
                debug!(?entity, ?owner, "created device");
                let _ = reply.send(Some(entity));
            } else {
                let _ = reply.send(None);
            }
        }
        Command::GetUser { user, reply } => {
            let username = world
                .get::<&Identity>(user)
                .ok()
                .map(|identity| identity.username.clone());

            let view = username.map(|username| {
                let devices: Vec<Entity> = world
                    .query::<(Entity, &OwnedBy)>()
                    .iter()
                    .filter(|(_, owned)| owned.0 == user)
                    .map(|(entity, _)| entity)
                    .collect();
                UserView { username, devices }
            });

            let _ = reply.send(view);
        }
        Command::RegisterVoicePeer {
            room,
            outbound,
            reply,
        } => {
            let entity = world.spawn((
                RoomMembership(room),
                AudioSource,
                AudioSink,
                OutboundTx(outbound),
                JitterState(SequenceTracker::new()),
            ));
            debug!(?entity, room = room.0, "registered voice peer");
            let _ = reply.send(entity);
        }
        Command::PacketReceived { from, packet: bytes } => {
            let Ok((header, _payload)) = packet::decode(bytes.clone()) else {
                debug!(?from, "dropped malformed packet");
                return;
            };

            if let Ok(mut jitter) = world.get::<&mut JitterState>(from) {
                jitter.0.observe(header.sequence);
            }

            let room = world.get::<&RoomMembership>(from).ok().map(|m| m.0);
            if let Some(room) = room {
                audio_forward::run(world, room, from, &bytes);
            }
        }
    }
}

/// Stable external representation of an `Entity`, for use in URLs/JSON.
pub fn entity_to_id(entity: Entity) -> String {
    entity.to_bits().get().to_string()
}

/// Inverse of `entity_to_id`. Returns `None` for malformed input; note this
/// does not by itself confirm the entity is still alive in the `World`.
pub fn id_to_entity(id: &str) -> Option<Entity> {
    id.parse::<u64>().ok().and_then(Entity::from_bits)
}
