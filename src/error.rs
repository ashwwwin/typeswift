/// Centralized error handling for production-ready code
use std::fmt;

#[derive(Debug)]
pub enum VoicyError {
    AudioInitFailed(String),
    ModelLoadFailed(String),
    TranscriptionFailed(String),
    HotkeyRegistrationFailed(String),
    WindowOperationFailed(String),
    ConfigLoadFailed(String),
}

impl fmt::Display for VoicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VoicyError::AudioInitFailed(msg) => write!(f, "Audio initialization failed: {}", msg),
            VoicyError::ModelLoadFailed(msg) => write!(f, "Model load failed: {}", msg),
            VoicyError::TranscriptionFailed(msg) => write!(f, "Transcription failed: {}", msg),
            VoicyError::HotkeyRegistrationFailed(msg) => write!(f, "Hotkey registration failed: {}", msg),
            VoicyError::WindowOperationFailed(msg) => write!(f, "Window operation failed: {}", msg),
            VoicyError::ConfigLoadFailed(msg) => write!(f, "Config load failed: {}", msg),
        }
    }
}

impl std::error::Error for VoicyError {}

pub type VoicyResult<T> = Result<T, VoicyError>;

impl From<anyhow::Error> for VoicyError {
    fn from(err: anyhow::Error) -> Self {
        VoicyError::ConfigLoadFailed(format!("Anyhow error: {}", err))
    }
}