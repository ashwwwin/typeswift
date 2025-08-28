mod audio;
mod config;
mod error;
mod event_loop;
mod input;
mod output;
mod state;
mod streaming_manager;
mod window;

use audio::ImprovedAudioProcessor as AudioProcessor;
use config::Config;
use error::VoicyResult;
use event_loop::{EventCallback, EventLoop};
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, rgb, size,
};
use input::{HotkeyEvent, HotkeyHandler};
use output::{TypingQueue, run_typing_diagnostic};
use state::{AppStateManager, RecordingState};
use streaming_manager::StreamingManager;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use window::WindowManager;

struct Voicy {
    state: AppStateManager,
    window_manager: WindowManager,
    typing_queue: TypingQueue,
    streaming_manager: StreamingManager,
    audio_processor: Arc<Mutex<AudioProcessor>>,
    config: Config,
    event_queue: Option<Arc<Mutex<Vec<HotkeyEvent>>>>,
}

impl Voicy {
    fn new(_cx: &mut Context<Self>) -> Self {
        let config = Config::load().unwrap_or_default();
        let state = AppStateManager::new();

        // Initialize audio processor
        let mut audio_processor = AudioProcessor::new(config.clone());

        println!("🚀 Initializing audio system...");
        match audio_processor.initialize() {
            Ok(()) => println!("✅ Audio system initialized successfully"),
            Err(e) => {
                eprintln!("❌ Failed to initialize audio system: {}", e);
                eprintln!("   Voicy will still start but recording won't work until model loads");
            }
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
            event_queue: None,
        }
    }

    fn set_event_queue(&mut self, queue: Arc<Mutex<Vec<HotkeyEvent>>>) {
        self.event_queue = Some(queue);
    }

    fn poll_events(&mut self) {
        // First, collect events from the queue
        let events_to_process = if let Some(ref queue) = self.event_queue {
            if let Ok(mut events) = queue.lock() {
                let count = events.len();
                if count > 0 {
                    println!("📥 Polling events, found {} events to process", count);
                }
                events.drain(..).collect::<Vec<HotkeyEvent>>()
            } else {
                Vec::new()
            }
        } else {
            println!("⚠️ No event queue set!");
            Vec::new()
        };

        // Then process them after releasing all locks
        for event in events_to_process {
            println!("🎬 Processing event: {:?}", event);
            if let Err(e) = self.handle_hotkey_event(event) {
                eprintln!("❌ Failed to handle event: {}", e);
            } else {
                println!("✅ Event handled successfully");
            }
        }
    }

    fn handle_hotkey_event(&mut self, event: HotkeyEvent) -> VoicyResult<()> {
        match event {
            HotkeyEvent::PushToTalkPressed => {
                if self.state.can_start_recording() {
                    println!("🎙️ Push-to-talk PRESSED - Starting recording");
                    self.state.set_recording_state(RecordingState::Recording);
                    self.state.clear_transcription();
                    self.streaming_manager.reset();  // Reset streaming manager
                    self.window_manager.show_without_focus()?;

                    // Start recording in audio processor
                    if let Ok(mut audio) = self.audio_processor.lock() {
                        if let Err(e) = audio.start_recording() {
                            eprintln!("❌ Failed to start recording: {}", e);
                            self.state.set_recording_state(RecordingState::Idle);
                            return Err(e);
                        }
                    }
                } else {
                    println!(
                        "⚠️ Cannot start recording, state: {:?}",
                        self.state.get_recording_state()
                    );
                }
            }

            HotkeyEvent::PushToTalkReleased => {
                if self.state.can_stop_recording() {
                    println!("🛑 Push-to-talk RELEASED - Stopping recording");
                    self.state.set_recording_state(RecordingState::Processing);
                    self.window_manager.hide()?;

                    // Stop recording and get final text
                    let final_text = if let Ok(mut audio) = self.audio_processor.lock() {
                        match audio.stop_recording() {
                            Ok(text) => text,
                            Err(e) => {
                                eprintln!("❌ Failed to stop recording: {}", e);
                                self.state.set_recording_state(RecordingState::Idle);
                                return Err(e);
                            }
                        }
                    } else {
                        String::new()
                    };

                    // Type the text if enabled
                    if !final_text.is_empty() && self.config.output.enable_typing {
                        let add_space = self.config.output.add_space_between_utterances;
                        println!("💬 Typing: '{}'", final_text);
                        self.typing_queue.queue_typing(final_text, add_space)?;
                    }

                    self.state.set_recording_state(RecordingState::Idle);
                } else {
                    println!(
                        "⚠️ Cannot stop recording, state: {:?}",
                        self.state.get_recording_state()
                    );
                }
            }

            HotkeyEvent::ToggleWindow => {
                if self.state.is_window_visible() {
                    self.window_manager.hide()?;
                    self.state.set_window_visible(false);
                } else {
                    self.window_manager.show_without_focus()?;
                    self.state.set_window_visible(true);
                }
            }

            HotkeyEvent::StartRecording => {
                if self.state.can_start_recording() {
                    self.handle_hotkey_event(HotkeyEvent::PushToTalkPressed)?;
                }
            }

            HotkeyEvent::StopRecording => {
                if self.state.can_stop_recording() {
                    self.handle_hotkey_event(HotkeyEvent::PushToTalkReleased)?;
                }
            }
        }

        Ok(())
    }

    fn poll_live_transcription(&mut self) {
        // Check for live transcriptions while recording
        if self.state.get_recording_state() == RecordingState::Recording {
            if let Ok(audio) = self.audio_processor.lock() {
                if let Some(live_text) = audio.get_live_transcription() {
                    self.state.append_transcription(&live_text);
                }
            }
        }
    }

    fn process_typing_queue(&mut self) {
        if let Err(e) = self.typing_queue.process_queue() {
            eprintln!("⚠️ Typing error: {}", e);
        }
    }
}

impl Voicy {
    fn start_polling(&self, _cx: &mut Context<Self>) {
        // Use a background thread to poll events
        let event_queue = self.event_queue.clone();
        let audio = self.audio_processor.clone();
        let typing_queue = self.typing_queue.clone();
        let streaming_manager = self.streaming_manager.clone();
        let state = self.state.clone();
        let window_manager = self.window_manager.clone();
        let config = self.config.clone();

        std::thread::spawn(move || {
            loop {
                // Poll and process events directly in background thread
                if let Some(ref queue) = event_queue {
                    if let Ok(mut events) = queue.lock() {
                        for event in events.drain(..) {
                            println!("🎬 Background processing event: {:?}", event);

                            match event {
                                HotkeyEvent::PushToTalkPressed => {
                                    if state.can_start_recording() {
                                        println!("🎙️ Starting recording");
                                        state.set_recording_state(RecordingState::Recording);
                                        state.clear_transcription();
                                        streaming_manager.reset();  // Reset for new recording
                                        window_manager.show_without_focus().ok();

                                        if let Ok(mut audio) = audio.lock() {
                                            audio.start_recording().ok();
                                        }
                                    }
                                }
                                HotkeyEvent::PushToTalkReleased => {
                                    if state.can_stop_recording() {
                                        println!("🛑 Stopping recording");
                                        state.set_recording_state(RecordingState::Processing);
                                        window_manager.hide().ok();

                                        let final_text = if let Ok(mut audio) = audio.lock() {
                                            audio.stop_recording().unwrap_or_default()
                                        } else {
                                            String::new()
                                        };

                                        if config.streaming.enabled {
                                            // Streaming mode: only type remaining text not yet typed
                                            if let Some(corrected_text) = streaming_manager.get_pending_corrections() {
                                                println!("🔄 Corrections pending: '{}'", corrected_text);
                                            }
                                            
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
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Poll for live transcriptions only if streaming is enabled
                if config.streaming.enabled && state.get_recording_state() == RecordingState::Recording {
                    if let Ok(audio) = audio.lock() {
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

                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }
}

impl Render for Voicy {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Just render, no polling here

        let recording_state = self.state.get_recording_state();
        let transcription = self.state.get_transcription();

        let status_text = match recording_state {
            RecordingState::Idle => "Ready".to_string(),
            RecordingState::Recording => {
                if transcription.is_empty() {
                    "Listening...".to_string()
                } else {
                    transcription.clone()
                }
            }
            RecordingState::Processing => "Processing...".to_string(),
        };

        let bg_color = match recording_state {
            RecordingState::Idle => rgb(0x1f2937),
            RecordingState::Recording => rgb(0xdc2626),
            RecordingState::Processing => rgb(0x3b82f6),
        };

        div()
            .id("voicy-main")
            .flex()
            .flex_col()
            .bg(bg_color)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(match recording_state {
                RecordingState::Idle => rgb(0x374151),
                RecordingState::Recording => rgb(0xef4444),
                RecordingState::Processing => rgb(0x60a5fa),
            })
            .text_xs()
            .text_color(rgb(0xffffff))
            .child(status_text)
    }
}

fn main() {
    // Check for diagnostic flag
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--typing-diagnostic" {
        run_typing_diagnostic();
        return;
    }

    // Load configuration
    let config = Config::load().unwrap_or_default();

    // Initialize hotkey handler
    let mut hotkey_handler = HotkeyHandler::new().expect("Failed to create hotkey handler");

    // Register hotkeys
    if let Err(e) = hotkey_handler.register_hotkeys(&config.hotkeys) {
        eprintln!("⚠️ Failed to register hotkeys: {}", e);
        return;
    }

    // Start the hotkey event loop
    let hotkey_receiver = hotkey_handler.start_event_loop();

    // Clone config for the closure
    let config_clone = config.clone();

    Application::new().run(move |cx: &mut App| {
        let window_size = size(
            px(config_clone.ui.window_width),
            px(config_clone.ui.window_height),
        );
        let gap_from_bottom = px(config_clone.ui.gap_from_bottom);

        // Get the primary display
        let displays = cx.displays();
        let screen = displays.first().expect("No displays found");

        // Calculate position for bottom center with gap
        let bounds = Bounds {
            origin: point(
                screen.bounds().center().x - window_size.width / 2.,
                screen.bounds().size.height - window_size.height - gap_from_bottom,
            ),
            size: window_size,
        };

        // Store events in a shared queue that Voicy can poll
        let event_queue = Arc::new(Mutex::new(Vec::new()));
        let event_queue_clone = event_queue.clone();
        let event_queue_for_voicy = event_queue.clone();

        let window = cx
            .open_window(
                WindowOptions {
                    is_movable: false,
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    display_id: Some(screen.id()),
                    focus: false,
                    show: false, // Must be visible for render to be called
                    kind: gpui::WindowKind::PopUp,
                    ..Default::default()
                },
                move |_window, cx| {
                    cx.new(|cx| {
                        let mut voicy = Voicy::new(cx);
                        voicy.set_event_queue(event_queue_for_voicy);
                        voicy.start_polling(cx);
                        voicy
                    })
                },
            )
            .unwrap();

        let _window_for_callback = window.clone();

        // Create the event callback that will handle hotkey events
        let event_callback: EventCallback = Arc::new(Mutex::new(move |event| {
            println!("🎯 Event callback triggered for: {:?}", event);
            // Queue the event for processing
            if let Ok(mut queue) = event_queue_clone.lock() {
                queue.push(event);
                println!("📦 Event queued successfully, queue size: {}", queue.len());

                // Note: Window updates need to happen on the main thread
                // The event will be processed on next render cycle
                println!("🔔 Event queued, will be processed on next render");

                Ok(())
            } else {
                Err(error::VoicyError::WindowOperationFailed(
                    "Failed to queue event".to_string(),
                ))
            }
        }));

        // Start the dedicated event loop
        let event_loop = EventLoop::new(hotkey_receiver, event_callback);
        let _event_loop_handle = event_loop.start();

        // Set up window properties
        if let Err(e) = WindowManager::setup_properties() {
            eprintln!("⚠️ Failed to setup window properties: {}", e);
        }

        // Initialize typing queue on main thread
        let typing_queue = TypingQueue::new(false);
        if let Err(e) = typing_queue.initialize_on_main_thread() {
            eprintln!("⚠️ Failed to initialize typing: {}", e);
        }

        println!("🚀 Voicy started with global shortcuts:");
        println!(
            "   Push-to-talk: {} (hold to record)",
            config_clone.hotkeys.push_to_talk
        );
        if let Some(ref key) = config_clone.hotkeys.toggle_window {
            println!("   Toggle window: {}", key);
        }
        println!("✅ Event loop running independently of UI");
    });
}
