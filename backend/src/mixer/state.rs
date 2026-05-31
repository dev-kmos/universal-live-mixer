use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerState {
    pub device_profile: String,
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub channels: Vec<ChannelState>,
    pub master: MasterState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelState {
    pub id: usize,
    pub name: String,
    pub mute: bool,
    pub fader_db: f32,
    pub pan: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterState {
    pub mute: bool,
    pub fader_db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterState {
    pub channels_peak_db: Vec<f32>,
    pub master_peak_db: [f32; 2],
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChannelPatch {
    pub name: Option<String>,
    pub mute: Option<bool>,
    pub fader_db: Option<f32>,
    pub pan: Option<f32>,
}

impl MixerState {
    pub fn focusrite_4i4_default() -> Self {
        Self {
            device_profile: "focusrite-scarlett-4i4".into(),
            sample_rate: 48_000,
            buffer_size: 128,
            channels: (0..4)
                .map(|index| ChannelState {
                    id: index,
                    name: format!("CH {}", index + 1),
                    mute: false,
                    fader_db: 0.0,
                    pan: 0.0,
                })
                .collect(),
            master: MasterState {
                mute: false,
                fader_db: 0.0,
            },
        }
    }
}

impl ChannelState {
    pub fn apply_patch(&mut self, patch: ChannelPatch) {
        if let Some(name) = patch.name {
            self.name = name;
        }

        if let Some(mute) = patch.mute {
            self.mute = mute;
        }

        if let Some(fader_db) = patch.fader_db {
            self.fader_db = fader_db.clamp(-60.0, 10.0);
        }

        if let Some(pan) = patch.pan {
            self.pan = pan.clamp(-1.0, 1.0);
        }
    }

    pub fn audio_params(&self) -> crate::audio::ChannelAudioParams {
        crate::audio::ChannelAudioParams {
            channel_id: self.id,
            mute: self.mute,
            fader_db: self.fader_db,
            pan: self.pan,
        }
    }
}

impl MeterState {
    pub fn silent(channel_count: usize) -> Self {
        Self {
            channels_peak_db: vec![-90.0; channel_count],
            master_peak_db: [-90.0, -90.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ChannelPatch, ChannelState};

    #[test]
    fn channel_patch_updates_name_mute_and_clamps_parameters() {
        let mut channel = ChannelState {
            id: 0,
            name: "CH 1".into(),
            mute: false,
            fader_db: 0.0,
            pan: 0.0,
        };

        channel.apply_patch(ChannelPatch {
            name: Some("Vocal".into()),
            mute: Some(true),
            fader_db: Some(24.0),
            pan: Some(-2.0),
        });

        assert_eq!(channel.name, "Vocal");
        assert!(channel.mute);
        assert_eq!(channel.fader_db, 10.0);
        assert_eq!(channel.pan, -1.0);

        channel.apply_patch(ChannelPatch {
            name: None,
            mute: None,
            fader_db: Some(-90.0),
            pan: Some(2.0),
        });

        assert_eq!(channel.name, "Vocal");
        assert!(channel.mute);
        assert_eq!(channel.fader_db, -60.0);
        assert_eq!(channel.pan, 1.0);
    }
}
