use std::{
    array,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
};

use jack::{
    AsyncClient, AudioIn, AudioOut, Client, ClientOptions, Control, NotificationHandler, Port,
    ProcessHandler, ProcessScope,
};

use crate::{
    audio::{AudioEngine, AudioError, ChannelAudioParams, SharedMixerState},
    mixer::state::{MeterState, MixerState},
};

const INPUT_CHANNELS: usize = 4;
const SILENCE_DB: f32 = -90.0;

type ActiveJackClient = AsyncClient<JackNotifications, MixerProcess>;

pub struct JackAudioEngine {
    realtime: Arc<RealtimeState>,
    active_client: Option<ActiveJackClient>,
}

impl JackAudioEngine {
    pub fn new() -> Self {
        Self {
            realtime: Arc::new(RealtimeState::new()),
            active_client: None,
        }
    }

    fn publish_initial_state(&self, mixer: &MixerState) {
        for channel in &mixer.channels {
            let params = channel.audio_params();
            self.realtime.update_channel(params);
        }
    }
}

impl AudioEngine for JackAudioEngine {
    fn start(&mut self, mixer: SharedMixerState) -> Result<(), AudioError> {
        if self.active_client.is_some() {
            return Ok(());
        }

        let mixer_snapshot = mixer
            .try_read()
            .map_err(|_| AudioError::new("failed to read initial mixer state"))?
            .clone();
        self.publish_initial_state(&mixer_snapshot);

        let (client, _status) = Client::new("universal-live-mixer", ClientOptions::NO_START_SERVER)
            .map_err(|err| AudioError::new(format!("failed to create JACK client: {err}")))?;

        let input_1 = client
            .register_port("input_1", AudioIn::default())
            .map_err(|err| AudioError::new(format!("failed to register input_1: {err}")))?;
        let input_2 = client
            .register_port("input_2", AudioIn::default())
            .map_err(|err| AudioError::new(format!("failed to register input_2: {err}")))?;
        let input_3 = client
            .register_port("input_3", AudioIn::default())
            .map_err(|err| AudioError::new(format!("failed to register input_3: {err}")))?;
        let input_4 = client
            .register_port("input_4", AudioIn::default())
            .map_err(|err| AudioError::new(format!("failed to register input_4: {err}")))?;
        let main_l = client
            .register_port("main_l", AudioOut::default())
            .map_err(|err| AudioError::new(format!("failed to register main_l: {err}")))?;
        let main_r = client
            .register_port("main_r", AudioOut::default())
            .map_err(|err| AudioError::new(format!("failed to register main_r: {err}")))?;

        let process = MixerProcess {
            realtime: self.realtime.clone(),
            inputs: [input_1, input_2, input_3, input_4],
            main_l,
            main_r,
        };

        let active_client = client
            .activate_async(JackNotifications, process)
            .map_err(|err| AudioError::new(format!("failed to activate JACK client: {err}")))?;

        self.active_client = Some(active_client);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.deactivate()
    }

    fn meters(&self) -> MeterState {
        self.realtime.meters()
    }

    fn update_channel(&self, params: ChannelAudioParams) -> Result<(), AudioError> {
        if params.channel_id >= INPUT_CHANNELS {
            return Err(AudioError::new(format!(
                "channel id {} is outside the JACK input range",
                params.channel_id
            )));
        }

        self.realtime.update_channel(params);
        Ok(())
    }
}

impl JackAudioEngine {
    fn deactivate(&mut self) -> Result<(), AudioError> {
        if let Some(active_client) = self.active_client.take() {
            active_client.deactivate().map_err(|err| {
                AudioError::new(format!("failed to deactivate JACK client: {err}"))
            })?;
        }

        Ok(())
    }
}

impl Drop for JackAudioEngine {
    fn drop(&mut self) {
        let _ = self.deactivate();
    }
}

struct JackNotifications;

impl NotificationHandler for JackNotifications {}

struct MixerProcess {
    realtime: Arc<RealtimeState>,
    inputs: [Port<AudioIn>; INPUT_CHANNELS],
    main_l: Port<AudioOut>,
    main_r: Port<AudioOut>,
}

impl ProcessHandler for MixerProcess {
    fn process(&mut self, _client: &Client, scope: &ProcessScope) -> Control {
        let input_1 = self.inputs[0].as_slice(scope);
        let input_2 = self.inputs[1].as_slice(scope);
        let input_3 = self.inputs[2].as_slice(scope);
        let input_4 = self.inputs[3].as_slice(scope);
        let inputs = [input_1, input_2, input_3, input_4];

        let main_l = self.main_l.as_mut_slice(scope);
        let main_r = self.main_r.as_mut_slice(scope);

        let mut channel_peaks = [0.0_f32; INPUT_CHANNELS];
        let mut master_peak_l = 0.0_f32;
        let mut master_peak_r = 0.0_f32;

        for frame in 0..scope.n_frames() as usize {
            let mut mixed_l = 0.0_f32;
            let mut mixed_r = 0.0_f32;

            for channel_index in 0..INPUT_CHANNELS {
                let params = &self.realtime.channels[channel_index];
                if params.mute.load(Ordering::Relaxed) {
                    continue;
                }

                let gain_l = load_f32(&params.gain_l_bits);
                let gain_r = load_f32(&params.gain_r_bits);
                let dry_sample = inputs[channel_index][frame];
                let channel_l = dry_sample * gain_l;
                let channel_r = dry_sample * gain_r;
                let channel_peak = channel_l.abs().max(channel_r.abs());
                if channel_peak > channel_peaks[channel_index] {
                    channel_peaks[channel_index] = channel_peak;
                }

                mixed_l += channel_l;
                mixed_r += channel_r;
            }

            main_l[frame] = mixed_l;
            main_r[frame] = mixed_r;

            let abs_l = mixed_l.abs();
            let abs_r = mixed_r.abs();
            if abs_l > master_peak_l {
                master_peak_l = abs_l;
            }
            if abs_r > master_peak_r {
                master_peak_r = abs_r;
            }
        }

        for (index, peak) in channel_peaks.into_iter().enumerate() {
            store_f32(&self.realtime.channel_peak_bits[index], peak);
        }
        store_f32(&self.realtime.master_peak_l_bits, master_peak_l);
        store_f32(&self.realtime.master_peak_r_bits, master_peak_r);

        Control::Continue
    }
}

struct RealtimeState {
    channels: [RealtimeChannelParams; INPUT_CHANNELS],
    channel_peak_bits: [AtomicU32; INPUT_CHANNELS],
    master_peak_l_bits: AtomicU32,
    master_peak_r_bits: AtomicU32,
}

impl RealtimeState {
    fn new() -> Self {
        Self {
            channels: array::from_fn(|_| RealtimeChannelParams::default()),
            channel_peak_bits: array::from_fn(|_| AtomicU32::new(0.0_f32.to_bits())),
            master_peak_l_bits: AtomicU32::new(0.0_f32.to_bits()),
            master_peak_r_bits: AtomicU32::new(0.0_f32.to_bits()),
        }
    }

    fn update_channel(&self, params: ChannelAudioParams) {
        if let Some(channel) = self.channels.get(params.channel_id) {
            let gains = channel_gains(params.fader_db, params.pan);
            channel.mute.store(params.mute, Ordering::Relaxed);
            store_f32(&channel.gain_l_bits, gains.left);
            store_f32(&channel.gain_r_bits, gains.right);
        }
    }

    fn meters(&self) -> MeterState {
        let channels_peak_db = self
            .channel_peak_bits
            .iter()
            .map(|peak| linear_to_db(load_f32(peak)))
            .collect();

        MeterState {
            channels_peak_db,
            master_peak_db: [
                linear_to_db(load_f32(&self.master_peak_l_bits)),
                linear_to_db(load_f32(&self.master_peak_r_bits)),
            ],
        }
    }
}

struct RealtimeChannelParams {
    mute: AtomicBool,
    gain_l_bits: AtomicU32,
    gain_r_bits: AtomicU32,
}

impl Default for RealtimeChannelParams {
    fn default() -> Self {
        Self {
            mute: AtomicBool::new(false),
            gain_l_bits: AtomicU32::new(std::f32::consts::FRAC_1_SQRT_2.to_bits()),
            gain_r_bits: AtomicU32::new(std::f32::consts::FRAC_1_SQRT_2.to_bits()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ChannelGains {
    left: f32,
    right: f32,
}

fn channel_gains(fader_db: f32, pan: f32) -> ChannelGains {
    let linear = db_to_linear(fader_db.clamp(-60.0, 10.0));
    let pan = pan.clamp(-1.0, 1.0);

    // Equal-power pan keeps perceived level steadier through the center than a
    // linear pan. At center, each side is roughly -3 dB; hard left/right sends
    // all signal to only one side.
    let pan_angle = (pan + 1.0) * std::f32::consts::FRAC_PI_4;

    ChannelGains {
        left: linear * pan_angle.cos(),
        right: linear * pan_angle.sin(),
    }
}

fn db_to_linear(db: f32) -> f32 {
    if db <= -60.0 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.000_031_62 {
        SILENCE_DB
    } else {
        (20.0 * linear.log10()).max(SILENCE_DB)
    }
}

fn load_f32(value: &AtomicU32) -> f32 {
    f32::from_bits(value.load(Ordering::Relaxed))
}

fn store_f32(target: &AtomicU32, value: f32) {
    target.store(value.to_bits(), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::{channel_gains, db_to_linear};

    const EPSILON: f32 = 0.000_01;

    #[test]
    fn zero_db_center_pan_is_minus_three_db_per_side() {
        let gains = channel_gains(0.0, 0.0);

        assert_close(gains.left, std::f32::consts::FRAC_1_SQRT_2);
        assert_close(gains.right, std::f32::consts::FRAC_1_SQRT_2);
    }

    #[test]
    fn zero_db_hard_left_pan_routes_left_only() {
        let gains = channel_gains(0.0, -1.0);

        assert_close(gains.left, 1.0);
        assert_close(gains.right, 0.0);
    }

    #[test]
    fn zero_db_hard_right_pan_routes_right_only() {
        let gains = channel_gains(0.0, 1.0);

        assert_close(gains.left, 0.0);
        assert_close(gains.right, 1.0);
    }

    #[test]
    fn minus_sixty_db_is_silent() {
        let gains = channel_gains(-60.0, 0.0);

        assert_close(gains.left, 0.0);
        assert_close(gains.right, 0.0);
    }

    #[test]
    fn fader_above_ten_db_is_clamped() {
        let gains = channel_gains(24.0, -1.0);

        assert_close(gains.left, db_to_linear(10.0));
        assert_close(gains.right, 0.0);
    }

    #[test]
    fn pan_outside_range_is_clamped() {
        let left = channel_gains(0.0, -2.0);
        let right = channel_gains(0.0, 2.0);

        assert_close(left.left, 1.0);
        assert_close(left.right, 0.0);
        assert_close(right.left, 0.0);
        assert_close(right.right, 1.0);
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "actual {actual} != expected {expected}"
        );
    }
}
