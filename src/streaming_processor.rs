use crate::audio_stream::AudioStream;
use crate::config::Config;
use crate::mlx::MLXParakeet;
use enigo::{Enigo, Keyboard, Settings};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct StreamingProcessor {
    config: Config,
    audio_buffer: Vec<f32>,  // Accumulate all audio linearly
    last_processed_position: usize,  // Track what we've sent to MLX
    process_timer: Instant,
    typed_so_far: String,  // Track everything we've typed
    enigo: Enigo,
}

impl StreamingProcessor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            audio_buffer: Vec::new(),
            last_processed_position: 0,
            process_timer: Instant::now(),
            typed_so_far: String::new(),
            enigo: Enigo::new(&Settings::default()).unwrap(),
        }
    }
    
    pub fn process_loop(
        mut self,
        stream: AudioStream,
        mlx_model: MLXParakeet,
        transcription_text: Arc<Mutex<String>>,
        should_stop: Arc<Mutex<bool>>,
    ) {
        let sample_rate = self.config.audio.target_sample_rate;
        let process_interval = Duration::from_millis(self.config.streaming.process_interval_ms as u64);
        let max_buffer_samples = (self.config.streaming.rolling_buffer_seconds * sample_rate as f32) as usize;
        
        // Read size: moderate chunks for stable capture
        let read_chunk_size = (sample_rate as usize * 50) / 1000; // 50ms chunks
        
        // Wait for reasonable initial audio before starting
        let min_initial_samples = (self.config.streaming.min_initial_audio_ms * sample_rate / 1000) as usize;
        
        if self.config.output.console_logging {
            println!("🚀 Starting real-time streaming mode:");
            println!("  - Process interval: {}ms", self.config.streaming.process_interval_ms);
            println!("  - Rolling buffer: {}s", self.config.streaming.rolling_buffer_seconds);
            println!("  - Min initial audio: {}ms", self.config.streaming.min_initial_audio_ms);
        }
        
        let mut audio_started = false;
        let mut accumulated_new_text = String::new();
        
        loop {
            // Check if we should stop
            if *should_stop.lock().unwrap() {
                break;
            }
            
            // Read audio in small chunks for responsiveness
            let new_audio = stream.read_chunk(read_chunk_size);
            
            if !new_audio.is_empty() {
                // Accumulate audio linearly
                self.audio_buffer.extend(&new_audio);
                
                // Limit total buffer size to prevent memory issues
                if self.audio_buffer.len() > max_buffer_samples * 2 {
                    // Keep only the last max_buffer_samples
                    let start = self.audio_buffer.len() - max_buffer_samples;
                    self.audio_buffer.drain(..start);
                    // Adjust position tracker
                    if self.last_processed_position > start {
                        self.last_processed_position -= start;
                    } else {
                        self.last_processed_position = 0;
                    }
                }
                
                // Check if we should process
                // Process frequently to maintain continuous context
                let new_audio_available = self.audio_buffer.len() - self.last_processed_position;
                
                let should_process = self.process_timer.elapsed() >= process_interval 
                    && self.audio_buffer.len() >= min_initial_samples
                    && new_audio_available > 0;  // Process ANY new audio to maintain context
                
                if should_process {
                    if !audio_started {
                        audio_started = true;
                        if self.config.output.console_logging {
                            println!("\n🎤 Audio stream active, processing in real-time...");
                        }
                    }
                    
                    // Get ONLY truly NEW audio since last processing
                    // MLX maintains context internally - we must not send overlapping audio!
                    let new_audio_chunk: Vec<f32> = self.audio_buffer[self.last_processed_position..].to_vec();
                    
                    // Calculate RMS for activity indication from recent audio
                    let rms = if !new_audio_chunk.is_empty() {
                        (new_audio_chunk.iter().map(|&x| x * x).sum::<f32>() / new_audio_chunk.len() as f32).sqrt()
                    } else {
                        0.0
                    };
                    
                    // Process if we have new audio (even silence, for context)
                    // Lower the threshold to ensure continuous context
                    if !new_audio_chunk.is_empty() {
                        if self.config.output.console_logging && rms > 0.001 {
                            println!("  🔊 Processing {} new samples (RMS: {:.4})", 
                                new_audio_chunk.len(), rms);
                        }
                        
                        // Update position BEFORE processing
                        self.last_processed_position = self.audio_buffer.len();
                        
                        // Apply consistent preprocessing to ALL audio (not just loud parts)
                        let processed_audio = if self.config.vad.enable_normalization {
                            self.normalize_audio(new_audio_chunk)
                        } else {
                            new_audio_chunk
                        };
                        
                        // Send ONLY NEW audio to MLX (it maintains context internally)
                        match mlx_model.process_audio_chunk(processed_audio) {
                            Ok(result) => {
                                // SIMPLE APPROACH: Just type truly NEW text
                                // Use full_text to know what's been transcribed total
                                // Only type the part we haven't typed yet
                                
                                if !result.full_text.is_empty() && self.config.output.enable_typing {
                                    let full_transcription = result.full_text.trim();
                                    
                                    // Only type what we haven't typed yet
                                    if full_transcription.len() > self.typed_so_far.len() {
                                        // Get only the NEW portion
                                        let new_portion = &full_transcription[self.typed_so_far.len()..];
                                        
                                        if !new_portion.is_empty() {
                                            if self.config.output.console_logging {
                                                println!("  💬 Typing: \"{}\"", new_portion);
                                            }
                                            
                                            // Type it
                                            if let Err(e) = self.enigo.text(new_portion) {
                                                if self.config.output.console_logging {
                                                    eprintln!("  ❌ Failed to type: {}", e);
                                                }
                                            } else {
                                                // Update what we've typed
                                                self.typed_so_far = full_transcription.to_string();
                                            }
                                        }
                                    }
                                    
                                    // Update shared state
                                    *transcription_text.lock().unwrap() = full_transcription.to_string();
                                    
                                } else if !result.text.is_empty() && self.config.output.enable_typing {
                                    // Fallback when no full_text: just type incremental text
                                    let new_text = result.text.trim();
                                    
                                    if !new_text.is_empty() {
                                        if self.config.output.console_logging {
                                            println!("  💬 Typing: \"{}\"", new_text);
                                        }
                                        
                                        // Natural spacing
                                        if !self.typed_so_far.is_empty() && !self.typed_so_far.ends_with(' ') && !new_text.starts_with(' ') {
                                            let _ = self.enigo.text(" ");
                                            self.typed_so_far.push(' ');
                                        }
                                        
                                        if let Err(e) = self.enigo.text(new_text) {
                                            if self.config.output.console_logging {
                                                eprintln!("  ❌ Failed to type: {}", e);
                                            }
                                        } else {
                                            self.typed_so_far.push_str(new_text);
                                        }
                                        
                                        accumulated_new_text.push_str(new_text);
                                    }
                                }
                            }
                            Err(e) => {
                                if self.config.output.console_logging {
                                    eprintln!("  ⚠️  MLX error: {}", e);
                                }
                            }
                        }
                    }
                    
                    self.process_timer = Instant::now();
                }
            }
            
            // Small sleep to prevent busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
        
        // Save any remaining text
        if !accumulated_new_text.is_empty() {
            let mut text = transcription_text.lock().unwrap();
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&accumulated_new_text);
        }
        
        if self.config.output.console_logging {
            println!("\n✅ Streaming processing complete");
        }
    }
    
    fn normalize_audio(&self, mut audio: Vec<f32>) -> Vec<f32> {
        // Remove DC offset if present
        if self.config.vad.enable_dc_offset_removal {
            let mean = audio.iter().sum::<f32>() / audio.len() as f32;
            if mean.abs() > 0.01 {
                for sample in &mut audio {
                    *sample -= mean;
                }
            }
        }
        
        // Normalize to consistent amplitude
        let max = audio.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if max > 0.01 && max < 0.9 {
            let scale = 0.5 / max;  // Target 0.5 amplitude for streaming
            for sample in &mut audio {
                *sample *= scale;
            }
        }
        
        audio
    }
}

// VAD-based processor for comparison
pub fn vad_processing_loop(
    stream: AudioStream,
    mlx_model: MLXParakeet,
    transcription_text: Arc<Mutex<String>>,
    should_stop: Arc<Mutex<bool>>,
    config: Config,
    sample_rate: u32,
) {
    // This is the original VAD-based implementation
    // Moved here for clarity when streaming is disabled
    
    // Calculate chunk size for processing from config
    let chunk_duration_ms = config.audio.chunk_duration_ms;
    let chunk_size = (sample_rate as usize * chunk_duration_ms as usize) / 1000;

    // Initialize Enigo for keyboard control
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    if config.output.console_logging {
        println!("📊 VAD-based processing mode:");
        println!("  - Sample rate: {} Hz", sample_rate);
        println!("  - Chunk size: {} samples ({} ms)", chunk_size, chunk_duration_ms);
        println!("  - Silence threshold: {:.1}s", config.vad.silence_duration_ms as f32 / 1000.0);
    }

    let mut accumulated_audio = Vec::new();
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
            let rms = (audio_chunk.iter().map(|&x| x * x).sum::<f32>() / audio_chunk.len() as f32).sqrt();

            // Speech detection using configured threshold
            let is_speech = rms > config.vad.speech_threshold;

            if is_speech {
                if !in_speech {
                    // Starting new speech segment
                    if config.output.console_logging {
                        println!("\n🎤 Speech detected... (RMS: {:.4})", rms);
                    }
                    in_speech = true;
                    speech_buffer.clear();
                }

                // Add audio to buffer
                speech_buffer.extend(&audio_chunk);

                // Log audio characteristics every second
                if config.output.console_logging && speech_buffer.len() % sample_rate as usize == 0 {
                    let max: f32 = speech_buffer.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                    println!("  📊 Buffer: {} samples, max: {:.4}", speech_buffer.len(), max);
                }
            } else if in_speech {
                // We're in speech but hit silence
                silence_count += 1;

                // Still add to buffer in case it's a brief pause
                speech_buffer.extend(&audio_chunk);

                // Process utterance after configured silence duration
                let silence_chunks = config.vad.silence_duration_ms / chunk_duration_ms;
                let min_samples = (sample_rate as usize * config.vad.min_speech_duration_ms as usize) / 1000;
                
                if silence_count >= silence_chunks && speech_buffer.len() >= min_samples {
                    // Process the complete utterance
                    let audio_to_process = speech_buffer.clone();
                    
                    if config.output.console_logging {
                        println!("  📦 Processing {} samples ({:.1}s)", 
                            audio_to_process.len(),
                            audio_to_process.len() as f32 / sample_rate as f32);
                    }

                    match mlx_model.process_audio_chunk(audio_to_process) {
                        Ok(result) => {
                            if !result.text.is_empty() {
                                let cleaned_text = result.text.trim();
                                
                                if !cleaned_text.is_empty() {
                                    if config.output.console_logging {
                                        println!("  📝 Transcription: {}", cleaned_text);
                                    }
                                    
                                    // Type the text if typing is enabled
                                    if config.output.enable_typing {
                                        // Add space between utterances if configured
                                        if config.output.add_space_between_utterances && !last_transcription.is_empty() {
                                            enigo.text(" ").unwrap();
                                        }
                                        
                                        if let Err(e) = enigo.text(cleaned_text) {
                                            if config.output.console_logging {
                                                eprintln!("  ❌ Failed to type text: {}", e);
                                            }
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
                } else if silence_count >= silence_chunks {
                    // Too short, discard
                    if config.output.console_logging {
                        println!("  🚫 Discarding short audio ({} samples)", speech_buffer.len());
                    }
                    in_speech = false;
                    speech_buffer.clear();
                    silence_count = 0;
                }
            }
        }

        // Small sleep to avoid busy-waiting
        std::thread::sleep(Duration::from_millis(50));
    }

    if config.output.console_logging {
        println!("\n✅ VAD processing complete");
    }
}