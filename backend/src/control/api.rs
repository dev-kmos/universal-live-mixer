use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::{
    mixer::state::{ChannelPatch, MasterPatch, MeterState, MixerState},
    persistence::scenes::{
        self, CreateSceneRequest, MoveSceneRequest, RenameSceneRequest, SceneList, SceneLoadResult,
    },
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

pub async fn patch_master(
    State(state): State<AppState>,
    Json(patch): Json<MasterPatch>,
) -> Result<Json<MixerState>, StatusCode> {
    let (updated_mixer, audio_params) = {
        let mut mixer = state.mixer.write().await;
        mixer.master.apply_patch(patch);
        (mixer.clone(), mixer.master.audio_params())
    };

    state
        .audio
        .read()
        .await
        .update_master(audio_params)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(updated_mixer))
}

pub async fn get_meters(State(state): State<AppState>) -> Json<MeterState> {
    Json(state.audio.read().await.meters())
}

pub async fn list_scenes() -> Result<Json<SceneList>, StatusCode> {
    scenes::list()
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn create_scene(
    State(state): State<AppState>,
    Json(request): Json<CreateSceneRequest>,
) -> Result<Json<SceneList>, StatusCode> {
    let mixer = state.mixer.read().await.clone();
    scenes::create(request.name, &mixer)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn save_scene(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
) -> Result<Json<SceneList>, StatusCode> {
    let mixer = state.mixer.read().await.clone();
    scenes::save_scene(&scene_id, &mixer)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn load_scene(
    State(state): State<AppState>,
    Path(scene_id): Path<String>,
) -> Result<Json<SceneLoadResult>, StatusCode> {
    let result = scenes::load_scene(&scene_id).map_err(|_| StatusCode::NOT_FOUND)?;
    sync_loaded_mixer(&state, &result.mixer).await?;
    Ok(Json(result))
}

pub async fn rename_scene(
    Path(scene_id): Path<String>,
    Json(request): Json<RenameSceneRequest>,
) -> Result<Json<SceneList>, StatusCode> {
    scenes::rename_scene(&scene_id, request.name)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn move_scene(
    Path(scene_id): Path<String>,
    Json(request): Json<MoveSceneRequest>,
) -> Result<Json<SceneList>, StatusCode> {
    scenes::move_scene(&scene_id, request.direction)
        .map(Json)
        .map_err(|_| StatusCode::NOT_FOUND)
}

pub async fn load_next_scene(
    State(state): State<AppState>,
) -> Result<Json<SceneLoadResult>, StatusCode> {
    let result = scenes::load_next().map_err(|_| StatusCode::NOT_FOUND)?;
    sync_loaded_mixer(&state, &result.mixer).await?;
    Ok(Json(result))
}

pub async fn load_previous_scene(
    State(state): State<AppState>,
) -> Result<Json<SceneLoadResult>, StatusCode> {
    let result = scenes::load_previous().map_err(|_| StatusCode::NOT_FOUND)?;
    sync_loaded_mixer(&state, &result.mixer).await?;
    Ok(Json(result))
}

pub async fn reload_current_scene(
    State(state): State<AppState>,
) -> Result<Json<SceneLoadResult>, StatusCode> {
    let result = scenes::reload_current().map_err(|_| StatusCode::NOT_FOUND)?;
    sync_loaded_mixer(&state, &result.mixer).await?;
    Ok(Json(result))
}

async fn sync_loaded_mixer(state: &AppState, mixer: &MixerState) -> Result<(), StatusCode> {
    let channel_params = mixer
        .channels
        .iter()
        .map(|channel| channel.audio_params())
        .collect::<Vec<_>>();
    let master_params = mixer.master.audio_params();

    *state.mixer.write().await = mixer.clone();

    let audio = state.audio.read().await;
    for params in channel_params {
        audio
            .update_channel(params)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    audio
        .update_master(master_params)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(())
}
