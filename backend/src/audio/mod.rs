pub mod jack;
mod null;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::mixer::state::{MeterState, MixerState};

pub use jack::JackAudioEngine;
pub use null::NullAudioEngine;

pub type SharedMixerState = Arc<RwLock<MixerState>>;

#[derive(Debug, Clone, Copy)]
pub struct ChannelAudioParams {
    pub channel_id: usize,
    pub mute: bool,
    pub fader_db: f32,
    pub pan: f32,
}

#[allow(dead_code)]
pub trait AudioEngine: Send + Sync {
    fn start(&mut self, mixer: SharedMixerState) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
    fn meters(&self) -> MeterState;
    fn update_channel(&self, params: ChannelAudioParams) -> Result<(), AudioError>;
}

#[derive(Debug)]
pub struct AudioError {
    message: String,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AudioError {}

impl AudioError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
