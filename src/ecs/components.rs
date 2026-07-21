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
