use bytes::Bytes;
use hecs::{Entity, World};

use crate::ecs::components::{OutboundTx, RoomId, RoomMembership, TextChannel};

/// Fans `message` (a serialized `ServerMessage`) out to every Device entity
/// in `room` with a `TextChannel`, except `from`. If one User has two
/// Devices both in this room, both get it — no special-casing needed.
pub fn run(world: &World, room: RoomId, from: Entity, message: &Bytes) {
    for (entity, _channel, membership, tx) in world
        .query::<(Entity, &TextChannel, &RoomMembership, &OutboundTx)>()
        .iter()
    {
        if entity != from && membership.0 == room {
            let _ = tx.0.send(message.clone());
        }
    }
}
