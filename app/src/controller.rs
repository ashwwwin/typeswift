use crate::services::audio::ImprovedAudioProcessor as AudioProcessor;
use crate::config::Config;
use crate::error::VoicyResult;
use crate::input::HotkeyEvent;
use crate::output::TypingQueue;
use crate::state::{AppStateManager, RecordingState};
use crate::window::WindowManager;
#[cfg(target_os = "macos")]
use crate::platform::macos::ffi as menubar_ffi;
use crossbeam_channel::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
        println!("🚀 Initializing audio system...");
        if let Err(e) = audio_processor.initialize() {
            eprintln!(
                "❌ Failed to initialize audio system: {}\n   Voicy will still start but recording won't work until model loads",
                e
            );
        } else {
            println!("✅ Audio system initialized successfully");
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
            println!("🔄 Controller started");
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
                            eprintln!("❌ Failed to handle event: {}", e);
                        }
                    }
                    Err(_) => {
                        eprintln!("⚠️ Event channel disconnected, controller stopping");
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
        println!("🎬 Controller handling event: {:?}", event);
        match event {
            HotkeyEvent::OpenPreferences => {
                // Handled by UI layer to open a separate GPUI window.
                // No changes to the main status window here.
            }
            HotkeyEvent::PushToTalkPressed => {
                if state.can_start_recording() {
                    println!("🎙️ Push-to-talk PRESSED - Starting recording");
                    state.set_recording_state(RecordingState::Recording);
                    state.clear_transcription();
                    window_manager.show_without_focus()?;

                    // Update menu bar icon
                    #[cfg(target_os = "macos")]
                    menubar_ffi::MenuBarController::set_recording(true);

                    if let Ok(mut audio) = audio_processor.lock() {
                        audio.start_recording()?;
                    }
                } else {
                    println!("⚠️ Cannot start recording, state: {:?}", state.get_recording_state());
                }
            }
            HotkeyEvent::PushToTalkReleased => {
                if state.can_stop_recording() {
                    println!("🛑 Push-to-talk RELEASED - Stopping recording");
                    state.set_recording_state(RecordingState::Processing);
                    // Ensure our window is hidden and focus returns before typing
                    window_manager.hide_and_deactivate_blocking()?;

                    // Update menu bar icon
                    #[cfg(target_os = "macos")]
                    menubar_ffi::MenuBarController::set_recording(false);

                    // Stop recording and get final text
                    let final_text = if let Ok(mut audio) = audio_processor.lock() {
                        audio.stop_recording().unwrap_or_default()
                    } else {
                        String::new()
                    };

                    let typing_enabled = config.read().output.enable_typing;
                    println!(
                        "🔎 Typing decision -> enabled: {}, text_len: {}",
                        typing_enabled,
                        final_text.len()
                    );

                    // Always type all text at once after release (streaming removed)
                    if !final_text.is_empty() && typing_enabled {
                        println!("💬 Typing final text: '{}'", final_text);
                        typing_queue
                            .queue_typing(
                                final_text,
                                config.read().output.add_space_between_utterances,
                            )
                            .ok();
                    }

                    state.set_recording_state(RecordingState::Idle);
                } else {
                    println!("⚠️ Cannot stop recording, state: {:?}", state.get_recording_state());
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
