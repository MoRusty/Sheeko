use bytes::Bytes;
use hecs::Entity;
use tokio::sync::mpsc;

use crate::common::jitter::SequenceTracker;

/// A User entity's logical identity. Persists across however many Device
/// entities are currently attached to it.
#[derive(Debug, Clone)]
pub struct Identity {
    pub username: String,
}

/// Links a Device entity back to the User entity that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedBy(pub Entity);

/// Marker: this Device entity is currently producing audio for its owning
/// User. Moving "who's producing my audio" between two Devices of the same
/// User is just removing this component from one entity and inserting it on
/// another — see PDR M5.
pub struct AudioSource;

/// Marker: this Device entity wants to receive audio for its owning User.
pub struct AudioSink;

/// Identifies a room. Rooms are just an identity anchor — membership is
/// queried via `RoomMembership` on Device entities, not a `Vec<Entity>`
/// stored on a Room entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomId(pub u64);

/// Which room a Device entity is currently active in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomMembership(pub RoomId);

/// How the driver task sends bytes back out to a Device: the far end (a UDP
/// recv-loop task, a WS write half, ...) owns the matching receiver and
/// forwards whatever it's given — the driver never touches a socket directly.
pub struct OutboundTx(pub mpsc::UnboundedSender<Bytes>);

/// Per-producer sequence-number tracking (see `common::jitter`), scoped to
/// whichever Device entity is currently sending audio.
pub struct JitterState(pub SequenceTracker);

/// Marker: this Device entity is a WS text session and should receive chat
/// fan-out for its room. Two Devices of the same User can both carry this —
/// the fan-out doesn't special-case "same user, multiple devices" at all.
pub struct TextChannel;
