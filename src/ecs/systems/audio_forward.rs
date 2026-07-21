use bytes::Bytes;
use hecs::{Entity, World};

use crate::ecs::components::{AudioSink, OutboundTx, RoomId, RoomMembership};

/// Forwards `packet` (raw, undecoded — CLAUDE.md's SFU model) to every Device
/// entity in `room` with an `AudioSink`, except `from` itself.
pub fn run(world: &World, room: RoomId, from: Entity, packet: &Bytes) {
    for (entity, _sink, membership, tx) in world
        .query::<(Entity, &AudioSink, &RoomMembership, &OutboundTx)>()
        .iter()
    {
        if entity != from && membership.0 == room {
            let _ = tx.0.send(packet.clone());
        }
    }
}
