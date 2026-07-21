use hecs::Entity;
use tokio::sync::oneshot;

/// Messages sent into the ECS driver task. The driver is the only task that
/// ever holds `&mut World`; every mutation and query goes through here.
pub enum Command {
    CreateUser {
        username: String,
        reply: oneshot::Sender<Entity>,
    },
    /// Fails (sends `None`) if `owner` does not refer to a live User entity.
    CreateDevice {
        owner: Entity,
        reply: oneshot::Sender<Option<Entity>>,
    },
    GetUser {
        user: Entity,
        reply: oneshot::Sender<Option<UserView>>,
    },
}

/// Snapshot of a User entity and the Device entities currently attached to it.
pub struct UserView {
    pub username: String,
    pub devices: Vec<Entity>,
}
