use parking_lot::RwLock;
use std::sync::Arc;

/// Single source of truth for application state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

/// Observable state container
pub struct AppStateManager {
    recording_state: Arc<RwLock<RecordingState>>,
    transcription: Arc<RwLock<String>>,
    is_window_visible: Arc<RwLock<bool>>,
    is_preferences_visible: Arc<RwLock<bool>>,
    listeners: Arc<RwLock<Vec<Box<dyn Fn() + Send + Sync>>>>,
}

impl AppStateManager {
    pub fn new() -> Self {
        Self {
            recording_state: Arc::new(RwLock::new(RecordingState::Idle)),
            transcription: Arc::new(RwLock::new(String::new())),
            is_window_visible: Arc::new(RwLock::new(false)),
            is_preferences_visible: Arc::new(RwLock::new(false)),
            listeners: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    pub fn get_recording_state(&self) -> RecordingState {
        *self.recording_state.read()
    }
    
    pub fn set_recording_state(&self, state: RecordingState) {
        let old_state = *self.recording_state.read();
        if old_state != state {
            println!("📊 State transition: {:?} -> {:?}", old_state, state);
            *self.recording_state.write() = state;
            self.notify_listeners();
        }
    }
    
    pub fn get_transcription(&self) -> String {
        self.transcription.read().clone()
    }
    
    pub fn set_transcription(&self, text: String) {
        *self.transcription.write() = text;
        self.notify_listeners();
    }
    
    pub fn append_transcription(&self, text: &str) {
        self.transcription.write().push_str(text);
        self.notify_listeners();
    }
    
    pub fn clear_transcription(&self) {
        self.transcription.write().clear();
        self.notify_listeners();
    }
    
    pub fn is_window_visible(&self) -> bool {
        *self.is_window_visible.read()
    }
    
    pub fn set_window_visible(&self, visible: bool) {
        *self.is_window_visible.write() = visible;
        self.notify_listeners();
    }

    pub fn is_preferences_visible(&self) -> bool {
        *self.is_preferences_visible.read()
    }

    pub fn set_preferences_visible(&self, visible: bool) {
        *self.is_preferences_visible.write() = visible;
        self.notify_listeners();
    }
    
    pub fn add_listener<F>(&self, listener: F) 
    where 
        F: Fn() + Send + Sync + 'static
    {
        self.listeners.write().push(Box::new(listener));
    }
    
    fn notify_listeners(&self) {
        for listener in self.listeners.read().iter() {
            listener();
        }
    }
    
    /// Check if we can start recording
    pub fn can_start_recording(&self) -> bool {
        self.get_recording_state() == RecordingState::Idle
    }
    
    /// Check if we can stop recording
    pub fn can_stop_recording(&self) -> bool {
        self.get_recording_state() == RecordingState::Recording
    }
}

impl Clone for AppStateManager {
    fn clone(&self) -> Self {
        Self {
            recording_state: Arc::clone(&self.recording_state),
            transcription: Arc::clone(&self.transcription),
            is_window_visible: Arc::clone(&self.is_window_visible),
            is_preferences_visible: Arc::clone(&self.is_preferences_visible),
            listeners: Arc::clone(&self.listeners),
        }
    }
}
