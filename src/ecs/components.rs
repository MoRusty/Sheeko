use hecs::Entity;

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
