# MVP Control API

Base URL:

```text
http://127.0.0.1:3798/api
```

## Health

```http
GET /health
```

Response:

```json
{
  "status": "ok"
}
```

## Get Mixer State

```http
GET /mixer
```

Response:

```json
{
  "device_profile": "focusrite-scarlett-4i4",
  "sample_rate": 48000,
  "buffer_size": 128,
  "channels": [
    {
      "id": 0,
      "name": "CH 1",
      "mute": false,
      "fader_db": 0.0,
      "pan": 0.0
    }
  ],
  "master": {
    "mute": false,
    "fader_db": 0.0
  }
}
```

## Patch Channel

```http
PATCH /channels/{channel_id}
Content-Type: application/json
```

Request:

```json
{
  "name": "Lead Vocal",
  "mute": true,
  "fader_db": -12.0,
  "pan": -0.25
}
```

All fields are optional.

Rules:

- `fader_db` is clamped to `-60.0..10.0`.
- `pan` is clamped to `-1.0..1.0`.
- `name` updates the channel label only.

Response: full mixer state.

## Patch Master

```http
PATCH /master
Content-Type: application/json
```

Request:

```json
{
  "fader_db": -6.0
}
```

Rules:

- `fader_db` is clamped to `-60.0..10.0`.

Response: full mixer state.

## Get Meters

```http
GET /meters
```

Response:

```json
{
  "channels_peak_db": [-30.0, -18.0, -90.0, -12.0],
  "master_peak_db": [-10.0, -11.0]
}
```

The MVP uses polling. Production should use WebSocket or a compact binary/event
stream to avoid wasteful HTTP request churn.

## List Scenes

```http
GET /scenes
```

Response:

```json
{
  "version": 1,
  "current_scene_id": "scene-1",
  "scenes": [
    {
      "id": "scene-1",
      "name": "Default",
      "file": "default.json"
    }
  ]
}
```

If `scenes/index.json` does not exist, the backend creates it. Existing
`scenes/default.json` is preserved and added as the first scene.

## Create Scene

```http
POST /scenes
Content-Type: application/json
```

Request:

```json
{
  "name": "Intro"
}
```

Creates a new scene from the current mixer state and makes it current.

## Save Scene

```http
POST /scenes/{scene_id}/save
```

Overwrites the scene with the current mixer state.

## Load Scene

```http
POST /scenes/{scene_id}/load
```

Loads the scene, replaces mixer state and synchronizes channel parameters and
master fader with the audio engine.

Response:

```json
{
  "scenes": {
    "version": 1,
    "current_scene_id": "scene-1",
    "scenes": []
  },
  "mixer": {}
}
```

## Rename Scene

```http
PATCH /scenes/{scene_id}
Content-Type: application/json
```

Request:

```json
{
  "name": "Vocal Check"
}
```

## Move Scene

```http
POST /scenes/{scene_id}/move
Content-Type: application/json
```

Request:

```json
{
  "direction": "up"
}
```

`direction` can be `up` or `down`.

## Previous / Next Scene

```http
POST /scenes/previous
POST /scenes/next
```

Loads the previous or next scene by manifest order, wrapping at the ends.

## Reload Current Scene

```http
POST /scenes/current/reload
```

Reloads the current scene from disk without changing scene order. This replaces
mixer state and synchronizes channel parameters and master fader with the audio
engine. If no current scene exists, the backend returns `404`.
