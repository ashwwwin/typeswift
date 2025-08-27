mod audio_stream;
mod mlx;

use audio_stream::AudioStream;
use enigo::{Enigo, Keyboard, Settings};
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, rgb, size,
};
use mlx::MLXParakeet;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

struct Voicy {
    audio_stream: Option<AudioStream>,
    mlx_model: Option<MLXParakeet>,
    state: RecordingState,
    transcription_text: Arc<Mutex<String>>,
    processing_thread: Option<thread::JoinHandle<()>>,
    should_stop: Arc<Mutex<bool>>,
    enable_typing: Arc<Mutex<bool>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RecordingState {
    Idle,
    Recording,
    Processing,
    Error,
}

impl Voicy {
    fn new() -> Self {
        // Don't load model on startup - wait until needed
        Self {
            audio_stream: None,
            mlx_model: None, // Start with no model loaded
            state: RecordingState::Idle,
            transcription_text: Arc::new(Mutex::new(String::new())),
            processing_thread: None,
            should_stop: Arc::new(Mutex::new(false)),
            enable_typing: Arc::new(Mutex::new(true)), // Enable typing by default
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

                    // Start MLX streaming with context windows
                    let left_context = 5; // 5 seconds of left context
                    let right_context = 3; // 3 seconds of right context

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
                    let enable_typing = self.enable_typing.clone();
                    let stream_clone = stream.clone();
                    let mlx_model_clone = mlx_model.clone();

                    // Start processing thread for continuous transcription
                    let handle = thread::spawn(move || {
                        Self::audio_processing_loop(
                            stream_clone,
                            mlx_model_clone,
                            transcription_text,
                            should_stop,
                            enable_typing,
                            sample_rate,
                        );
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

        // Stop MLX streaming and get final transcription
        if let Some(ref mlx_model) = self.mlx_model {
            match mlx_model.stop_streaming() {
                Ok(final_text) => {
                    if !final_text.is_empty() {
                        println!("\n📝 Final Transcription:\n{}", final_text);

                        // Final text is already typed during processing

                        *self.transcription_text.lock().unwrap() = final_text;
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

        // Optional: Unload model after use to free RAM
        // Uncomment if you want aggressive memory management
        self.unload_model();

        cx.notify();
    }

    fn unload_model(&mut self) {
        if self.mlx_model.is_some() {
            println!("🧹 Unloading MLX model to free RAM...");
            self.mlx_model = None;
            // Python GC should collect it
            println!("✅ Model unloaded");
        }
    }
    fn audio_processing_loop(
        stream: AudioStream,
        mlx_model: MLXParakeet,
        transcription_text: Arc<Mutex<String>>,
        should_stop: Arc<Mutex<bool>>,
        enable_typing: Arc<Mutex<bool>>,
        sample_rate: u32,
    ) {
        // Calculate chunk size for processing (e.g., 0.5 seconds of audio)
        let chunk_duration_ms = 500;
        let chunk_size = (sample_rate as usize * chunk_duration_ms) / 1000;

        // Initialize Enigo for keyboard control
        let mut enigo = Enigo::new(&Settings::default()).unwrap();

        println!("📊 Audio processing configuration:");
        println!("  - Sample rate: {} Hz", sample_rate);
        println!(
            "  - Chunk size: {} samples ({} ms)",
            chunk_size, chunk_duration_ms
        );

        let mut accumulated_audio = Vec::new();

        // Buffer for continuous speech segments
        let mut speech_buffer: Vec<f32> = Vec::new();
        let mut in_speech = false;
        let mut silence_count = 0;
        let mut last_transcription = String::new();

        loop {
            // Check if we should stop
            if *should_stop.lock().unwrap() {
                break;
            }

            // Read whatever is available from the stream
            let available = stream.read_chunk(chunk_size);

            if !available.is_empty() {
                accumulated_audio.extend(available);
            }

            // Process when we have accumulated enough audio
            if accumulated_audio.len() >= chunk_size {
                // Take exactly chunk_size samples for processing
                let audio_chunk: Vec<f32> = accumulated_audio.drain(..chunk_size).collect();

                // Simple and effective VAD
                let rms = (audio_chunk.iter().map(|&x| x * x).sum::<f32>()
                    / audio_chunk.len() as f32)
                    .sqrt();

                // Balanced threshold for speech detection
                // Too low = noise, too high = misses speech
                let is_speech = rms > 0.003; // Balanced threshold

                if is_speech {
                    if !in_speech {
                        // Starting new speech segment
                        println!("\n🎤 Listening... (RMS: {:.4})", rms);
                        in_speech = true;
                        speech_buffer.clear();
                        silence_count = 0;
                    }

                    // Add audio to buffer
                    speech_buffer.extend(&audio_chunk);

                    // Log audio characteristics every second
                    if speech_buffer.len() % sample_rate as usize == 0 {
                        let max: f32 = speech_buffer
                            .iter()
                            .map(|&x| x.abs())
                            .fold(0.0f32, f32::max);
                        let mean: f32 =
                            speech_buffer.iter().sum::<f32>() / speech_buffer.len() as f32;
                        println!(
                            "  📊 Buffer: {} samples, max: {:.4}, mean: {:.4}, DC: {:.4}",
                            speech_buffer.len(),
                            max,
                            mean.abs(),
                            mean
                        );
                    }

                    silence_count = 0;
                } else if in_speech {
                    // We're in speech but hit silence
                    silence_count += 1;

                    // Still add to buffer in case it's a brief pause
                    speech_buffer.extend(&audio_chunk);

                    // After 1.5 seconds of silence (3 chunks), process the utterance
                    // This helps capture complete thoughts
                    // Also require minimum audio length (0.5 seconds) to avoid noise
                    let min_samples = (sample_rate as usize * 500) / 1000; // 0.5 seconds
                    if silence_count >= 3 && speech_buffer.len() >= min_samples {
                        // Analyze and clean the audio before sending
                        let mean = speech_buffer.iter().sum::<f32>() / speech_buffer.len() as f32;
                        
                        // Remove DC offset if detected
                        let mut audio_to_process = if mean.abs() > 0.01 {
                            println!("  ⚡ Removing DC offset (mean = {:.4})", mean);
                            speech_buffer.iter().map(|&x| x - mean).collect()
                        } else {
                            speech_buffer.clone()
                        };
                        
                        let max = audio_to_process
                            .iter()
                            .map(|&x| x.abs())
                            .fold(0.0f32, f32::max);
                        
                        // Always normalize audio to full scale for model
                        if max > 0.01 {
                            // Target 0.95 amplitude (nearly full scale)
                            let target_amplitude = 0.95;
                            let scale = target_amplitude / max;
                            println!("  ⚡ Normalizing audio (max: {:.4} -> {:.2})", max, target_amplitude);
                            audio_to_process.iter_mut().for_each(|s| *s *= scale);
                        } else {
                            println!("  ⚠️  Audio too quiet to process (max: {:.4})", max);
                        }
                        
                        let duration_sec = audio_to_process.len() as f32 / sample_rate as f32;

                        println!(
                            "  📦 Processing: {:.1}s, {} samples, max: {:.4}",
                            duration_sec,
                            audio_to_process.len(),
                            max
                        );

                        println!(
                            "  📊 Sending: {} samples ({:.2}s) of processed audio",
                            audio_to_process.len(),
                            audio_to_process.len() as f32 / sample_rate as f32
                        );

                        // Warn about quiet audio
                        if max < 0.01 {
                            println!("  ⚠️  WARNING: Very quiet audio (max < 0.01)");
                        }

                        // Process the complete utterance
                        match mlx_model.process_audio_chunk(audio_to_process) {
                            Ok(result) => {
                                println!(
                                    "  🔍 Result: text='{}', tokens={}",
                                    result.text,
                                    result.tokens.len()
                                );

                                if !result.text.is_empty() {
                                    // Clean the transcription text
                                    let cleaned_text = result.text.trim();
                                    
                                    if !cleaned_text.is_empty() {
                                        println!("  📝 Transcription: {}", cleaned_text);
                                        
                                        // Type the text if typing is enabled
                                        if *enable_typing.lock().unwrap() {
                                            // Add a space between utterances for natural flow
                                            if !last_transcription.is_empty() {
                                                enigo.text(" ").unwrap();
                                            }
                                            
                                            println!("  ✅ Typing: {}", cleaned_text);
                                            if let Err(e) = enigo.text(cleaned_text) {
                                                eprintln!("  ❌ Failed to type text: {}", e);
                                            }
                                        }
                                        
                                        last_transcription = cleaned_text.to_string();
                                        *transcription_text.lock().unwrap() = cleaned_text.to_string();
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("❌ Error: {}", e);
                            }
                        }

                        // Reset for next utterance
                        in_speech = false;
                        speech_buffer.clear();
                        silence_count = 0;
                        
                    } else if silence_count >= 3 {
                        // Too short, discard
                        println!("  🚫 Discarding short audio segment ({} samples)", speech_buffer.len());
                        in_speech = false;
                        speech_buffer.clear();
                        silence_count = 0;
                    }
                }
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(Duration::from_millis(50));
        }

        println!("\n✅ Processing complete");
    }
}

impl Render for Voicy {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_text = match self.state {
            RecordingState::Idle => "Click to start streaming",
            RecordingState::Recording => "🔴 Streaming... (click to stop)",
            RecordingState::Processing => "Processing...",
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
        let window_size = size(px(90.), px(39.0));
        let gap_from_bottom = px(70.);

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
