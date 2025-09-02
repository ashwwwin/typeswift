pub mod config;
pub mod error;
pub mod platform;
pub mod services;
pub mod controller;
pub mod state;
pub mod window;
pub mod output;

// Backward-compat shim: some modules may still refer to `crate::audio`.
// Keep a thin module to avoid wide churn until all call sites are migrated.
#[allow(dead_code)]
pub mod audio {
    pub use crate::services::audio::{ImprovedAudioProcessor, Transcriber, AudioCapture};
}

pub mod input;
