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

/// Log error and continue execution without panicking
pub fn log_error(error: &VoicyError) {
    eprintln!("❌ Error: {}", error);
}

/// Handle recoverable errors gracefully
pub fn handle_recoverable<T>(result: VoicyResult<T>, default: T) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            log_error(&e);
            default
        }
    }
}

impl From<pyo3::PyErr> for VoicyError {
    fn from(err: pyo3::PyErr) -> Self {
        VoicyError::ModelLoadFailed(format!("Python error: {}", err))
    }
}

impl From<anyhow::Error> for VoicyError {
    fn from(err: anyhow::Error) -> Self {
        VoicyError::ConfigLoadFailed(format!("Anyhow error: {}", err))
    }
}