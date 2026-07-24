use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentState {
    pub theme: ThemeChoice,
    pub open_files: Vec<PathBuf>,
    pub active_file: Option<PathBuf>,
    pub last_directory: Option<PathBuf>,
    pub display_user_strref: bool,
    pub display_hex_strref: bool,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::System,
            open_files: Vec::new(),
            active_file: None,
            last_directory: None,
            display_user_strref: false,
            display_hex_strref: false,
        }
    }
}

impl PersistentState {
    fn path() -> Option<PathBuf> {
        ProjectDirs::from("org", "Aurora Tools", "Aurora TLK Explorer")
            .map(|dirs| dirs.config_dir().join("session.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn store(&self) {
        let Some(path) = Self::path() else {
            return;
        };
        let Some(parent) = path.parent() else {
            return;
        };
        if fs::create_dir_all(parent).is_err() {
            return;
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = crate::formats::atomic_write(&path, &bytes);
        }
    }
}
