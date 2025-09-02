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
    streaming_manager: StreamingManager,
    audio_processor: Arc<Mutex<AudioProcessor>>,
    config: Config,
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
        let streaming_manager = StreamingManager::new(typing_queue.clone());

        Self {
            state,
            window_manager: WindowManager::new(),
            typing_queue,
            streaming_manager,
            audio_processor: Arc::new(Mutex::new(audio_processor)),
            config,
        }
    }

    pub fn state(&self) -> AppStateManager { self.state.clone() }

    pub fn window_manager(&self) -> WindowManager { self.window_manager.clone() }

    pub fn start(self, receiver: Receiver<HotkeyEvent>) {
        // Spawn worker thread to process events and periodic tasks
        let AppController {
            state,
            window_manager,
            typing_queue,
            streaming_manager,
            audio_processor,
            config,
        } = self;

        std::thread::spawn(move || {
            println!("🔄 Controller started");
            loop {
                match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(event) => {
                        if let Err(e) = Self::handle_event(
                            &state,
                            &window_manager,
                            &typing_queue,
                            &streaming_manager,
                            &audio_processor,
                            &config,
                            event,
                        ) {
                            eprintln!("❌ Failed to handle event: {}", e);
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                        // Periodic tasks (e.g., live transcription) if ever supported
                        if config.streaming.enabled
                            && state.get_recording_state() == RecordingState::Recording
                        {
                            if let Ok(audio) = audio_processor.lock() {
                                if let Some(live_text) = audio.get_live_transcription() {
                                    // Update UI with live transcription
                                    state.set_transcription(live_text.clone());

                                    // Type incrementally in streaming mode
                                    if config.output.enable_typing {
                                        streaming_manager.process_live_text(&live_text);
                                    }
                                }
                            }
                        }
                    }
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
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
        streaming_manager: &StreamingManager,
        audio_processor: &Arc<Mutex<AudioProcessor>>,
        config: &Config,
        event: HotkeyEvent,
    ) -> VoicyResult<()> {
        println!("🎬 Controller handling event: {:?}", event);
        match event {
            HotkeyEvent::PushToTalkPressed => {
                if state.can_start_recording() {
                    println!("🎙️ Push-to-talk PRESSED - Starting recording");
                    state.set_recording_state(RecordingState::Recording);
                    state.clear_transcription();
                    streaming_manager.reset();
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
                    window_manager.hide()?;

                    // Update menu bar icon
                    #[cfg(target_os = "macos")]
                    menubar_ffi::MenuBarController::set_recording(false);

                    // Stop recording and get final text
                    let final_text = if let Ok(mut audio) = audio_processor.lock() {
                        audio.stop_recording().unwrap_or_default()
                    } else {
                        String::new()
                    };

                    if config.streaming.enabled {
                        // Streaming mode: type remaining text not yet typed
                        if !final_text.is_empty() && config.output.enable_typing {
                            let current_transcription = state.get_transcription();
                            if final_text.len() > current_transcription.len() {
                                let remaining_text = &final_text[current_transcription.len()..];
                                if !remaining_text.is_empty() {
                                    typing_queue
                                        .queue_typing(
                                            remaining_text.to_string(),
                                            config.output.add_space_between_utterances,
                                        )
                                        .ok();
                                }
                            }
                        }
                    } else {
                        // Normal mode: type all text at once after release
                        if !final_text.is_empty() && config.output.enable_typing {
                            println!("💬 Typing final text: '{}'", final_text);
                            typing_queue
                                .queue_typing(
                                    final_text,
                                    config.output.add_space_between_utterances,
                                )
                                .ok();
                        }
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

// Lightweight streaming manager inlined here to avoid an extra file.
#[derive(Clone)]
struct StreamingManager {
    typing_queue: TypingQueue,
    typed_text: Arc<parking_lot::RwLock<String>>,
    pending_text: Arc<parking_lot::RwLock<String>>,
}

impl StreamingManager {
    fn new(typing_queue: TypingQueue) -> Self {
        Self {
            typing_queue,
            typed_text: Arc::new(parking_lot::RwLock::new(String::new())),
            pending_text: Arc::new(parking_lot::RwLock::new(String::new())),
        }
    }

    fn process_live_text(&self, new_full_text: &str) {
        let typed = self.typed_text.read();
        if new_full_text.len() > typed.len() {
            if new_full_text.starts_with(typed.as_str()) {
                let new_part = &new_full_text[typed.len()..];
                if !new_part.is_empty() {
                    println!("⌨️ Live typing: '{}'", new_part);
                    if self
                        .typing_queue
                        .queue_typing(new_part.to_string(), false)
                        .is_ok()
                    {
                        drop(typed);
                        let mut typed_mut = self.typed_text.write();
                        *typed_mut = new_full_text.to_string();
                    }
                }
            } else {
                // Non-prefix change (model correction). Record pending.
                let mut pending = self.pending_text.write();
                *pending = new_full_text.to_string();
            }
        }
    }

    fn reset(&self) {
        let mut typed = self.typed_text.write();
        typed.clear();
        let mut pending = self.pending_text.write();
        pending.clear();
    }

    #[allow(dead_code)]
    fn get_pending_corrections(&self) -> Option<String> {
        let pending = self.pending_text.read();
        if !pending.is_empty() {
            Some(pending.clone())
        } else {
            None
        }
    }
}
