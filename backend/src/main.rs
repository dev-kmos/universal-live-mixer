mod audio;
mod control;
mod mixer;
mod persistence;

use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{get, patch, post},
    Router,
};
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    audio::{AudioEngine, JackAudioEngine, NullAudioEngine},
    control::api,
    mixer::state::MixerState,
};

#[derive(Clone)]
pub struct AppState {
    pub mixer: Arc<RwLock<MixerState>>,
    pub audio: Arc<RwLock<Box<dyn AudioEngine>>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mixer = Arc::new(RwLock::new(MixerState::focusrite_4i4_default()));
    let mut engine = create_audio_engine();
    engine
        .start(mixer.clone())
        .expect("failed to start audio engine");

    let state = AppState {
        mixer,
        audio: Arc::new(RwLock::new(engine)),
    };

    let app = Router::new()
        .route("/api/health", get(api::health))
        .route("/api/mixer", get(api::get_mixer))
        .route("/api/channels/{channel_id}", patch(api::patch_channel))
        .route("/api/master", patch(api::patch_master))
        .route("/api/meters", get(api::get_meters))
        .route("/api/scenes", get(api::list_scenes).post(api::create_scene))
        .route("/api/scenes/next", post(api::load_next_scene))
        .route("/api/scenes/previous", post(api::load_previous_scene))
        .route(
            "/api/scenes/current/reload",
            post(api::reload_current_scene),
        )
        .route("/api/scenes/{scene_id}/save", post(api::save_scene))
        .route("/api/scenes/{scene_id}/load", post(api::load_scene))
        .route("/api/scenes/{scene_id}", patch(api::rename_scene))
        .route("/api/scenes/{scene_id}/move", post(api::move_scene))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3798));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind HTTP listener");

    tracing::info!("control API listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server failed");
}

fn create_audio_engine() -> Box<dyn AudioEngine> {
    match std::env::var("ULM_AUDIO_BACKEND")
        .unwrap_or_else(|_| "jack".into())
        .as_str()
    {
        "null" => {
            tracing::info!("using NullAudioEngine");
            Box::new(NullAudioEngine::new(4))
        }
        "jack" => {
            tracing::info!("using JackAudioEngine");
            Box::new(JackAudioEngine::new())
        }
        unknown => {
            tracing::warn!("unknown ULM_AUDIO_BACKEND={unknown}; falling back to JackAudioEngine");
            Box::new(JackAudioEngine::new())
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
