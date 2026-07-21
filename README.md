# Sheeko — an ECS Discord clone

A real-time text + voice server, built **data-oriented (ECS)** instead of object-oriented. A `User` is a logical identity, decoupled from any single connection — a phone and a laptop can both be attached to the same account at once, each contributing a different capability (say, one streaming audio, one just watching text), and which device is "live" can move between them at runtime with no reconnect. An OOP model where a `User` struct owns its socket can't express that without special-casing; here it's just moving a component between two entities.

Text runs over WebSockets, voice over a UDP SFU (Selective Forwarding Unit) that forwards Opus/RTP-style packets without decoding them. Built as a learning project — Tokio, WebSockets, Opus, and `hecs` from first principles — but every milestone below is real, tested code, not a sketch.

## The thesis, concretely

```
User (entity)
 ├─ Device A ── OwnedBy(User), RoomMembership, AudioSource, AudioSink
 └─ Device B ── OwnedBy(User), RoomMembership, AudioSink
```

Switching the active microphone from Device A to Device B is: remove `AudioSource` from A, add it to B. No reconnect, no room mutation, no special case for "the same user on two devices." The SFU's forwarding query (`RoomMembership + AudioSource`) just returns a different entity on the next packet. This is implemented, tested, and verified live — see `PUT /users/{id}/audio-device/{device_id}` and `tests/device_switch.rs`.

## Architecture

A single Tokio task owns the entire `hecs::World` (all Users, Devices, Rooms). Every other task — one per TCP/WS connection, one recv loop per UDP socket — only ever sends it a `Command` over a channel; nothing else touches the `World` directly. No `Arc<Mutex<...>>`, no lock contention, no shared-state bugs by construction.

```
┌────────────┐   Command    ┌──────────────────┐
│  gateway   │ ───────────▶ │                  │
│ (REST/WS)  │              │   ECS driver      │
└────────────┘              │  (hecs::World,    │
┌────────────┐   Command    │  single writer)   │
│ voice_node │ ───────────▶ │                  │
│ (UDP SFU)  │              └──────────────────┘
└────────────┘
```

`gateway` and `voice_node` are separate OS processes (separate `World`s); a UDP packet's `ssrc` field is what currently links multiple voice peers together as "devices of one user" (see [CLAUDE.md](CLAUDE.md) for the full component/system breakdown).

## Quickstart

```bash
# text chat
cargo run --bin gateway                       # serves REST + /ws on :3030
cargo run --bin term_client -- <user_id> <room>   # after creating a user via curl, see below

# voice SFU
cargo run --bin voice_node                    # UDP SFU on :5000

# audio pipeline (needs the `audio` feature — see below)
cargo run --bin mic_test --features audio     # live input amplitude meter
cargo run --bin voice_client --features audio # plays a 440Hz test tone
```

Create a user and two devices via REST:

```bash
curl -s -X POST localhost:3030/users -d '{"username":"alice"}'
curl -s -X POST localhost:3030/users/<user_id>/devices -d '{"as_audio_source": true}'
curl -s -X PUT  localhost:3030/users/<user_id>/audio-device/<other_device_id>  # switch
```

### The `audio` feature

`mic_test` and `voice_client` need `cpal` (ALSA on Linux) and `audiopus` (libopus). Everything else builds with zero native dependencies. On Debian/Ubuntu: `apt install libopus-dev libasound2-dev pkg-config`; on Arch: `pacman -S opus alsa-lib`. `cargo build` (no `--features audio`) never touches either.

## Binaries

| Binary | What it is |
| :--- | :--- |
| `tcp_echo`, `udp_ping_pong` | Bare async networking primitives — no ECS, just Tokio fundamentals. |
| `gateway` | Axum REST + WebSocket server: User/Device entities, `/ws` text chat, the device-switch endpoint. |
| `voice_node` | UDP SFU: auto-registers peers, forwards raw Opus packets by room + capability, takes a `switch <ssrc> <addr>` control line on stdin. |
| `mic_test` | Live input-amplitude meter (`--features audio`). |
| `voice_client` | Plays a 440Hz test tone; tags itself `AudioSource` locally (`--features audio`). |
| `term_client` | Terminal chat client over WebSockets. |

## Testing

```bash
cargo test                        # unit + integration tests
cargo test --features audio       # + Opus round-trip test
cargo clippy --all-targets        # clean, both feature sets
```


## License

No license file yet — all rights reserved by default until one is added.
