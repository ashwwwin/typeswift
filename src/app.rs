use crate::config::Config;
use crate::error::{VoicyError, VoicyResult};
use anyhow::Result;
use parking_lot::RwLock;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamingState {
    Idle,
    Loading,
    Recording,
    Processing,
    Error,
}

pub struct VoicyApp {
    config: Config,
    state: Arc<RwLock<AppState>>,
    streaming_state: Arc<RwLock<StreamingState>>,
    transcription_text: Arc<RwLock<String>>,
    ui_update_needed: Arc<RwLock<bool>>,
}

impl VoicyApp {
    pub fn new() -> VoicyResult<Self> {
        let config = Config::load().map_err(|e| {
            VoicyError::ConfigLoadFailed(format!("Failed to load config: {}", e))
        })?;

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(AppState::Idle)),
            streaming_state: Arc::new(RwLock::new(StreamingState::Idle)),
            transcription_text: Arc::new(RwLock::new(String::new())),
            ui_update_needed: Arc::new(RwLock::new(false)),
        })
    }

    pub fn get_config(&self) -> &Config {
        &self.config
    }

    pub fn get_state(&self) -> AppState {
        *self.state.read()
    }

    pub fn set_state(&self, new_state: AppState) {
        let mut state = self.state.write();
        if *state != new_state {
            println!("🔄 App state change: {:?} → {:?}", *state, new_state);
            *state = new_state;
            self.request_ui_update();
        }
    }

    pub fn get_streaming_state(&self) -> StreamingState {
        *self.streaming_state.read()
    }

    pub fn set_streaming_state(&self, new_state: StreamingState) {
        let mut state = self.streaming_state.write();
        if *state != new_state {
            println!("🔄 Streaming state change: {:?} → {:?}", *state, new_state);
            *state = new_state;
            self.request_ui_update();
        }
    }

    pub fn get_transcription(&self) -> String {
        self.transcription_text.read().clone()
    }

    pub fn set_transcription(&self, text: String) {
        *self.transcription_text.write() = text;
        self.request_ui_update();
    }

    pub fn needs_ui_update(&self) -> bool {
        let needs_update = *self.ui_update_needed.read();
        if needs_update {
            *self.ui_update_needed.write() = false;
        }
        needs_update
    }

    pub fn request_ui_update(&self) {
        *self.ui_update_needed.write() = true;
    }

    pub fn start_recording(&self) -> VoicyResult<()> {
        if self.get_state() != AppState::Idle {
            println!("Cannot start recording: current state is {:?}", self.get_state());
            return Ok(());
        }

        println!("🚀 Starting recording session...");
        self.set_state(AppState::Recording);
        self.set_streaming_state(StreamingState::Loading);
        self.set_transcription(String::new());

        Ok(())
    }

    pub fn stop_recording(&self) -> VoicyResult<()> {
        if self.get_state() != AppState::Recording {
            println!("Cannot stop recording: current state is {:?}", self.get_state());
            return Ok(());
        }

        println!("🛑 Stopping recording session...");
        self.set_streaming_state(StreamingState::Processing);

        let state = self.state.clone();
        let streaming_state = self.streaming_state.clone();
        let transcription = self.transcription_text.clone();
        let config = self.config.clone();

        thread::spawn(move || {
            thread::sleep(Duration::from_millis(1000));
            
            *state.write() = AppState::Idle;
            *streaming_state.write() = StreamingState::Idle;

            if config.ui.auto_hide_on_stop {
                println!("✅ Recording session completed");
            }
        });

        Ok(())
    }

    pub fn handle_hotkey_press(&self) -> VoicyResult<()> {
        match self.get_state() {
            AppState::Idle => {
                println!("🎙️ Push-to-talk PRESSED - Starting recording");
                self.start_recording()
            }
            _ => {
                println!("Hotkey pressed but app is busy (state: {:?})", self.get_state());
                Ok(())
            }
        }
    }

    pub fn handle_hotkey_release(&self) -> VoicyResult<()> {
        match self.get_state() {
            AppState::Recording => {
                println!("🛑 Push-to-talk RELEASED - Stopping recording");
                self.stop_recording()
            }
            _ => {
                println!("Hotkey released but not recording (state: {:?})", self.get_state());
                Ok(())
            }
        }
    }
}

impl Clone for VoicyApp {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            streaming_state: Arc::clone(&self.streaming_state),
            transcription_text: Arc::clone(&self.transcription_text),
            ui_update_needed: Arc::clone(&self.ui_update_needed),
        }
    }
}