# Universal Live Mixer MVP Architecture

## Scope

The MVP is a small live software mixer for Linux/PipeWire and a Focusrite
Scarlett 4i4-style interface:

- 4 mono input channels
- 1 stereo main output
- mute
- fader
- pan
- peak metering
- JSON scene save/load
- local web UI

EQ, compressors, gates, AUX buses and effects are explicitly out of scope for
this milestone.

## Directory Structure

```text
.
├── Cargo.toml
├── backend
│   ├── Cargo.toml
│   └── src
│       ├── audio
│       │   ├── mod.rs
│       │   ├── null.rs
│       │   └── jack.rs
│       ├── control
│       │   ├── api.rs
│       │   └── mod.rs
│       ├── mixer
│       │   ├── mod.rs
│       │   └── state.rs
│       ├── persistence
│       │   ├── mod.rs
│       │   └── scenes.rs
│       └── main.rs
├── frontend
│   ├── index.html
│   ├── package.json
│   ├── tsconfig.json
│   └── src
│       ├── main.ts
│       └── styles.css
├── docs
│   ├── api.md
│   └── architecture.md
└── scenes
    └── default.json
```

## Runtime Diagram

```text
┌──────────────────────────────────────────────────────────────┐
│                         Web Browser                           │
│  channel strips, faders, mute buttons, meters, scene save      │
└───────────────────────────────┬──────────────────────────────┘
                                │ HTTP JSON, later WebSocket
┌───────────────────────────────▼──────────────────────────────┐
│                        Backend Process                         │
│                                                               │
│  ┌─────────────────────┐        ┌──────────────────────────┐  │
│  │ Control API          │        │ Scene Persistence         │  │
│  │ axum HTTP routes     │◄──────►│ JSON files in scenes/     │  │
│  └──────────┬──────────┘        └──────────────────────────┘  │
│             │                                                  │
│  ┌──────────▼──────────┐                                       │
│  │ Mixer State Store    │                                       │
│  │ channels/master      │                                       │
│  └──────────┬──────────┘                                       │
│             │ control snapshots / realtime commands             │
│  ┌──────────▼──────────┐                                       │
│  │ Audio Engine         │                                       │
│  │ JackAudioEngine      │                                       │
│  │ or NullAudioEngine   │                                       │
│  └──────────┬──────────┘                                       │
└─────────────┼──────────────────────────────────────────────────┘
              │
┌─────────────▼──────────────────────────────────────────────────┐
│                         PipeWire Graph                          │
│ JACK ports exposed through pipewire-jack                        │
└────────────────────────────────────────────────────────────────┘
```

## Backend Responsibilities

- Own the mixer state.
- Run the audio engine.
- Expose control endpoints.
- Save and load scenes.
- Publish meter values.
- Keep realtime audio code isolated from HTTP and disk IO.

The `audio` module defaults to `JackAudioEngine`, which opens JACK ports through
`pipewire-jack`. `NullAudioEngine` remains available through
`ULM_AUDIO_BACKEND=null` for control-plane testing without audio ports.

## Frontend Responsibilities

- Render an X-Air Edit-like channel-strip view.
- Send small parameter patches to the backend.
- Poll meter values for the MVP.
- Trigger scene save/load.

The frontend does not own authoritative mixer state. It renders backend state.

## Minimal Libraries

Backend:

- Rust workspace
- `axum` for the local HTTP JSON API
- `tokio` for the async control plane
- `serde` and `serde_json` for state and scene serialization
- `tower-http` for CORS and HTTP tracing
- `tracing` for non-realtime diagnostics

Frontend:

- TypeScript
- Vite
- browser DOM APIs

The MVP intentionally avoids a heavy frontend framework and avoids LV2,
EQ/dynamics DSP libraries and device-control dependencies.

## Realtime Boundary

The audio callback must not:

- allocate memory
- parse JSON
- perform filesystem IO
- wait on mutexes
- log
- call HTTP/frontend code

Control changes should eventually cross into the audio thread through a bounded
lock-free queue or atomically swapped parameter block.
