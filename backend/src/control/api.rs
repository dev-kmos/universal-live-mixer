use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::{
    mixer::state::{ChannelPatch, MeterState, MixerState},
    persistence::scenes,
    AppState,
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn get_mixer(State(state): State<AppState>) -> Json<MixerState> {
    Json(state.mixer.read().await.clone())
}

pub async fn patch_channel(
    State(state): State<AppState>,
    Path(channel_id): Path<usize>,
    Json(patch): Json<ChannelPatch>,
) -> Result<Json<MixerState>, StatusCode> {
    let (updated_mixer, audio_params) = {
        let mut mixer = state.mixer.write().await;
        let audio_params = {
            let channel = mixer
                .channels
                .iter_mut()
                .find(|channel| channel.id == channel_id)
                .ok_or(StatusCode::NOT_FOUND)?;

            channel.apply_patch(patch);
            channel.audio_params()
        };

        (mixer.clone(), audio_params)
    };

    state
        .audio
        .read()
        .await
        .update_channel(audio_params)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(updated_mixer))
}

pub async fn get_meters(State(state): State<AppState>) -> Json<MeterState> {
    Json(state.audio.read().await.meters())
}

pub async fn save_scene(
    State(state): State<AppState>,
    Path(scene_name): Path<String>,
) -> Result<Json<MixerState>, StatusCode> {
    let mixer = state.mixer.read().await.clone();
    scenes::save(&scene_name, &mixer).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(mixer))
}

pub async fn load_scene(
    State(state): State<AppState>,
    Path(scene_name): Path<String>,
) -> Result<Json<MixerState>, StatusCode> {
    let mixer = scenes::load(&scene_name).map_err(|_| StatusCode::NOT_FOUND)?;
    let channel_params = mixer
        .channels
        .iter()
        .map(|channel| channel.audio_params())
        .collect::<Vec<_>>();

    *state.mixer.write().await = mixer.clone();

    let audio = state.audio.read().await;
    for params in channel_params {
        audio
            .update_channel(params)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(mixer))
}
