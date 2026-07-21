use bytes::Bytes;
use hecs::World;
use tokio::sync::mpsc;

use sheeko::common::jitter::SequenceTracker;
use sheeko::common::packet;
use sheeko::ecs::commands::Command;
use sheeko::ecs::components::{AudioSink, AudioSource, JitterState, OutboundTx, RoomId, RoomMembership};

const ROOM: RoomId = RoomId(1);
const OTHER_ROOM: RoomId = RoomId(999);

/// Exercises `Command::PacketReceived` -> `audio_forward` end to end against
/// a bare `World`, with mpsc channels standing in for real sockets — no
/// networking, no driver task, just the same `handle_command` the driver
/// calls in production.
#[test]
fn packet_received_forwards_to_room_peers_but_not_the_sender() {
    let mut world = World::new();

    let (sender_tx, mut sender_rx) = mpsc::unbounded_channel::<Bytes>();
    let sender = world.spawn((
        RoomMembership(ROOM),
        AudioSource,
        AudioSink,
        OutboundTx(sender_tx),
        JitterState(SequenceTracker::new()),
    ));

    let (sink_tx, mut sink_rx) = mpsc::unbounded_channel::<Bytes>();
    let _sink = world.spawn((RoomMembership(ROOM), AudioSink, OutboundTx(sink_tx)));

    // a device in a different room must never receive this packet
    let (other_room_tx, mut other_room_rx) = mpsc::unbounded_channel::<Bytes>();
    let _other_room_device =
        world.spawn((RoomMembership(OTHER_ROOM), AudioSink, OutboundTx(other_room_tx)));

    let header = packet::Header {
        sequence: 1,
        timestamp: 0,
        ssrc: 42,
    };
    let raw = packet::encode(header, b"opus payload");

    sheeko::ecs::handle_command(
        &mut world,
        Command::PacketReceived {
            from: sender,
            packet: raw.clone(),
        },
    );

    let forwarded = sink_rx
        .try_recv()
        .expect("sink should have received the forwarded packet");
    assert_eq!(forwarded, raw);

    assert!(
        sender_rx.try_recv().is_err(),
        "sender must not receive its own packet back"
    );
    assert!(
        other_room_rx.try_recv().is_err(),
        "a device in a different room must not receive the packet"
    );
}
