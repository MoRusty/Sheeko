use hecs::{Entity, World};

use crate::ecs::components::{AudioSource, OwnedBy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DeviceSwitchError {
    #[error("target device is not owned by this user")]
    TargetNotOwnedByUser,
}

/// Moves `AudioSource` from whichever Device entity currently carries it for
/// `user` onto `to_device`. This is the literal implementation of the
/// project's core thesis: no User or Room mutation, just a component moving
/// between two entities — see PDR's "ECS Gold Standard".
pub fn run(world: &mut World, user: Entity, to_device: Entity) -> Result<(), DeviceSwitchError> {
    let target_owned_by_user = world
        .get::<&OwnedBy>(to_device)
        .map(|owned| owned.0 == user)
        .unwrap_or(false);
    if !target_owned_by_user {
        return Err(DeviceSwitchError::TargetNotOwnedByUser);
    }

    let previous_sources: Vec<Entity> = world
        .query::<(Entity, &OwnedBy, &AudioSource)>()
        .iter()
        .filter(|(_, owned, _)| owned.0 == user)
        .map(|(entity, _, _)| entity)
        .collect();

    for entity in previous_sources {
        let _ = world.remove_one::<AudioSource>(entity);
    }

    if world.get::<&AudioSource>(to_device).is_err() {
        let _ = world.insert_one(to_device, AudioSource);
    }

    Ok(())
}
