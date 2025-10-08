// Hard gate: this crate only supports macOS
#[cfg(not(target_os = "macos"))]
compile_error!("This crate supports only macOS (target_os = \"macos\").");

pub mod config;
pub mod error;
pub mod platform;
pub mod services;
pub mod controller;
pub mod state;
pub mod window;
pub mod output;
pub mod mem;

// Backward-compat shim: some modules may still refer to `crate::audio`.
// Keep a thin module to avoid wide churn until all call sites are migrated.
#[allow(dead_code)]
pub mod audio {
    pub use crate::services::audio::{ImprovedAudioProcessor, Transcriber, AudioCapture};
}

pub mod input;
