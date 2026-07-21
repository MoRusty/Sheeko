# Project Definition Report (PDR) - RustCord

## 1. Executive Summary
RustCord is an educational and functional prototype of a real-time communication platform (similar to Discord), built **data-oriented (ECS) rather than object-oriented**. Where a typical implementation models a "user" as a monolithic object owning one connection, RustCord decouples logical identity (a User entity) from physical connections (Device entities): a single user can have a laptop producing video and a phone producing audio at the same time — not two accounts, the same user, with the active audio/video source movable between their devices at runtime. The project prioritizes a deep understanding of this entity/component design alongside asynchronous networking, audio codecs, and real-time UDP forwarding (SFU), over building a production-grade GUI. The outcome will be a CLI-based system where two users can text chat via WebSockets and voice chat via UDP with sub-200ms latency, with the multi-device-per-user model working end-to-end.

## 2. Project Objectives
- **Primary**: Build a minimal viable backend that supports 1-to-1 text messaging and 3-user voice chat.
- **Core thesis**: Model Users and Devices as separate ECS entities (via `hecs`) so a single logical identity can have multiple simultaneously-connected devices, each contributing different capabilities (audio/video/text), with capabilities movable between devices at runtime without a reconnect.
- **Secondary**: Establish a reusable Rust codebase for low-latency audio networking.
- **Learning**: Master Tokio async, WebSockets, UDP sockets, the Opus codec, and data-oriented ECS design in Rust.

## 3. Scope

### In-Scope
- ECS-based state model (`hecs`): User and Device entities, capability components, a single-writer driver task as the sole owner of the `World`.
- REST API for user/device registration and channel listing (in-memory ECS state, no database).
- Persistent WebSocket connection for real-time text, fanned out to every Device entity in a room regardless of which User owns it.
- UDP voice server capable of receiving encrypted Opus packets and forwarding them (SFU) by querying Device entities' room membership and capability components.
- Runtime capability switching: move the active `AudioSource` (or other capability) from one Device entity to another belonging to the same User, without dropping the Room/User state.
- Local audio capture and playback using `cpal`.
- Basic jitter detection (log dropped/reordered packets).

### Out-of-Scope
- Video streaming / screen sharing (the Device/component model supports a future `VideoSource`/`VideoSink`, but no video codec or transport is implemented in this project).
- User permissions / role-based access control (RBAC).
- Database persistence (PostgreSQL) for the initial MVP (we use the in-memory ECS `World`).
- Full WebRTC browser compatibility (we target custom Rust CLI clients first).
- Global load balancing or Kubernetes deployment.
- A general-purpose or reusable ECS crate — `hecs` is used as-is; we are not building our own ECS.

## 4. Architectural Overview
- **ECS Driver Task**: a single Tokio task owns the `hecs::World` exclusively. All other tasks (one per TCP/WS connection, one recv-loop per UDP socket) communicate with it only via `Command` messages over an `mpsc` channel — never a shared/locked reference to the `World` itself.
- **Gateway**: Axum server listening on `:3030`. Handles HTTP endpoints to create User/Device entities, `/api/join` to assign a device to a room and a voice-node address, and WebSocket `/ws` for text — all by sending `Command`s to the driver task.
- **Voice Node**: A separate Tokio instance listening on a specific UDP port (e.g., `:5000`). Forwarding logic is an ECS system: query Device entities with `RoomMembership(room) + AudioSource` for producers, `RoomMembership(room) + AudioSink` for consumers, and forward raw bytes between them — no per-room `HashMap<RoomId, Vec<UdpPeer>>` maintained outside the `World`.
- **Client**: A terminal app that spins up two Tokio tasks — one for WebSocket text, one for UDP voice — either of which can be run from a different device than its sibling while still representing the same User entity server-side.

## 5. Milestones & Roadmap

| Milestone | Difficulty | Description | Linked Tasks (from 10-list) | Status |
| :--- | :--- | :--- | :--- | :--- |
| **M1: Foundations + ECS core** | 2/5 | Async TCP echo, UDP ping-pong, and an Axum gateway seed backed by the ECS driver task (User/Device entity creation + lookup) instead of a flat key-value store. | Tasks 1, 2, 7 | Done |
| **M2: Audio Pipeline** | 1/5 | Capture mic, play beep, encode/decode Opus frames; a Device entity tags itself `AudioSource` as soon as it's producing audio. | Tasks 4, 5, 6 | Done |
| **M3: Jitter & Forwarding** | 2/5 | UDP sequence tracker, ECS-based SFU forwarding system (query producers/consumers by room + capability component). | Tasks 8, 10 | Done |
| **M4: Text Gateway** | 2/5 | WebSocket broadcast server fanning out to every `TextChannel`-tagged Device in a room (including multiple devices of the same user). | Tasks 3, 9 | Done |
| **M5: Full Integration** | 3/5 | Combine Gateway + Voice Node; DTLS/SRTP; implement the runtime device-switch endpoint (move `AudioSource` between two Devices of one User mid-call). | All tasks combined. | Not started |

## 6. Technical Specifications
- **Language**: Rust (Latest Stable, Edition 2024).
- **State Model**: Entity-Component-System via `hecs`. A single-writer driver task owns the `World`; all mutation and querying happens through `Command` messages processed reactively (no fixed tick/scheduler needed).
- **Audio Specs**: 48 kHz sampling rate, 20ms packetization (960 samples per frame), Opus bitrate ~32-64 kbps.
- **Transport**:
  - **Text**: JSON over WebSockets.
  - **Voice**: Raw RTP headers + Opus payload over UDP (with DTLS encryption added in M5).
- **Concurrency Model**: Multi-threaded Tokio runtime (work-stealing) for I/O; the ECS `World` itself is mutated by exactly one task to avoid lock contention entirely.

## 7. Risks & Mitigations
| Risk | Impact | Mitigation |
| :--- | :--- | :--- |
| **UDP packet loss over Wi-Fi** | Choppy audio | Implement basic sequence-number logging initially; add lightweight jitter buffer (delayed forwarding) later. |
| **Opus library compilation failures** | Blocked audio phase | Use `audiopus` which bundles the C library; ensure `pkg-config` and `cmake` are installed. |
| **Tokio task sprawl** | Resource exhaustion | Limit concurrent UDP tasks per room; one task per connection/socket, never per-packet; the driver task is the one intentional long-lived singleton. |
| **`hecs` ergonomics unfamiliar for an event-driven server** (most examples are game loops, not async servers) | Wasted time fighting the library mid-feature | De-risk with an isolated spike test (spawn/despawn, add/remove component, run a query) before building real endpoints on top — done in M1. |
| **Accidental concurrent `World` mutation** | Data races / borrow panics | Hard rule: only the driver task ever holds `&mut World`. Anything that looks like it needs concurrent access becomes a new `Command` variant instead. |

## 8. Definition of Done (Success Criteria)
- [x] Gateway can create a User entity and attach multiple Device entities to it, queryable via REST (M1, done).
- [x] Two separate terminal clients can exchange "Hello" via WebSocket (M4, done; verified with real `term_client` processes and automated in `tests/ws_chat.rs`, which also proves one User's two Devices both receive a fan-out from a third party).
- [x] A user can run `cargo run --bin voice_client --features audio` and hear a 440Hz tone (M2, done; also tags a local Device entity `AudioSource`).
- [x] A user can run `cargo run --bin mic_test --features audio` and see amplitude levels in the terminal (M2, done).
- [ ] **Gold Standard**: Client A speaks into the mic; Client B (on the same LAN) receives and plays the audio with less than 200ms round-trip delay, verified via terminal logs.
- [ ] **ECS Gold Standard**: Mid-call, switch the active audio source for one user from Device A to Device B (both attached to the same User entity) and confirm the room keeps receiving audio from the new device with no Room/User-level reconnection.
