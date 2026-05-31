use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::mixer::state::MixerState;

const SCENE_DIR: &str = "scenes";

pub fn save(scene_name: &str, mixer: &MixerState) -> io::Result<()> {
    save_to_dir(SCENE_DIR, scene_name, mixer)
}

pub fn load(scene_name: &str) -> io::Result<MixerState> {
    load_from_dir(SCENE_DIR, scene_name)
}

pub fn save_to_dir(
    scene_dir: impl AsRef<Path>,
    scene_name: &str,
    mixer: &MixerState,
) -> io::Result<()> {
    fs::create_dir_all(scene_dir.as_ref())?;
    let body = serde_json::to_string_pretty(mixer).map_err(io::Error::other)?;
    fs::write(scene_path(scene_dir, scene_name), body)
}

pub fn load_from_dir(scene_dir: impl AsRef<Path>, scene_name: &str) -> io::Result<MixerState> {
    let body = fs::read_to_string(scene_path(scene_dir, scene_name))?;
    serde_json::from_str(&body).map_err(io::Error::other)
}

fn scene_path(scene_dir: impl AsRef<Path>, scene_name: &str) -> PathBuf {
    let safe_name = scene_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>();

    scene_dir.as_ref().join(format!("{safe_name}.json"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{mixer::state::MixerState, persistence::scenes};

    #[test]
    fn saves_and_loads_scene_json() {
        let scene_dir = std::env::temp_dir().join(format!(
            "ulm-scene-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));

        let mut mixer = MixerState::focusrite_4i4_default();
        mixer.channels[0].name = "Lead Vocal".into();
        mixer.channels[0].mute = true;
        mixer.channels[0].fader_db = -12.5;
        mixer.channels[0].pan = -0.25;

        scenes::save_to_dir(&scene_dir, "test-scene", &mixer).expect("scene save failed");
        let loaded = scenes::load_from_dir(&scene_dir, "test-scene").expect("scene load failed");

        assert_eq!(loaded.channels[0].name, "Lead Vocal");
        assert!(loaded.channels[0].mute);
        assert_eq!(loaded.channels[0].fader_db, -12.5);
        assert_eq!(loaded.channels[0].pan, -0.25);
        assert_eq!(loaded.channels.len(), 4);

        fs::remove_dir_all(scene_dir).expect("failed to remove temp scene dir");
    }
}
