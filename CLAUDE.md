# CLAUDE.md - RustCord (ECS Discord Clone)

## Project Overview
This is a high-performance, real-time communication server built in Rust, mimicking Discord's core architecture (Text over WebSockets, Voice over UDP/SFU) — but built **data-oriented (ECS), not object-oriented**. The central design thesis: a "User" is a logical identity decoupled from any single device/connection. Multiple devices (phone, laptop, browser tab) can attach to the *same* user simultaneously, each contributing a different capability — one device streams video, another streams audio — and a capability like "who's producing my audio right now" can move between devices at runtime without dropping the room or reconnecting. An OOP model where a `User` struct owns its socket would force one connection per identity and make device-switching a special case; ECS makes it the default. The project is structured as a learning pathway, starting from basic networking primitives and scaling up to a fully functional voice gateway.

## Core Architecture
- **ECS World & Driver Task**: An `hecs::World` holds all live state (Users, Devices, Rooms) as entities with components. A single dedicated Tokio task owns the `World` exclusively (single-writer); every other task — one per TCP/WS connection, one recv-loop per UDP socket — sends `Command` messages into it over an `mpsc` channel rather than touching it directly. This replaces `Arc<Mutex<...>>`/`DashMap`-style shared state entirely; see [Entity/Component Model](#entitycomponent-model) below.
- **Gateway (API + Text)**: Axum web server handling HTTP REST endpoints and upgrading to WebSockets (`tokio-tungstenite`) for real-time text chat. Endpoints translate requests into `Command`s sent to the ECS driver task.
- **Voice Server (SFU)**: A separate Tokio runtime process listening on UDP ports. Uses an **SFU (Selective Forwarding Unit)** model: it forwards encrypted Opus/RTP packets between peers without decoding them to save CPU. Forwarding decisions are ECS queries (which Device entities are in this room with an `AudioSink` component), not a hand-rolled `HashMap<RoomId, Vec<SocketAddr>>`.
- **Persistence**: In-memory only via the ECS `World` for now. Persistent data eventually uses PostgreSQL (`sqlx`) if ever needed beyond the World's lifetime.

## Entity/Component Model
- **User entity**: logical identity. Components: `Identity { username }`, `Presence` (aggregated online/offline from its devices).
- **Device entity**: one per physical connection. Components: `OwnedBy(user_entity)`, `Transport`/`OutboundTx` (how to reach it), `RoomMembership(RoomId)`, and dynamic capability markers — `AudioSource`, `AudioSink`, `VideoSource`, `VideoSink`, `TextChannel` — added/removed at runtime as the user reconfigures which device does what.
- **Room entity**: identity anchor; membership is queried via `RoomMembership` on Device entities, not a `Vec<Entity>` stored on Room.
- **Device switching**: to move "who's producing my audio" from one device to another, remove `AudioSource` from the old Device entity and add it to the new one. No User or Room mutation needed — the forwarding system's query just returns a different entity next pass. This is the literal implementation of the project's core thesis.
- **Concurrency rule**: never give a second task `&mut World`. If something seems to need concurrent mutation, it belongs in its own `Command` variant processed by the single driver task, not a reason to reach for `Arc<Mutex<World>>`.

## Project Phases (Derived from 10 Baby Steps)
0. **Phase 0: Foundations + ECS core** - Async TCP/UDP, basic Axum routing, and the ECS driver task / User+Device entity model established from day one (not bolted on later).
1. **Phase 1: Audio Pipeline** - `cpal` for mic/speakers, `audiopus` for Opus codec; devices tag themselves with capability components (e.g. `AudioSource`) as soon as audio exists.
2. **Phase 2: Real-time Transport** - UDP jitter handling + ECS-based SFU forwarding (query producers/consumers by room membership and capability component).
3. **Phase 3: Text Gateway** - WebSocket chat room with the same component-based fan-out (multiple devices of one user can both hold `TextChannel` and both receive a broadcast).
4. **Phase 4: The Voice Server** - DTLS/SRTP handshake (`webrtc-rs`), full integration, and the runtime device-switch feature made concrete end-to-end.

## Key Crates & Their Roles
- `tokio`: Async runtime (networking, timers, spawning tasks).
- `hecs`: Minimal archetypal ECS — the `World` that models Users/Devices/Rooms as entities and components. No built-in scheduler; systems are plain functions called reactively by the driver task.
- `axum`: HTTP server + WebSocket upgrade.
- `tokio-tungstenite`: WebSocket protocol implementation.
- `audiopus`: Safe Rust bindings to Opus (encode/decode 48kHz/20ms frames).
- `cpal`: Cross-platform audio I/O (mic capture and speaker playback).
- `webrtc-rs`: Used for DTLS handshake and SRTP encryption (advanced phases).
- `tracing` / `tracing-subscriber`: Structured logging (debugging UDP packet flow, ECS command flow).

## Coding Guidelines (for AI assistance)
- **Error Handling**: Use `anyhow::Result` for binaries, `thiserror` for custom library errors.
- **Concurrency**: Prefer `tokio::spawn` for CPU-bound or blocking tasks (via `spawn_blocking` for Opus encoding). Shared state lives in the ECS `World`, owned by a single driver task — do **not** wrap it in `Arc<Mutex<...>>` or reach for `DashMap`; add a `Command` variant instead.
- **UDP Notes**: Audio packets are lossy. We do **not** retransmit. We rely on RTP sequence numbers to detect jitter.
- **Performance**: Avoid unnecessary heap allocations in the UDP hot loop. Use `Bytes` or `Vec<u8>` with pre-allocated buffers.

## Testing Strategy
- **Unit tests**: Opus encode/decode round-trips; ECS component add/remove/query ergonomics (see the `hecs` spike test in `src/ecs/mod.rs`).
- **Integration tests**: Spawn a `World` with fake Device entities (channels standing in for real sockets) and call system functions (e.g. `audio_forward_system`) directly — no real networking required. Separately, spawn a local UDP server and client to simulate packet forwarding end-to-end.
- **Manual testing**: Use `socat` or custom CLI clients to send raw UDP packets to validate the server logic; `curl` against the gateway to exercise User/Device entity creation.

## Current Status
Check the PDR (Project Definition Report) for milestone tracking. M1 (Foundations + ECS core), M2 (Audio Pipeline), and M3 (Jitter & Forwarding) are complete:
- `tcp_echo`, `udp_ping_pong`, the `gateway` seed (ECS driver task + User/Device entity endpoints), `mic_test`, and `voice_client` (440Hz tone + local `AudioSource` tagging) are all implemented and verified.
- Opus round-trips are unit-tested (`src/common/opus_codec.rs`); the `audio` feature is confirmed to stay out of non-audio binaries.
- `common::jitter::SequenceTracker` detects drops/reorders across `u16` wraparound; `common::packet` frames raw RTP-like packets over `Bytes`.
- `voice_node` is a real UDP SFU: `ecs::systems::audio_forward` queries Device entities by `RoomMembership` + `AudioSink` and forwards raw, undecoded packets via each one's `OutboundTx`; verified with a real two-peer UDP exchange (registration, room-scoped forwarding, no self-echo) plus an in-process `tests/ecs_forward.rs` integration test.

`cargo clippy --all-targets` (with and without `--features audio`) is clean. Currently moving into M4 (Text Gateway).
