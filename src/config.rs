#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub audio: AudioConfig,
    pub vad: VadConfig,
    pub streaming: StreamingConfig,
    pub model: ModelConfig,
    pub ui: UiConfig,
    pub output: OutputConfig,
    pub hotkeys: HotkeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub target_sample_rate: u32,
    pub chunk_duration_ms: u32,
    pub buffer_size_seconds: u32,
    pub resampler_quality: ResamplerQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResamplerQuality {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    pub enabled: bool, // Enable VAD-based processing (vs continuous)
    pub speech_threshold: f32,
    pub silence_duration_ms: u32,
    pub min_speech_duration_ms: u32,
    pub enable_dc_offset_removal: bool,
    pub enable_normalization: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    pub enabled: bool,               // Enable continuous streaming mode
    pub rolling_buffer_seconds: f32, // Keep last N seconds of audio
    pub process_interval_ms: u32,    // Process every N milliseconds
    pub min_initial_audio_ms: u32,   // Wait for N ms before first inference
    pub lookahead_tokens: usize,     // Keep last N tokens tentative
    pub confidence_threshold: f32,   // Finalize tokens above this confidence
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_name: String,
    pub left_context_seconds: usize,
    pub right_context_seconds: usize,
    pub keep_loaded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub gap_from_bottom: f32,
    pub show_audio_levels: bool,
    pub auto_hide_on_stop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub enable_typing: bool,
    pub add_space_between_utterances: bool,
    pub console_logging: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub toggle_window: Option<String>, // Optional separate toggle
    pub push_to_talk: String,          // Main push-to-talk hotkey
    pub start_recording: Option<String>,
    pub stop_recording: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio: AudioConfig {
                target_sample_rate: 16000,
                chunk_duration_ms: 500,
                buffer_size_seconds: 10,
                resampler_quality: ResamplerQuality::High,
            },
            vad: VadConfig {
                enabled: false, // Disable VAD for streaming mode
                speech_threshold: 0.003,
                silence_duration_ms: 500,
                min_speech_duration_ms: 500,
                enable_dc_offset_removal: true,
                enable_normalization: true,
            },
            streaming: StreamingConfig {
                enabled: false, // true = type while speaking, false = type after release
                rolling_buffer_seconds: 10.0,
                process_interval_ms: 500, // Process every 500ms for calmer typing
                min_initial_audio_ms: 500, // Wait for initial audio chunk
                lookahead_tokens: 3,
                confidence_threshold: 0.85,
            },
            model: ModelConfig {
                model_name: "mlx-community/parakeet-tdt-0.6b-v2".to_string(),
                left_context_seconds: 5,
                right_context_seconds: 3,
                keep_loaded: true,
            },
            ui: UiConfig {
                window_width: 90.0,
                window_height: 39.0,
                gap_from_bottom: 70.0,
                show_audio_levels: false,
                auto_hide_on_stop: true, // Always hide after push-to-talk release
            },
            output: OutputConfig {
                enable_typing: true,
                add_space_between_utterances: true,
                console_logging: true,
            },
            hotkeys: HotkeyConfig {
                toggle_window: None, // Disabled by default, use push-to-talk instead
                push_to_talk: "Space".to_string(), // Hold to record
                start_recording: None,
                stop_recording: None,
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
