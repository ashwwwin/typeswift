use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub audio: AudioConfig,
    pub streaming: StreamingConfig,
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
pub struct StreamingConfig {
    pub enabled: bool,             // Enable continuous streaming mode
    pub process_interval_ms: u32,  // Process every N milliseconds
    pub min_initial_audio_ms: u32, // Wait for N ms before first inference
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                target_sample_rate: 16000,
            },
            streaming: StreamingConfig {
                enabled: false,            // true = type while speaking, false = type after release
                process_interval_ms: 250,  // Optimized for lower latency
                min_initial_audio_ms: 300, // Reduced for faster response
            },
            model: ModelConfig {
                model_name: "mlx-community/parakeet-tdt-0.6b-v2".to_string(),
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
                toggle_window: None,               // Disabled by default
                push_to_talk: "Space".to_string(), // Hold to record
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        // Try to load from config file
        if let Ok(home) = std::env::var("HOME") {
            let config_path = PathBuf::from(home).join(".voicy").join("config.toml");
            if config_path.exists() {
                let contents = std::fs::read_to_string(config_path)?;
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
}
