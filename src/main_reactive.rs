mod app;
mod audio_improved;
mod config;
mod error;
mod input;
mod output;
mod window;

use audio_improved::ImprovedAudioProcessor as AudioProcessor;
use config::Config;
use error::VoicyResult;
use gpui::{
    actions, App, AppContext, Application, AsyncAppContext, Bounds, Context, Entity, EventEmitter,
    Global, Model, ModelContext, Render, View, ViewContext, VisualContext, Window, WindowBounds,
    WindowOptions, div, point, prelude::*, px, rgb, size,
};
use input::{HotkeyEvent, HotkeyHandler};
use output::{TypingQueue, run_typing_diagnostic};
use std::sync::Arc;
use std::time::Duration;
use window::WindowManager;

// Define actions that can be triggered
actions!(voicy, [StartRecording, StopRecording, ToggleWindow]);

// Global singleton for hotkey channel
struct HotkeyChannel(std::sync::mpsc::Receiver<HotkeyEvent>);
impl Global for HotkeyChannel {}

// Reactive state model
#[derive(Clone, Debug, PartialEq)]
enum RecordingState {
    Idle,
    Recording,
    Processing,
}

struct AppState {
    recording_state: RecordingState,
    transcription: String,
    window_visible: bool,
    config: Config,
}

impl AppState {
    fn new(config: Config) -> Self {
        Self {
            recording_state: RecordingState::Idle,
            transcription: String::new(),
            window_visible: false,
            config,
        }
    }
}

// Event emitter for state changes
impl EventEmitter<StateChanged> for AppState {}

#[derive(Clone, Debug)]
struct StateChanged;

// The main view component
struct VoicyView {
    state: Model<AppState>,
    audio: Arc<parking_lot::Mutex<AudioProcessor>>,
    typing_queue: TypingQueue,
    window_manager: WindowManager,
}

impl VoicyView {
    fn new(cx: &mut ViewContext<Self>) -> Self {
        let config = Config::load().unwrap_or_default();
        
        // Create reactive state model
        let state = cx.new_model(|_| AppState::new(config.clone()));
        
        // Subscribe to state changes
        cx.observe(&state, |this, _, cx| {
            // State changed, UI will automatically re-render
            cx.notify();
        }).detach();
        
        // Initialize audio
        let mut audio_processor = AudioProcessor::new(config.clone());
        if let Err(e) = audio_processor.initialize() {
            eprintln!("Failed to initialize audio: {}", e);
        }
        
        // Poll for hotkey events reactively
        cx.spawn(|this, mut cx| async move {
            loop {
                cx.update(|cx| {
                    if let Some(channel) = cx.try_global::<HotkeyChannel>() {
                        while let Ok(event) = channel.0.try_recv() {
                            cx.dispatch_action(match event {
                                HotkeyEvent::PushToTalkPressed => StartRecording.boxed_clone(),
                                HotkeyEvent::PushToTalkReleased => StopRecording.boxed_clone(),
                                HotkeyEvent::ToggleWindow => ToggleWindow.boxed_clone(),
                                _ => continue,
                            });
                        }
                    }
                }).ok();
                
                smol::Timer::after(Duration::from_millis(10)).await;
            }
        }).detach();
        
        Self {
            state,
            audio: Arc::new(parking_lot::Mutex::new(audio_processor)),
            typing_queue: TypingQueue::new(true),
            window_manager: WindowManager::new(),
        }
    }
    
    fn start_recording(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            if state.recording_state != RecordingState::Idle {
                return;
            }
            
            state.recording_state = RecordingState::Recording;
            state.transcription.clear();
            cx.emit(StateChanged);
        });
        
        // Start audio recording asynchronously
        let audio = self.audio.clone();
        cx.spawn(|this, mut cx| async move {
            if let Ok(mut audio) = audio.lock() {
                if let Err(e) = audio.start_recording() {
                    eprintln!("Failed to start recording: {}", e);
                }
            }
            
            // Poll for live transcriptions
            while this.update(&mut cx, |this, cx| {
                this.state.read(cx).recording_state == RecordingState::Recording
            }).unwrap_or(false) {
                if let Ok(audio) = audio.lock() {
                    if let Some(text) = audio.get_live_transcription() {
                        this.update(&mut cx, |this, cx| {
                            this.state.update(cx, |state, cx| {
                                state.transcription.push_str(&text);
                                cx.emit(StateChanged);
                            });
                        }).ok();
                    }
                }
                smol::Timer::after(Duration::from_millis(100)).await;
            }
        }).detach();
        
        self.window_manager.show_without_focus().ok();
    }
    
    fn stop_recording(&mut self, cx: &mut ViewContext<Self>) {
        self.state.update(cx, |state, cx| {
            if state.recording_state != RecordingState::Recording {
                return;
            }
            
            state.recording_state = RecordingState::Processing;
            cx.emit(StateChanged);
        });
        
        // Stop audio and process final text
        let audio = self.audio.clone();
        let typing_queue = self.typing_queue.clone();
        let config = self.state.read(cx).config.clone();
        
        cx.spawn(|this, mut cx| async move {
            let final_text = if let Ok(mut audio) = audio.lock() {
                audio.stop_recording().unwrap_or_default()
            } else {
                String::new()
            };
            
            if !final_text.is_empty() && config.output.enable_typing {
                typing_queue.queue_typing(
                    final_text,
                    config.output.add_space_between_utterances
                ).ok();
            }
            
            this.update(&mut cx, |this, cx| {
                this.state.update(cx, |state, cx| {
                    state.recording_state = RecordingState::Idle;
                    cx.emit(StateChanged);
                });
            }).ok();
        }).detach();
        
        self.window_manager.hide().ok();
    }
}

impl Render for VoicyView {
    fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        
        let status_text = match state.recording_state {
            RecordingState::Idle => "Ready",
            RecordingState::Recording => {
                if state.transcription.is_empty() {
                    "Listening..."
                } else {
                    &state.transcription
                }
            }
            RecordingState::Processing => "Processing...",
        };
        
        let bg_color = match state.recording_state {
            RecordingState::Idle => rgb(0x1f2937),
            RecordingState::Recording => rgb(0xdc2626),
            RecordingState::Processing => rgb(0x3b82f6),
        };
        
        div()
            .id("voicy")
            .flex()
            .flex_col()
            .bg(bg_color)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(match state.recording_state {
                RecordingState::Idle => rgb(0x374151),
                RecordingState::Recording => rgb(0xef4444),
                RecordingState::Processing => rgb(0x60a5fa),
            })
            .text_xs()
            .text_color(rgb(0xffffff))
            .child(status_text)
            // Register action handlers
            .on_action(cx.listener(Self::start_recording))
            .on_action(cx.listener(Self::stop_recording))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--typing-diagnostic" {
        run_typing_diagnostic();
        return;
    }
    
    // Initialize hotkeys OUTSIDE of GPUI
    let config = Config::load().unwrap_or_default();
    let mut hotkey_handler = HotkeyHandler::new().expect("Failed to create hotkey handler");
    
    if let Err(e) = hotkey_handler.register_hotkeys(&config.hotkeys) {
        eprintln!("Failed to register hotkeys: {}", e);
        return;
    }
    
    let hotkey_receiver = hotkey_handler.start_event_loop();
    
    Application::new().run(move |cx: &mut App| {
        // Store the hotkey channel as a global
        cx.set_global(HotkeyChannel(hotkey_receiver));
        
        let window_size = size(
            px(config.ui.window_width),
            px(config.ui.window_height),
        );
        
        let displays = cx.displays();
        let screen = displays.first().expect("No displays found");
        
        let bounds = Bounds {
            origin: point(
                screen.bounds().center().x - window_size.width / 2.,
                screen.bounds().size.height - window_size.height - px(config.ui.gap_from_bottom),
            ),
            size: window_size,
        };
        
        cx.open_window(
            WindowOptions {
                is_movable: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(screen.id()),
                focus: false,
                show: false,
                kind: gpui::WindowKind::PopUp,
                ..Default::default()
            },
            |cx| cx.new_view(VoicyView::new),
        ).unwrap();
        
        WindowManager::setup_properties().ok();
        
        println!("🚀 Voicy started (reactive architecture)");
        println!("   Push-to-talk: {}", config.hotkeys.push_to_talk);
    });
}