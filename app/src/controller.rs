use crate::services::audio::ImprovedAudioProcessor as AudioProcessor;
use crate::config::Config;
use crate::error::VoicyResult;
use crate::input::HotkeyEvent;
use crate::output::TypingQueue;
use crate::state::{AppStateManager, RecordingState};
use crate::window::WindowManager;
use crate::platform::macos::ffi as menubar_ffi;
use crossbeam_channel::Receiver;
use std::sync::{Arc, Mutex};
use tracing::{info, warn, error, debug};

/// Central controller that owns the app orchestration and processes events.
pub struct AppController {
    state: AppStateManager,
    window_manager: WindowManager,
    typing_queue: TypingQueue,
    audio_processor: Arc<Mutex<AudioProcessor>>,
    config: Arc<parking_lot::RwLock<Config>>,
}

impl AppController {
    pub fn new(config: Config) -> Self {
        let state = AppStateManager::new();

        // Initialize audio processor early so errors surface, but don't crash the app
        let mut audio_processor = AudioProcessor::new(config.clone());
        info!("Initializing audio system...");
        if let Err(e) = audio_processor.initialize() {
            error!(
                "Failed to initialize audio system: {}. Typeswift will still start but recording won't work until model loads",
                e
            );
        } else {
            info!("Audio system initialized successfully");
        }

        let typing_queue = TypingQueue::new(true);

        Self {
            state,
            window_manager: WindowManager::new(),
            typing_queue,
            audio_processor: Arc::new(Mutex::new(audio_processor)),
            config: Arc::new(parking_lot::RwLock::new(config)),
        }
    }

    pub fn state(&self) -> AppStateManager { self.state.clone() }

    pub fn window_manager(&self) -> WindowManager { self.window_manager.clone() }

    pub fn config_handle(&self) -> Arc<parking_lot::RwLock<Config>> { self.config.clone() }

    pub fn start(self, receiver: Receiver<HotkeyEvent>) {
        // Spawn worker thread to process events and periodic tasks
        let AppController {
            state,
            window_manager,
            typing_queue,
            audio_processor,
            config,
        } = self;

        std::thread::spawn(move || {
            info!("Controller started");
            loop {
                match receiver.recv() {
                    Ok(event) => {
                        if let Err(e) = Self::handle_event(
                            &state,
                            &window_manager,
                            &typing_queue,
                            &audio_processor,
                            &config,
                            event,
                        ) {
                            error!("Failed to handle event: {}", e);
                        }
                    }
                    Err(_) => {
                        warn!("Event channel disconnected, controller stopping");
                        break;
                    }
                }
            }
        });
    }

    fn handle_event(
        state: &AppStateManager,
        window_manager: &WindowManager,
        typing_queue: &TypingQueue,
        audio_processor: &Arc<Mutex<AudioProcessor>>,
        config: &Arc<parking_lot::RwLock<Config>>,
        event: HotkeyEvent,
    ) -> VoicyResult<()> {
        info!("Controller handling event: {:?}", event);
        match event {
            HotkeyEvent::OpenPreferences => {
                // Handled by UI layer to open a separate GPUI window.
                // No changes to the main status window here.
            }
            HotkeyEvent::PushToTalkPressed => {
                if state.can_start_recording() {
                    info!("Push-to-talk PRESSED - Starting recording");
                    state.set_recording_state(RecordingState::Recording);
                    state.clear_transcription();
                    window_manager.show_without_focus()?;

                    // Update menu bar icon
                    menubar_ffi::MenuBarController::set_recording(true);

                    if let Ok(mut audio) = audio_processor.lock() {
                        audio.start_recording()?;
                    }
                } else {
                    warn!("Cannot start recording, state: {:?}", state.get_recording_state());
                }
            }
            HotkeyEvent::PushToTalkReleased => {
                if state.can_stop_recording() {
                    info!("Push-to-talk RELEASED - Stopping recording");
                    state.set_recording_state(RecordingState::Processing);
                    // Ensure our window is hidden and focus returns before typing
                    window_manager.hide_and_deactivate_blocking()?;

                    // Update menu bar icon
                    menubar_ffi::MenuBarController::set_recording(false);

                    // Offload finalization to a background thread to keep controller responsive
                    let typing_queue = typing_queue.clone();
                    let audio_processor = Arc::clone(audio_processor);
                    let config = Arc::clone(config);
                    let state = state.clone();
                    std::thread::spawn(move || {
                        let final_text = if let Ok(mut audio) = audio_processor.lock() {
                            audio.stop_recording().unwrap_or_default()
                        } else {
                            String::new()
                        };

                        // Ensure PTT modifiers are fully released and focus returned before typing
                            info!("Waiting for modifier release before typing...");
                            let _ = menubar_ffi::wait_modifiers_released(300);
                        // Small delay for app focus settle
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        info!("Queueing typing: len={}, add_space={} ", final_text.len(), config.read().output.add_space_between_utterances);

                        let typing_enabled = config.read().output.enable_typing;
                        debug!("Typing decision -> enabled: {}, text_len: {}", typing_enabled, final_text.len());

                        if !final_text.is_empty() && typing_enabled {
                            let add_space = config.read().output.add_space_between_utterances;
                            info!("Typing final text ({} chars)", final_text.len());
                            match typing_queue.queue_typing(final_text.clone(), add_space) {
                                Ok(()) => info!("Typing queued successfully"),
                                Err(e) => error!("Failed to queue typing: {}", e),
                            }
                        }

                        state.set_recording_state(RecordingState::Idle);
                        info!("Processing complete; state=Idle");
                    });
                } else {
                    warn!("Cannot stop recording, state: {:?}", state.get_recording_state());
                }
            }
            HotkeyEvent::ToggleWindow => {
                if state.is_window_visible() {
                    window_manager.hide()?;
                    state.set_window_visible(false);
                } else {
                    window_manager.show_without_focus()?;
                    state.set_window_visible(true);
                }
            }
        }

        Ok(())
    }
}
