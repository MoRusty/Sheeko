use bytes::Bytes;
use hecs::World;
use tokio::sync::{mpsc, oneshot};

use sheeko::common::jitter::SequenceTracker;
use sheeko::common::packet::{self, Header};
use sheeko::ecs::commands::Command;
use sheeko::ecs::components::{
    AudioSink, AudioSource, Identity, JitterState, OutboundTx, OwnedBy, RoomId, RoomMembership,
};

const ROOM: RoomId = RoomId(7);

fn sample_packet() -> Bytes {
    packet::encode(
        Header {
            sequence: 1,
            timestamp: 0,
            ssrc: 1,
        },
        b"hello",
    )
}

/// The literal ECS Gold Standard: switching `AudioSource` from one Device to
/// another, both owned by the same User, changes which device's packets
/// actually get forwarded — proving the switch is a real gate, not cosmetic.
#[test]
fn switch_moves_audio_source_and_gates_forwarding() {
    let mut world = World::new();
    let user = world.spawn((Identity {
        username: "carol".into(),
    },));

    let (a_tx, _a_rx) = mpsc::unbounded_channel::<Bytes>();
    let device_a = world.spawn((
        OwnedBy(user),
        RoomMembership(ROOM),
        AudioSource,
        AudioSink,
        OutboundTx(a_tx),
        JitterState(SequenceTracker::new()),
    ));

    let (b_tx, _b_rx) = mpsc::unbounded_channel::<Bytes>();
    let device_b = world.spawn((
        OwnedBy(user),
        RoomMembership(ROOM),
        AudioSink,
        OutboundTx(b_tx),
        JitterState(SequenceTracker::new()),
    ));

    let (listener_tx, mut listener_rx) = mpsc::unbounded_channel::<Bytes>();
    let _listener = world.spawn((RoomMembership(ROOM), AudioSink, OutboundTx(listener_tx)));

    assert!(world.get::<&AudioSource>(device_a).is_ok());
    assert!(world.get::<&AudioSource>(device_b).is_err());

    // Before switching: device_a forwards, device_b (no AudioSource) is dropped.
    sheeko::ecs::handle_command(
        &mut world,
        Command::PacketReceived {
            from: device_a,
            packet: sample_packet(),
        },
    );
    assert!(
        listener_rx.try_recv().is_ok(),
        "device_a should forward before switch"
    );

    sheeko::ecs::handle_command(
        &mut world,
        Command::PacketReceived {
            from: device_b,
            packet: sample_packet(),
        },
    );
    assert!(
        listener_rx.try_recv().is_err(),
        "device_b should be dropped before switch (no AudioSource)"
    );

    let (reply, mut reply_rx) = oneshot::channel();
    sheeko::ecs::handle_command(
        &mut world,
        Command::SwitchAudioDevice {
            user,
            to_device: device_b,
            reply,
        },
    );
    assert!(reply_rx.try_recv().unwrap().is_ok());

    assert!(
        world.get::<&AudioSource>(device_a).is_err(),
        "device_a should have lost AudioSource"
    );
    assert!(
        world.get::<&AudioSource>(device_b).is_ok(),
        "device_b should now have AudioSource"
    );

    // After switching: device_a is now dropped, device_b forwards.
    sheeko::ecs::handle_command(
        &mut world,
        Command::PacketReceived {
            from: device_a,
            packet: sample_packet(),
        },
    );
    assert!(
        listener_rx.try_recv().is_err(),
        "device_a should be dropped after switch"
    );

    sheeko::ecs::handle_command(
        &mut world,
        Command::PacketReceived {
            from: device_b,
            packet: sample_packet(),
        },
    );
    assert!(
        listener_rx.try_recv().is_ok(),
        "device_b should forward after switch"
    );
}

#[test]
fn switch_to_device_not_owned_by_user_fails() {
    let mut world = World::new();
    let user_a = world.spawn((Identity {
        username: "a".into(),
    },));
    let user_b = world.spawn((Identity {
        username: "b".into(),
    },));

    let (tx_a, _rx_a) = mpsc::unbounded_channel::<Bytes>();
    let device_a = world.spawn((
        OwnedBy(user_a),
        RoomMembership(ROOM),
        AudioSource,
        AudioSink,
        OutboundTx(tx_a),
        JitterState(SequenceTracker::new()),
    ));

    let (tx_b, _rx_b) = mpsc::unbounded_channel::<Bytes>();
    let device_b = world.spawn((
        OwnedBy(user_b),
        RoomMembership(ROOM),
        AudioSink,
        OutboundTx(tx_b),
        JitterState(SequenceTracker::new()),
    ));

    let (reply, mut reply_rx) = oneshot::channel();
    sheeko::ecs::handle_command(
        &mut world,
        Command::SwitchAudioDevice {
            user: user_a,
            to_device: device_b,
            reply,
        },
    );
    assert!(reply_rx.try_recv().unwrap().is_err());
    assert!(
        world.get::<&AudioSource>(device_a).is_ok(),
        "device_a should keep AudioSource on a failed switch"
    );
}
