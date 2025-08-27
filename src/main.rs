mod audio_stream;
mod config;
mod mlx;
mod streaming_processor;

use audio_stream::AudioStream;
use config::Config;
use streaming_processor::{StreamingProcessor, vad_processing_loop};
use enigo::{Enigo, Keyboard, Settings};
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, rgb, size,
};
use mlx::MLXParakeet;
use std::sync::{Arc, Mutex};
use std::thread;

struct Voicy {
    config: Config,
    audio_stream: Option<AudioStream>,
    mlx_model: Option<MLXParakeet>,
    state: RecordingState,
    transcription_text: Arc<Mutex<String>>,
    processing_thread: Option<thread::JoinHandle<()>>,
    should_stop: Arc<Mutex<bool>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RecordingState {
    Idle,
    Recording,
    Error,
}

impl Voicy {
    fn new() -> Self {
        let config = Config::load().unwrap_or_default();
        
        // Don't load model on startup - wait until needed
        Self {
            config,
            audio_stream: None,
            mlx_model: None, // Start with no model loaded
            state: RecordingState::Idle,
            transcription_text: Arc::new(Mutex::new(String::new())),
            processing_thread: None,
            should_stop: Arc::new(Mutex::new(false)),
        }
    }

    fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        match self.state {
            RecordingState::Idle => {
                self.start_streaming(cx);
            }
            RecordingState::Recording => {
                self.stop_streaming(cx);
            }
            _ => {}
        }
    }

    fn start_streaming(&mut self, cx: &mut Context<Self>) {
        // Load model on demand if not already loaded
        if self.mlx_model.is_none() {
            println!("🚀 Loading MLX model on demand...");
            match MLXParakeet::new() {
                Ok(model) => {
                    println!("✅ Model loaded successfully");
                    self.mlx_model = Some(model);
                }
                Err(e) => {
                    eprintln!("❌ Failed to load model: {}", e);
                    self.state = RecordingState::Error;
                    cx.notify();
                    return;
                }
            }
        }

        if let Some(ref mlx_model) = self.mlx_model {
            // Get the required sample rate from the model
            let sample_rate = mlx_model.get_sample_rate();

            // Initialize audio stream
            match AudioStream::new(sample_rate) {
                Ok(stream) => {
                    // Start the audio stream
                    if let Err(e) = stream.start() {
                        eprintln!("Failed to start audio stream: {}", e);
                        self.state = RecordingState::Error;
                        cx.notify();
                        return;
                    }

                    // Start MLX streaming with context windows from config
                    let left_context = self.config.model.left_context_seconds;
                    let right_context = self.config.model.right_context_seconds;

                    if let Err(e) = mlx_model.start_streaming(left_context, right_context) {
                        eprintln!("Failed to start MLX streaming: {}", e);
                        stream.stop();
                        self.state = RecordingState::Error;
                        cx.notify();
                        return;
                    }

                    // Clear previous transcription  
                    *self.transcription_text.lock().unwrap() = String::new();

                    // Set up the processing thread
                    *self.should_stop.lock().unwrap() = false;
                    let should_stop = self.should_stop.clone();
                    let transcription_text = self.transcription_text.clone();
                    let config_clone = self.config.clone();
                    let stream_clone = stream.clone();
                    let mlx_model_clone = mlx_model.clone();

                    // Start processing thread - choose mode based on config
                    let handle = thread::spawn(move || {
                        if config_clone.streaming.enabled {
                            // Use real-time streaming processor
                            let processor = StreamingProcessor::new(config_clone.clone());
                            processor.process_loop(
                                stream_clone,
                                mlx_model_clone,
                                transcription_text,
                                should_stop,
                            );
                        } else {
                            // Use VAD-based processor
                            vad_processing_loop(
                                stream_clone,
                                mlx_model_clone,
                                transcription_text,
                                should_stop,
                                config_clone,
                                sample_rate,
                            );
                        }
                    });

                    self.audio_stream = Some(stream);
                    self.processing_thread = Some(handle);
                    self.state = RecordingState::Recording;

                    println!("🎙️ Started real-time transcription");
                }
                Err(e) => {
                    eprintln!("Failed to create audio stream: {}", e);
                    self.state = RecordingState::Error;
                }
            }
        } else {
            println!("⚠️  No MLX model available");
            self.state = RecordingState::Error;
        }
        cx.notify();
    }

    fn stop_streaming(&mut self, cx: &mut Context<Self>) {
        // Signal the processing thread to stop
        *self.should_stop.lock().unwrap() = true;

        // Stop the audio stream
        if let Some(ref stream) = self.audio_stream {
            stream.stop();
        }

        // Stop MLX streaming and get any remaining text
        if let Some(ref mlx_model) = self.mlx_model {
            match mlx_model.stop_streaming() {
                Ok(remaining_text) => {
                    if !remaining_text.is_empty() {
                        println!("\n📝 Remaining text:\n{}", remaining_text);
                        
                        // Type any remaining text that wasn't processed yet
                        if self.config.output.enable_typing {
                            if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
                                // Add space if there was previous text
                                let current_text = self.transcription_text.lock().unwrap();
                                if self.config.output.add_space_between_utterances && !current_text.is_empty() {
                                    let _ = enigo.text(" ");
                                }
                                drop(current_text);
                                
                                // Type the remaining text
                                let _ = enigo.text(&remaining_text);
                            }
                        }
                        
                        // Append to transcription text
                        let mut transcription = self.transcription_text.lock().unwrap();
                        if !transcription.is_empty() {
                            transcription.push(' ');
                        }
                        transcription.push_str(&remaining_text);
                    }
                }
                Err(e) => {
                    eprintln!("Error stopping MLX streaming: {}", e);
                }
            }
        }

        // Wait for processing thread to finish
        if let Some(handle) = self.processing_thread.take() {
            let _ = handle.join();
        }

        self.audio_stream = None;
        self.state = RecordingState::Idle;
        println!("🛑 Stopped real-time transcription");

        // Optionally unload model based on config
        if !self.config.model.keep_loaded {
            self.unload_model();
        }

        cx.notify();
    }

    #[allow(dead_code)]
    fn unload_model(&mut self) {
        // Keep model loaded for better performance
        // Only unload if explicitly needed for memory management
        if self.mlx_model.is_some() {
            println!("🧹 Unloading MLX model to free RAM...");
            self.mlx_model = None;
            println!("✅ Model unloaded");
        }
    }
}

impl Render for Voicy {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_text = match self.state {
            RecordingState::Idle => "Click to start streaming",
            RecordingState::Recording => "🔴 Streaming... (click to stop)",
            RecordingState::Error => "❌ Error occurred",
        };

        let bg_color = match self.state {
            RecordingState::Recording => rgb(0xdc2626), // Red when streaming
            RecordingState::Error => rgb(0x991b1b),     // Dark red for errors
            _ => rgb(0x1f2937),                         // Dark gray when idle
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
            .border_color(match self.state {
                RecordingState::Recording => rgb(0xef4444),
                RecordingState::Error => rgb(0xb91c1c),
                _ => rgb(0x374151),
            })
            .text_xs()
            .text_color(rgb(0xffffff))
            .child(status_text)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.toggle_recording(cx);
                }),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let config = Config::load().unwrap_or_default();
        let window_size = size(px(config.ui.window_width), px(config.ui.window_height));
        let gap_from_bottom = px(config.ui.gap_from_bottom);

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

        cx.open_window(
            WindowOptions {
                is_movable: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(screen.id()),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Voicy::new()),
        )
        .unwrap();
    });
}
