use std::{fs, io, path::Path};

#[cfg(test)]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::mixer::state::MixerState;

const SCENE_DIR: &str = "scenes";
const INDEX_FILE: &str = "index.json";
const DEFAULT_SCENE_FILE: &str = "default.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneManifest {
    pub version: u32,
    pub current_scene_id: Option<String>,
    pub scenes: Vec<SceneEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEntry {
    pub id: String,
    pub name: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneList {
    pub version: u32,
    pub current_scene_id: Option<String>,
    pub scenes: Vec<SceneEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLoadResult {
    pub scenes: SceneList,
    pub mixer: MixerState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSceneRequest {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameSceneRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveSceneRequest {
    pub direction: MoveDirection,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveDirection {
    Up,
    Down,
}

impl From<SceneManifest> for SceneList {
    fn from(manifest: SceneManifest) -> Self {
        Self {
            version: manifest.version,
            current_scene_id: manifest.current_scene_id,
            scenes: manifest.scenes,
        }
    }
}

pub fn list() -> io::Result<SceneList> {
    list_from_dir(SCENE_DIR)
}

pub fn create(name: Option<String>, mixer: &MixerState) -> io::Result<SceneList> {
    create_in_dir(SCENE_DIR, name, mixer)
}

pub fn save_scene(scene_id: &str, mixer: &MixerState) -> io::Result<SceneList> {
    save_scene_in_dir(SCENE_DIR, scene_id, mixer)
}

pub fn load_scene(scene_id: &str) -> io::Result<SceneLoadResult> {
    load_scene_from_dir(SCENE_DIR, scene_id)
}

pub fn rename_scene(scene_id: &str, name: String) -> io::Result<SceneList> {
    rename_scene_in_dir(SCENE_DIR, scene_id, name)
}

pub fn move_scene(scene_id: &str, direction: MoveDirection) -> io::Result<SceneList> {
    move_scene_in_dir(SCENE_DIR, scene_id, direction)
}

pub fn load_next() -> io::Result<SceneLoadResult> {
    load_relative_scene_from_dir(SCENE_DIR, 1)
}

pub fn load_previous() -> io::Result<SceneLoadResult> {
    load_relative_scene_from_dir(SCENE_DIR, -1)
}

pub fn reload_current() -> io::Result<SceneLoadResult> {
    reload_current_from_dir(SCENE_DIR)
}

pub fn list_from_dir(scene_dir: impl AsRef<Path>) -> io::Result<SceneList> {
    ensure_manifest(scene_dir).map(SceneList::from)
}

pub fn create_in_dir(
    scene_dir: impl AsRef<Path>,
    name: Option<String>,
    mixer: &MixerState,
) -> io::Result<SceneList> {
    let scene_dir = scene_dir.as_ref();
    let mut manifest = ensure_manifest(scene_dir)?;
    let id = next_scene_id(&manifest);
    let file = format!("{id}.json");
    let name =
        clean_scene_name(name).unwrap_or_else(|| format!("Scene {}", manifest.scenes.len() + 1));

    save_scene_file(scene_dir, &file, mixer)?;
    manifest.current_scene_id = Some(id.clone());
    manifest.scenes.push(SceneEntry { id, name, file });
    save_manifest(scene_dir, &manifest)?;
    Ok(manifest.into())
}

pub fn save_scene_in_dir(
    scene_dir: impl AsRef<Path>,
    scene_id: &str,
    mixer: &MixerState,
) -> io::Result<SceneList> {
    let scene_dir = scene_dir.as_ref();
    let mut manifest = ensure_manifest(scene_dir)?;
    let scene = find_scene(&manifest, scene_id)?.clone();

    save_scene_file(scene_dir, &scene.file, mixer)?;
    manifest.current_scene_id = Some(scene.id);
    save_manifest(scene_dir, &manifest)?;
    Ok(manifest.into())
}

pub fn load_scene_from_dir(
    scene_dir: impl AsRef<Path>,
    scene_id: &str,
) -> io::Result<SceneLoadResult> {
    let scene_dir = scene_dir.as_ref();
    let mut manifest = ensure_manifest(scene_dir)?;
    let scene = find_scene(&manifest, scene_id)?.clone();
    let mixer = load_scene_file(scene_dir, &scene.file)?;

    manifest.current_scene_id = Some(scene.id);
    save_manifest(scene_dir, &manifest)?;
    Ok(SceneLoadResult {
        scenes: manifest.into(),
        mixer,
    })
}

pub fn rename_scene_in_dir(
    scene_dir: impl AsRef<Path>,
    scene_id: &str,
    name: String,
) -> io::Result<SceneList> {
    let scene_dir = scene_dir.as_ref();
    let mut manifest = ensure_manifest(scene_dir)?;
    let scene = find_scene_mut(&mut manifest, scene_id)?;
    scene.name = clean_scene_name(Some(name)).unwrap_or_else(|| scene.id.clone());
    save_manifest(scene_dir, &manifest)?;
    Ok(manifest.into())
}

pub fn move_scene_in_dir(
    scene_dir: impl AsRef<Path>,
    scene_id: &str,
    direction: MoveDirection,
) -> io::Result<SceneList> {
    let scene_dir = scene_dir.as_ref();
    let mut manifest = ensure_manifest(scene_dir)?;
    let index = manifest
        .scenes
        .iter()
        .position(|scene| scene.id == scene_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "scene not found"))?;

    match direction {
        MoveDirection::Up if index > 0 => manifest.scenes.swap(index, index - 1),
        MoveDirection::Down if index + 1 < manifest.scenes.len() => {
            manifest.scenes.swap(index, index + 1)
        }
        _ => {}
    }

    save_manifest(scene_dir, &manifest)?;
    Ok(manifest.into())
}

pub fn load_relative_scene_from_dir(
    scene_dir: impl AsRef<Path>,
    offset: isize,
) -> io::Result<SceneLoadResult> {
    let scene_dir = scene_dir.as_ref();
    let manifest = ensure_manifest(scene_dir)?;
    if manifest.scenes.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "no scenes"));
    }

    let current_index = manifest
        .current_scene_id
        .as_ref()
        .and_then(|id| manifest.scenes.iter().position(|scene| &scene.id == id))
        .unwrap_or(0);
    let len = manifest.scenes.len() as isize;
    let next_index = (current_index as isize + offset).rem_euclid(len) as usize;
    let scene_id = manifest.scenes[next_index].id.clone();
    load_scene_from_dir(scene_dir, &scene_id)
}

pub fn reload_current_from_dir(scene_dir: impl AsRef<Path>) -> io::Result<SceneLoadResult> {
    let scene_dir = scene_dir.as_ref();
    let manifest = ensure_manifest(scene_dir)?;
    let scene_id = manifest
        .current_scene_id
        .clone()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no current scene"))?;
    load_scene_from_dir(scene_dir, &scene_id)
}

#[cfg(test)]
pub fn save_to_dir(
    scene_dir: impl AsRef<Path>,
    scene_name: &str,
    mixer: &MixerState,
) -> io::Result<()> {
    fs::create_dir_all(scene_dir.as_ref())?;
    let body = serde_json::to_string_pretty(mixer).map_err(io::Error::other)?;
    fs::write(legacy_scene_path(scene_dir, scene_name), body)
}

#[cfg(test)]
pub fn load_from_dir(scene_dir: impl AsRef<Path>, scene_name: &str) -> io::Result<MixerState> {
    let body = fs::read_to_string(legacy_scene_path(scene_dir, scene_name))?;
    serde_json::from_str(&body).map_err(io::Error::other)
}

fn ensure_manifest(scene_dir: impl AsRef<Path>) -> io::Result<SceneManifest> {
    let scene_dir = scene_dir.as_ref();
    fs::create_dir_all(scene_dir)?;
    let index_path = scene_dir.join(INDEX_FILE);

    if index_path.exists() {
        let body = fs::read_to_string(index_path)?;
        return serde_json::from_str(&body).map_err(io::Error::other);
    }

    let default_path = scene_dir.join(DEFAULT_SCENE_FILE);
    let scenes = if default_path.exists() {
        vec![SceneEntry {
            id: "scene-1".into(),
            name: "Default".into(),
            file: DEFAULT_SCENE_FILE.into(),
        }]
    } else {
        Vec::new()
    };
    let current_scene_id = scenes.first().map(|scene| scene.id.clone());
    let manifest = SceneManifest {
        version: 1,
        current_scene_id,
        scenes,
    };
    save_manifest(scene_dir, &manifest)?;
    Ok(manifest)
}

fn save_manifest(scene_dir: &Path, manifest: &SceneManifest) -> io::Result<()> {
    let body = serde_json::to_string_pretty(manifest).map_err(io::Error::other)?;
    fs::write(scene_dir.join(INDEX_FILE), body)
}

fn save_scene_file(scene_dir: &Path, file: &str, mixer: &MixerState) -> io::Result<()> {
    let body = serde_json::to_string_pretty(mixer).map_err(io::Error::other)?;
    fs::write(scene_dir.join(safe_file_name(file)), body)
}

fn load_scene_file(scene_dir: &Path, file: &str) -> io::Result<MixerState> {
    let body = fs::read_to_string(scene_dir.join(safe_file_name(file)))?;
    serde_json::from_str(&body).map_err(io::Error::other)
}

fn find_scene<'a>(manifest: &'a SceneManifest, scene_id: &str) -> io::Result<&'a SceneEntry> {
    manifest
        .scenes
        .iter()
        .find(|scene| scene.id == scene_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "scene not found"))
}

fn find_scene_mut<'a>(
    manifest: &'a mut SceneManifest,
    scene_id: &str,
) -> io::Result<&'a mut SceneEntry> {
    manifest
        .scenes
        .iter_mut()
        .find(|scene| scene.id == scene_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "scene not found"))
}

fn next_scene_id(manifest: &SceneManifest) -> String {
    let mut index = manifest.scenes.len() + 1;
    loop {
        let id = format!("scene-{index}");
        if !manifest.scenes.iter().any(|scene| scene.id == id) {
            return id;
        }
        index += 1;
    }
}

fn clean_scene_name(name: Option<String>) -> Option<String> {
    name.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
fn legacy_scene_path(scene_dir: impl AsRef<Path>, scene_name: &str) -> PathBuf {
    scene_dir
        .as_ref()
        .join(format!("{}.json", safe_scene_stem(scene_name)))
}

fn safe_file_name(file: &str) -> String {
    file.chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_' || *ch == '.')
        .collect()
}

#[cfg(test)]
fn safe_scene_stem(scene_name: &str) -> String {
    scene_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{mixer::state::MixerState, persistence::scenes};

    #[test]
    fn saves_and_loads_legacy_scene_json() {
        let scene_dir = temp_scene_dir("legacy");
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

        cleanup(scene_dir);
    }

    #[test]
    fn creates_manifest_from_existing_default_json() {
        let scene_dir = temp_scene_dir("default");
        let mixer = MixerState::focusrite_4i4_default();
        scenes::save_to_dir(&scene_dir, "default", &mixer).expect("default scene save failed");

        let list = scenes::list_from_dir(&scene_dir).expect("scene list failed");

        assert_eq!(list.current_scene_id.as_deref(), Some("scene-1"));
        assert_eq!(list.scenes.len(), 1);
        assert_eq!(list.scenes[0].name, "Default");
        assert_eq!(list.scenes[0].file, "default.json");

        cleanup(scene_dir);
    }

    #[test]
    fn creates_saves_loads_renames_moves_and_steps_scenes() {
        let scene_dir = temp_scene_dir("manager");
        let mut mixer = MixerState::focusrite_4i4_default();

        let list = scenes::create_in_dir(&scene_dir, Some("Intro".into()), &mixer)
            .expect("scene create failed");
        assert_eq!(list.current_scene_id.as_deref(), Some("scene-1"));
        assert_eq!(list.scenes[0].name, "Intro");

        mixer.channels[0].mute = true;
        mixer.master.fader_db = -6.0;
        let list = scenes::create_in_dir(&scene_dir, Some("Finale".into()), &mixer)
            .expect("second scene create failed");
        assert_eq!(list.current_scene_id.as_deref(), Some("scene-2"));

        mixer.channels[0].mute = false;
        mixer.channels[0].fader_db = -18.0;
        scenes::save_scene_in_dir(&scene_dir, "scene-1", &mixer).expect("scene save failed");
        let loaded = scenes::load_scene_from_dir(&scene_dir, "scene-1").expect("scene load failed");
        assert_eq!(loaded.scenes.current_scene_id.as_deref(), Some("scene-1"));
        assert_eq!(loaded.mixer.channels[0].fader_db, -18.0);

        let list = scenes::rename_scene_in_dir(&scene_dir, "scene-1", "Vocal".into())
            .expect("rename failed");
        assert_eq!(list.scenes[0].name, "Vocal");

        let list = scenes::move_scene_in_dir(&scene_dir, "scene-2", scenes::MoveDirection::Up)
            .expect("move up failed");
        assert_eq!(list.scenes[0].id, "scene-2");

        let loaded =
            scenes::load_scene_from_dir(&scene_dir, "scene-2").expect("scene-2 load failed");
        assert_eq!(loaded.mixer.channels[0].mute, true);
        assert_eq!(loaded.mixer.master.fader_db, -6.0);

        let previous =
            scenes::load_relative_scene_from_dir(&scene_dir, -1).expect("previous failed");
        assert_eq!(previous.scenes.current_scene_id.as_deref(), Some("scene-1"));

        let next = scenes::load_relative_scene_from_dir(&scene_dir, 1).expect("next failed");
        assert_eq!(next.scenes.current_scene_id.as_deref(), Some("scene-2"));

        let mut changed_mixer = MixerState::focusrite_4i4_default();
        changed_mixer.channels[0].fader_db = -30.0;
        scenes::save_scene_in_dir(&scene_dir, "scene-2", &changed_mixer)
            .expect("scene-2 overwrite failed");
        let reloaded = scenes::reload_current_from_dir(&scene_dir).expect("reload current failed");
        assert_eq!(reloaded.scenes.current_scene_id.as_deref(), Some("scene-2"));
        assert_eq!(reloaded.mixer.channels[0].fader_db, -30.0);

        cleanup(scene_dir);
    }

    fn temp_scene_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ulm-scene-test-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ))
    }

    fn cleanup(scene_dir: PathBuf) {
        fs::remove_dir_all(scene_dir).expect("failed to remove temp scene dir");
    }
}
