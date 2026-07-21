use anyhow::Result;
use bytes::Bytes;
use hecs::Entity;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use sheeko::common::packet;
use sheeko::ecs::components::RoomId;
use sheeko::ecs::{self, Command, DriverHandle};

/// M5 has no gateway/voice_node session negotiation yet — every peer that
/// sends a packet here joins this one room. A packet's `ssrc` stands in for
/// "which logical user this device belongs to" (its real RTP purpose):
/// multiple UDP peers sending the same `ssrc` are treated as multiple
/// Devices of one User, exactly like a phone and a laptop would be.
const ROOM: RoomId = RoomId(0);

#[tokio::main]
async fn main() -> Result<()> {
    sheeko::telemetry::init();

    let socket = Arc::new(UdpSocket::bind("127.0.0.1:5000").await?);
    info!("voice_node listening on 127.0.0.1:5000");
    info!("control: type \"switch <ssrc> <device_addr>\" and press enter to move the active audio source");

    let driver = ecs::spawn_driver();
    let mut peers: HashMap<SocketAddr, Entity> = HashMap::new();
    let mut users_by_ssrc: HashMap<u32, Entity> = HashMap::new();

    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut buf = [0u8; 1500];

    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                let (n, addr) = result?;
                let packet = Bytes::copy_from_slice(&buf[..n]);

                let Ok((header, _payload)) = packet::decode(packet.clone()) else {
                    warn!(%addr, "dropped malformed packet");
                    continue;
                };

                let device = match peers.get(&addr) {
                    Some(&entity) => entity,
                    None => {
                        let (owner, is_first_device) = match users_by_ssrc.get(&header.ssrc) {
                            Some(&user) => (user, false),
                            None => {
                                let user = create_user(&driver, header.ssrc).await;
                                users_by_ssrc.insert(header.ssrc, user);
                                (user, true)
                            }
                        };

                        let entity =
                            register_peer(&driver, socket.clone(), addr, owner, is_first_device).await;
                        info!(%addr, ssrc = header.ssrc, ?entity, as_source = is_first_device, "registered voice peer");
                        peers.insert(addr, entity);
                        entity
                    }
                };

                driver.send(Command::PacketReceived { from: device, packet });
            }
            line = lines.next_line() => {
                let Some(text) = line? else { break };
                handle_control_line(&text, &driver, &users_by_ssrc, &peers).await;
            }
        }
    }

    Ok(())
}

async fn create_user(driver: &DriverHandle, ssrc: u32) -> Entity {
    let (reply, rx) = oneshot::channel();
    driver.send(Command::CreateUser {
        username: format!("ssrc-{ssrc}"),
        reply,
    });
    rx.await.expect("driver task outlives every voice_node connection")
}

/// Registers `addr` as a Device entity owned by `owner` and spawns the task
/// that owns writing back out to it — the driver only ever sees the
/// `OutboundTx` sender end, never the socket or address directly.
async fn register_peer(
    driver: &DriverHandle,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    owner: Entity,
    as_source: bool,
) -> Entity {
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
        owner,
        room: ROOM,
        as_source,
        outbound: tx,
        reply,
    });
    rx.await
        .expect("driver task outlives every voice_node connection")
        .expect("owner was just created by this same function")
}

/// Parses and executes a `switch <ssrc> <device_addr>` control line typed
/// into voice_node's own stdin — a minimal, dependency-free way to trigger
/// the ECS Gold Standard by hand without a second control-plane server.
async fn handle_control_line(
    text: &str,
    driver: &DriverHandle,
    users_by_ssrc: &HashMap<u32, Entity>,
    peers: &HashMap<SocketAddr, Entity>,
) {
    let mut parts = text.split_whitespace();
    match (parts.next(), parts.next(), parts.next()) {
        (Some("switch"), Some(ssrc_str), Some(addr_str)) => {
            let Ok(ssrc) = ssrc_str.parse::<u32>() else {
                warn!(%ssrc_str, "not a valid ssrc");
                return;
            };
            let Ok(addr) = addr_str.parse::<SocketAddr>() else {
                warn!(%addr_str, "not a valid socket address");
                return;
            };
            let Some(&user) = users_by_ssrc.get(&ssrc) else {
                warn!(ssrc, "no user registered for this ssrc yet");
                return;
            };
            let Some(&to_device) = peers.get(&addr) else {
                warn!(%addr, "no device registered at this address yet");
                return;
            };

            let (reply, reply_rx) = oneshot::channel();
            driver.send(Command::SwitchAudioDevice {
                user,
                to_device,
                reply,
            });
            match reply_rx.await {
                Ok(Ok(())) => info!(ssrc, %addr, "switched active audio source"),
                Ok(Err(err)) => warn!(ssrc, %addr, %err, "switch rejected"),
                Err(_) => warn!("driver did not respond to switch request"),
            }
        }
        _ => warn!(%text, "unrecognized control line; expected \"switch <ssrc> <device_addr>\""),
    }
}
