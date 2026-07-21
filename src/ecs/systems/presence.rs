use hecs::{Entity, World};

use crate::ecs::components::OwnedBy;

/// A User entity has no separate "online" flag to go stale — presence is
/// computed on demand from whether any Device entity currently exists with
/// `OwnedBy(user)`. There's no disconnect/despawn path yet (M5 doesn't add
/// one), so this is necessarily just "has at least one registered device".
pub fn is_online(world: &World, user: Entity) -> bool {
    world.query::<&OwnedBy>().iter().any(|owned| owned.0 == user)
}
