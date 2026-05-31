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

## Save Scene

```http
POST /scenes/{scene_name}
```

Saves the current mixer state as:

```text
scenes/{scene_name}.json
```

## Load Scene

```http
GET /scenes/{scene_name}
```

Loads a JSON scene and replaces the current mixer state.
