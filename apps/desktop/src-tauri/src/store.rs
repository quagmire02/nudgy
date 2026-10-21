//! Settings and small UI state, as JSON files in the app config folder.
//! Nothing here is sensitive; activity and sessions live in the encrypted DB.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Window positions and toggles that aren't user settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UiState {
    /// Overlay window's top-left, physical pixels.
    pub overlay_position: Option<(i32, i32)>,
    /// The user hid the panda from the tray.
    pub panda_hidden: bool,
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Store { dir }
    }

    pub fn load<T: DeserializeOwned + Default>(&self, name: &str) -> T {
        let path = self.dir.join(name);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                log::warn!("{} is invalid, using defaults: {e}", path.display());
                T::default()
            }),
            Err(_) => T::default(),
        }
    }

    pub fn save<T: Serialize>(&self, name: &str, value: &T) {
        if let Err(e) = write_atomic(&self.dir.join(name), &serde_json::to_vec_pretty(value).unwrap()) {
            log::warn!("can't save {name}: {e}");
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}
