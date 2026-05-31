use crate::{
    audio::{AudioEngine, AudioError, ChannelAudioParams, SharedMixerState},
    mixer::state::MeterState,
};

/// Non-realtime audio backend used while the control plane is being built.
///
/// It does not connect to PipeWire or JACK and does not process audio. Its role
/// is to keep the backend, API, scene handling and frontend buildable before the
/// real JACK engine is implemented.
pub struct NullAudioEngine {
    meters: MeterState,
    running: bool,
}

impl NullAudioEngine {
    pub fn new(channel_count: usize) -> Self {
        Self {
            meters: MeterState::silent(channel_count),
            running: false,
        }
    }
}

impl AudioEngine for NullAudioEngine {
    fn start(&mut self, _mixer: SharedMixerState) -> Result<(), AudioError> {
        self.running = true;
        tracing::warn!("using NullAudioEngine; no audio ports are opened and no audio is mixed");
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.running = false;
        Ok(())
    }

    fn meters(&self) -> MeterState {
        self.meters.clone()
    }

    fn update_channel(&self, _params: ChannelAudioParams) -> Result<(), AudioError> {
        Ok(())
    }
}
