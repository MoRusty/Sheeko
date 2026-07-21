use anyhow::Result;
use bytes::Bytes;
use hecs::Entity;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use sheeko::ecs::components::RoomId;
use sheeko::ecs::{self, Command, DriverHandle};

/// M3 has no room/session negotiation yet (that's M5, tied to the gateway's
/// `/api/join`) — every peer that sends a packet here joins this one room.
const ROOM: RoomId = RoomId(0);

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let socket = Arc::new(UdpSocket::bind("127.0.0.1:5000").await?);
    info!("voice_node listening on 127.0.0.1:5000");

    let driver = ecs::spawn_driver();
    let mut peers: HashMap<SocketAddr, Entity> = HashMap::new();

    let mut buf = [0u8; 1500];
    loop {
        let (n, addr) = socket.recv_from(&mut buf).await?;
        let packet = Bytes::copy_from_slice(&buf[..n]);

        let device = match peers.get(&addr) {
            Some(&entity) => entity,
            None => {
                let entity = register_peer(&driver, socket.clone(), addr).await;
                info!(%addr, ?entity, "registered new voice peer");
                peers.insert(addr, entity);
                entity
            }
        };

        driver.send(Command::PacketReceived {
            from: device,
            packet,
        });
    }
}

/// Registers `addr` as a Device entity and spawns the task that owns writing
/// back out to it — the driver only ever sees the `OutboundTx` sender end,
/// never the socket or address directly.
async fn register_peer(driver: &DriverHandle, socket: Arc<UdpSocket>, addr: SocketAddr) -> Entity {
    let (tx, mut rx) = mpsc::unbounded_channel::<Bytes>();

    tokio::spawn(async move {
        while let Some(bytes) = rx.recv().await {
            if let Err(err) = socket.send_to(&bytes, addr).await {
                warn!(%addr, %err, "failed to forward packet");
            }
        }
    });

    let (reply, rx) = oneshot::channel();
    driver.send(Command::RegisterVoicePeer {
        room: ROOM,
        outbound: tx,
        reply,
    });
    rx.await
        .expect("driver task outlives every voice_node connection")
}
