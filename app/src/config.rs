use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub audio: AudioConfig,
    pub model: ModelConfig,
    pub ui: UiConfig,
    pub output: OutputConfig,
    pub hotkeys: HotkeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub target_sample_rate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_name: String,
    pub left_context_seconds: usize,
    pub right_context_seconds: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub gap_from_bottom: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub enable_typing: bool,
    pub add_space_between_utterances: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct HotkeyConfig {
        pub toggle_window: Option<String>, // Optional separate toggle
        pub push_to_talk: String,          // Main push-to-talk hotkey
        pub preferences: Option<String>,   // Open preferences/settings
    }

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                target_sample_rate: 16000,
            },
            model: ModelConfig {
                model_name: "mlx-community/parakeet-tdt-0.6b-v3".to_string(),
                left_context_seconds: 5,
                right_context_seconds: 3,
            },
            ui: UiConfig {
                window_width: 90.0,
                window_height: 39.0,
                gap_from_bottom: 70.0,
            },
            output: OutputConfig {
                enable_typing: true,
                add_space_between_utterances: true,
            },
            hotkeys: HotkeyConfig {
                toggle_window: None, // Disabled by default
                push_to_talk: "fn".to_string(), // Use fn key on macOS (requires accessibility permissions)
                                                // Alternative: "cmd+space" or "opt+space"
                preferences: None,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        // Try Typeswift config first, then fallback to legacy Voicy path
        if let Ok(home) = std::env::var("HOME") {
            let typeswift_path = PathBuf::from(&home).join(".typeswift").join("config.toml");
            if typeswift_path.exists() {
                let contents = std::fs::read_to_string(typeswift_path)?;
                return Ok(toml::from_str(&contents)?);
            }
            let legacy_path = PathBuf::from(&home).join(".voicy").join("config.toml");
            if legacy_path.exists() {
                let contents = std::fs::read_to_string(legacy_path)?;
                return Ok(toml::from_str(&contents)?);
            }
        }
        // Return default if no config file
        Ok(Self::default())
    }

    pub fn save(&self, path: PathBuf) -> Result<()> {
        let toml_string = toml::to_string_pretty(self)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, toml_string)?;
        Ok(())
    }

    pub fn config_path() -> Option<PathBuf> {
        if let Ok(home) = std::env::var("HOME") {
            Some(PathBuf::from(home).join(".typeswift").join("config.toml"))
        } else {
            None
        }
    }
}
