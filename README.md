# Universal Live Mixer

MVP scaffold for a Linux/PipeWire live software mixer aimed at small USB audio
interfaces such as the Focusrite Scarlett 4i4.

## Status

- This is an early experimental prototype.
- Do not rely on it for critical live sound work.
- Use at your own risk.
- Tested primarily on Linux with PipeWire/JACK and Focusrite Scarlett 4i4.

## MVP Scope

- 4 mono input channels
- stereo main output
- web control surface
- mute
- fader
- pan
- metering API
- JSON scene save/load

Out of scope for the MVP:

- EQ
- compressor
- gate
- AUX buses
- reverb/delay
- LV2 hosting
- MIDI and Stream Deck

## Architecture

See [docs/architecture.md](docs/architecture.md).

## API

See [docs/api.md](docs/api.md).

## Backend

```sh
pw-jack cargo run -p universal-live-mixer-backend
```

The backend listens on:

```text
http://127.0.0.1:3798
```

The default runtime audio backend is `JackAudioEngine`. It creates a JACK client
named `universal-live-mixer` and registers:

- `input_1`
- `input_2`
- `input_3`
- `input_4`
- `main_l`
- `main_r`

Run it through `pw-jack` so the JACK API talks to PipeWire:

```sh
pw-jack cargo run -p universal-live-mixer-backend
```

To run the control API without audio ports:

```sh
ULM_AUDIO_BACKEND=null cargo run -p universal-live-mixer-backend
```

To verify that the JACK ports exist:

```sh
pw-jack jack_lsp | grep universal-live-mixer
```

This version does not auto-connect to the Scarlett. Connect ports manually in
`qpwgraph`:

- Scarlett capture/playback source ports -> `universal-live-mixer:input_1..4`
- `universal-live-mixer:main_l` -> Scarlett playback left
- `universal-live-mixer:main_r` -> Scarlett playback right

## Frontend

```sh
cd frontend
npm install
npm run dev
```

The frontend listens on:

```text
http://127.0.0.1:5173
```

## Implementation Plan

1. Keep the control API and state model stable.
2. Keep `JackAudioEngine` simple and stable: 4 mono inputs to main L/R.
3. Add explicit device/profile detection for Scarlett 4i4.
4. Add optional auto-connect once manual routing is reliable.
5. Replace HTTP meter polling with WebSocket meter streaming.
6. Add tests around audio parameter conversion where practical outside JACK.
