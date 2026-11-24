use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock::TimeFormat;
use crate::theme::ThemeMode;

pub const MIN_WINDOW_WIDTH: f32 = 720.0;
pub const MIN_WINDOW_HEIGHT: f32 = 520.0;
pub const DEFAULT_WINDOW_WIDTH: f32 = 1180.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 720.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemeMode,
    pub time_format: TimeFormat,
    pub pin: bool,
    pub window_x: Option<f32>,
    pub window_y: Option<f32>,
    pub window_w: Option<f32>,
    pub window_h: Option<f32>,
}

impl Settings {
    pub fn load_from(path: &Path) -> Result<Self, String> {
        let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
        serde_json::from_str(&raw).map_err(|e| e.to_string())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, body).map_err(|e| e.to_string())?;
        if let Err(err) = fs::rename(&tmp_path, path) {
            let _ = fs::remove_file(&tmp_path);
            return Err(err.to_string());
        }
        Ok(())
    }

    pub fn load_default() -> (Self, Option<String>) {
        let Some(path) = default_settings_path() else {
            return (Self::default(), None);
        };
        if !path.exists() {
            return (Self::default(), None);
        }
        match Self::load_from(&path) {
            Ok(settings) => (settings, None),
            Err(_) => (
                Self::default(),
                Some("Could not load saved settings. Defaults are active.".to_string()),
            ),
        }
    }
}

pub fn default_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("block-clock").join("settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_and_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("settings.json");

        let original = Settings {
            theme: ThemeMode::Dark,
            time_format: TimeFormat::Hour12,
            pin: true,
            window_x: Some(120.0),
            window_y: Some(80.0),
            window_w: Some(1180.0),
            window_h: Some(720.0),
        };
        original.save_to(&path).expect("save settings");
        assert!(!path.with_extension("json.tmp").exists());
        let loaded = Settings::load_from(&path).expect("load settings");
        assert_eq!(original, loaded);
    }

    #[test]
    fn partial_json_and_legacy_auto_time_format_load_with_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{"time_format": "auto"}"#).expect("write partial json");

        let loaded = Settings::load_from(&path).expect("load partial settings");
        assert_eq!(loaded.time_format, TimeFormat::Hour24);
        assert_eq!(loaded.theme, ThemeMode::System);
        assert!(!loaded.pin);
        assert_eq!(loaded.window_x, None);
        assert_eq!(loaded.window_y, None);
        assert_eq!(loaded.window_w, None);
        assert_eq!(loaded.window_h, None);
    }

    #[test]
    fn missing_or_corrupt_file_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.json");
        assert!(Settings::load_from(&missing).is_err());

        let corrupt = dir.path().join("corrupt.json");
        fs::write(&corrupt, "{ not valid json").expect("write corrupt");
        assert!(Settings::load_from(&corrupt).is_err());
    }
}
