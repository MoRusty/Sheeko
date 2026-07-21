use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::info;

use sheeko::ecs::{self, Command, DriverHandle};

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
}

#[derive(Serialize)]
struct CreateUserResponse {
    user_id: String,
}

#[derive(Serialize)]
struct CreateDeviceResponse {
    device_id: String,
}

#[derive(Serialize)]
struct UserResponse {
    user_id: String,
    username: String,
    devices: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let driver = ecs::spawn_driver();
    let app = Router::new()
        .route("/users", post(create_user))
        .route("/users/{user_id}/devices", post(create_device))
        .route("/users/{user_id}", get(get_user))
        .with_state(driver);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3030").await?;
    info!("gateway listening on 127.0.0.1:3030");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn create_user(
    State(driver): State<DriverHandle>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), StatusCode> {
    let (reply, rx) = oneshot::channel();
    driver.send(Command::CreateUser {
        username: req.username,
        reply,
    });
    let entity = rx.await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user_id: ecs::entity_to_id(entity),
        }),
    ))
}

async fn create_device(
    State(driver): State<DriverHandle>,
    Path(user_id): Path<String>,
) -> Result<(StatusCode, Json<CreateDeviceResponse>), StatusCode> {
    let owner = ecs::id_to_entity(&user_id).ok_or(StatusCode::NOT_FOUND)?;

    let (reply, rx) = oneshot::channel();
    driver.send(Command::CreateDevice { owner, reply });
    let device = rx
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateDeviceResponse {
            device_id: ecs::entity_to_id(device),
        }),
    ))
}

async fn get_user(
    State(driver): State<DriverHandle>,
    Path(user_id): Path<String>,
) -> Result<Json<UserResponse>, StatusCode> {
    let user = ecs::id_to_entity(&user_id).ok_or(StatusCode::NOT_FOUND)?;

    let (reply, rx) = oneshot::channel();
    driver.send(Command::GetUser { user, reply });
    let view = rx
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(UserResponse {
        user_id,
        username: view.username,
        devices: view.devices.into_iter().map(ecs::entity_to_id).collect(),
    }))
}
