pub mod commands;
pub mod components;
pub mod systems;
pub mod world;

pub use commands::{Command, UserView};
pub use world::{DriverHandle, entity_to_id, handle_command, id_to_entity, spawn_driver};

#[cfg(test)]
mod tests {
    use super::*;
    use hecs::{Entity, World};

    /// Spike test de-risking hecs ergonomics before building real endpoints
    /// on top: spawn, add/remove a component, run a query, despawn.
    #[test]
    fn spawn_query_mutate_despawn() {
        let mut world = World::new();

        let user = world.spawn((components::Identity {
            username: "alice".into(),
        },));
        let device = world.spawn((components::OwnedBy(user),));

        let owned: Vec<Entity> = world
            .query::<(Entity, &components::OwnedBy)>()
            .iter()
            .filter(|(_, owned)| owned.0 == user)
            .map(|(entity, _)| entity)
            .collect();
        assert_eq!(owned, vec![device]);

        // capability components can move between entities at runtime
        struct AudioSource;
        world.insert_one(device, AudioSource).unwrap();
        assert!(world.get::<&AudioSource>(device).is_ok());
        world.remove_one::<AudioSource>(device).unwrap();
        assert!(world.get::<&AudioSource>(device).is_err());

        world.despawn(device).unwrap();
        assert!(!world.contains(device));

        // round-trip through the external id representation
        let id = entity_to_id(user);
        assert_eq!(id_to_entity(&id), Some(user));
    }
}
