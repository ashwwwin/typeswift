use crate::config::{Config, StreamingConfig, ModelConfig};
use crate::error::{VoicyError, VoicyResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::RwLock;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use ringbuf::{traits::*, HeapRb, HeapCons};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Manages audio capture with proper error recovery
pub struct AudioCapture {
    consumer: Arc<Mutex<HeapCons<f32>>>,
    is_recording: Arc<RwLock<bool>>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new(target_sample_rate: u32) -> VoicyResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| VoicyError::AudioInitFailed("No input device available".to_string()))?;

        let supported_config = device.default_input_config()
            .map_err(|e| VoicyError::AudioInitFailed(format!("Failed to get device config: {}", e)))?;
        
        let device_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;
        
        println!("📊 Audio device: {} Hz, {} channels → {} Hz", 
                 device_sample_rate, channels, target_sample_rate);

        // Create ring buffer with sufficient size
        let ring_buffer_size = target_sample_rate as usize * 30; // 30 seconds buffer
        let rb = HeapRb::<f32>::new(ring_buffer_size);
        let (mut producer, consumer) = rb.split();

        let config = supported_config.into();
        let is_recording = Arc::new(RwLock::new(false));
        let is_recording_clone = is_recording.clone();
        
        // Setup resampler if needed
        let needs_resampling = device_sample_rate != target_sample_rate;
        let resample_ratio = target_sample_rate as f64 / device_sample_rate as f64;
        
        let mut resampler = if needs_resampling {
            let params = SincInterpolationParameters {
                sinc_len: 128,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Linear,
                oversampling_factor: 128,
                window: WindowFunction::BlackmanHarris2,
            };
            
            Some(SincFixedIn::<f32>::new(
                resample_ratio, 2.0, params, 1024, 1
            ).map_err(|e| VoicyError::AudioInitFailed(format!("Failed to create resampler: {}", e)))?)
        } else {
            None
        };
        
        let mut input_buffer = Vec::with_capacity(1024);
        let mut overflow_count = 0usize;

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if !*is_recording_clone.read() {
                    return;
                }
                
                // Log periodically to confirm audio is flowing
                static SAMPLE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                let count = SAMPLE_COUNTER.fetch_add(data.len(), std::sync::atomic::Ordering::Relaxed);
                if count % 48000 == 0 {  // Log every second at 48kHz
                    println!("🎵 Audio stream active: {} total samples captured", count);
                }
                
                // Convert to mono
                let mono_data: Vec<f32> = if channels > 1 {
                    data.chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect()
                } else {
                    data.to_vec()
                };
                
                // Handle resampling if needed
                if let Some(ref mut resampler) = resampler {
                    input_buffer.extend(mono_data);
                    
                    while input_buffer.len() >= 1024 {
                        let input_chunk: Vec<f32> = input_buffer.drain(..1024).collect();
                        
                        if let Ok(resampled) = resampler.process(&[input_chunk], None) {
                            for sample in &resampled[0] {
                                if producer.try_push(*sample).is_err() {
                                    overflow_count += 1;
                                    if overflow_count % 1000 == 0 {
                                        eprintln!("⚠️ Audio buffer overflow: {} samples dropped", overflow_count);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // No resampling needed, direct copy
                    for sample in mono_data {
                        if producer.try_push(sample).is_err() {
                            overflow_count += 1;
                            if overflow_count % 1000 == 0 {
                                eprintln!("⚠️ Audio buffer overflow: {} samples dropped", overflow_count);
                            }
                        }
                    }
                }
            },
            |err| eprintln!("❌ Audio stream error: {}", err),
            None,
        ).map_err(|e| VoicyError::AudioInitFailed(format!("Failed to build stream: {}", e)))?;

        stream.play().map_err(|e| VoicyError::AudioInitFailed(format!("Failed to start stream: {}", e)))?;
        
        // Keep stream alive by leaking it - it will live for the duration of the program
        Box::leak(Box::new(stream));
        
        Ok(Self {
            consumer: Arc::new(Mutex::new(consumer)),
            is_recording,
            sample_rate: target_sample_rate,
        })
    }

    pub fn start_recording(&self) -> VoicyResult<()> {
        *self.is_recording.write() = true;
        println!("🎤 Audio capture started");
        Ok(())
    }

    pub fn stop_recording(&self) -> VoicyResult<()> {
        *self.is_recording.write() = false;
        println!("🎤 Audio capture stopped");
        Ok(())
    }

    pub fn read_audio(&self, max_samples: usize) -> Vec<f32> {
        let mut consumer = self.consumer.lock().unwrap();
        let mut samples = Vec::with_capacity(max_samples);
        
        while samples.len() < max_samples {
            if let Some(sample) = consumer.try_pop() {
                samples.push(sample);
            } else {
                break;
            }
        }
        
        if samples.len() > 0 && samples.len() % 1000 == 0 {
            println!("📊 Ring buffer read: {} samples", samples.len());
        }
        
        samples
    }
    
    pub fn is_recording(&self) -> bool {
        *self.is_recording.read()
    }
}

/// Handles transcription with proper error recovery
pub struct Transcriber {
    model: Arc<Mutex<Option<Py<PyAny>>>>,
    context: Arc<Mutex<Option<Py<PyAny>>>>,
    sample_rate: u32,
    model_config: ModelConfig,
    streaming_config: StreamingConfig,
}

impl Transcriber {
    pub fn new(model_config: ModelConfig, streaming_config: StreamingConfig) -> VoicyResult<Self> {
        // Try to load the model, but don't fail if Python isn't available
        let (model, sample_rate) = match Self::try_load_model(&model_config.model_name) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("⚠️ Failed to load MLX model: {}", e);
                eprintln!("   Running in demo mode - transcription will be simulated");
                (None, 16000)
            }
        };
        
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            context: Arc::new(Mutex::new(None)),
            sample_rate,
            model_config,
            streaming_config,
        })
    }
    
    fn try_load_model(model_name: &str) -> VoicyResult<(Option<Py<PyAny>>, u32)> {
        Python::with_gil(|py| {
            // Check if required modules are available
            if py.import("parakeet_mlx").is_err() {
                return Ok((None, 16000));
            }
            
            let parakeet_mlx = py.import("parakeet_mlx")?;
            let numpy = py.import("numpy")?;
            py.import("builtins")?.setattr("np", numpy)?;
            
            println!("🚀 Loading MLX Parakeet model: {}", model_name);
            let model = parakeet_mlx
                .getattr("from_pretrained")?
                .call1((model_name,))?;
            
            let preprocessor_config = model.getattr("preprocessor_config")?;
            let sample_rate: u32 = preprocessor_config.getattr("sample_rate")?.extract()?;
            
            println!("✅ Model loaded! Sample rate: {} Hz", sample_rate);
            
            Ok((Some(model.into()), sample_rate))
        }).map_err(|e: pyo3::PyErr| VoicyError::ModelLoadFailed(format!("Python error: {}", e)))
    }
    
    pub fn start_session(&self) -> VoicyResult<()> {
        let model = self.model.lock().unwrap();
        
        if let Some(ref model_py) = *model {
            Python::with_gil(|py| {
                let model_ref = model_py.bind(py);
                
                let kwargs = PyDict::new(py);
                // Use context sizes from config
                kwargs.set_item("context_size", (
                    self.model_config.left_context_seconds,
                    self.model_config.right_context_seconds
                ))?;
                
                let context = model_ref
                    .getattr("transcribe_stream")?
                    .call((), Some(&kwargs))?
                    .call_method0("__enter__")?;
                
                *self.context.lock().unwrap() = Some(context.unbind());
                
                println!("🎙️ Transcription session started");
                Ok(())
            }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Failed to start session: {}", e)))
        } else {
            // Demo mode
            println!("🎙️ Transcription session started (demo mode)");
            Ok(())
        }
    }
    
    pub fn process_audio(&self, audio: Vec<f32>) -> VoicyResult<String> {
        println!("🔬 Transcriber::process_audio called with {} samples", audio.len());
        let context = self.context.lock().unwrap();
        
        if let Some(ref context_py) = *context {
            println!("📝 Using real MLX model for transcription");
            Python::with_gil(|py| {
                let context_ref = context_py.bind(py);
                let numpy = py.import("numpy")?;
                let mlx = py.import("mlx.core")?;
                
                // Normalize audio
                let mut normalized = audio.clone();
                let max_amp = normalized.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                if max_amp > 1.5 {
                    let scale = 0.99 / max_amp;
                    for sample in &mut normalized {
                        *sample *= scale;
                    }
                }
                
                let numpy_array = numpy
                    .getattr("array")?
                    .call1((normalized,))?
                    .call_method1("astype", ("float32",))?;
                
                let audio_array = mlx.getattr("array")?.call1((numpy_array,))?;
                context_ref.call_method1("add_audio", (audio_array,))?;
                
                // Get transcribed text from both finalized and draft tokens
                let mut text = String::new();
                
                // Check finalized tokens
                if let Ok(finalized) = context_ref.getattr("finalized_tokens") {
                    if let Ok(token_list) = finalized.extract::<Vec<Py<PyAny>>>() {
                        println!("  📊 Found {} finalized tokens", token_list.len());
                        for token_obj in token_list {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(t) = token_text.extract::<String>() {
                                    text.push_str(&t);
                                }
                            }
                        }
                    }
                }
                
                // Also check draft tokens for live feedback
                if text.is_empty() {
                    if let Ok(draft) = context_ref.getattr("draft_tokens") {
                        if let Ok(token_list) = draft.extract::<Vec<Py<PyAny>>>() {
                            println!("  📊 Found {} draft tokens", token_list.len());
                            for token_obj in token_list {
                                let token_bound = token_obj.bind(py);
                                if let Ok(token_text) = token_bound.getattr("text") {
                                    if let Ok(t) = token_text.extract::<String>() {
                                        text.push_str(&t);
                                    }
                                }
                            }
                        }
                    }
                }
                
                if !text.is_empty() {
                    println!("  ✅ Transcribed text: '{}'", text);
                }
                
                Ok(text)
            }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Processing failed: {}", e)))
        } else {
            // Demo mode - simulate transcription
            println!("⚠️ No transcription context - running in demo mode");
            Ok(format!("[Demo transcription]"))
        }
    }
    
    pub fn end_session(&self) -> VoicyResult<String> {
        let mut context = self.context.lock().unwrap();
        
        if let Some(context_py) = context.take() {
            Python::with_gil(|py| {
                let context_ref = context_py.bind(py);
                
                // Get any final text
                let mut final_text = String::new();
                if let Ok(finalized) = context_ref.getattr("finalized_tokens") {
                    if let Ok(token_list) = finalized.extract::<Vec<Py<PyAny>>>() {
                        for token_obj in token_list {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(t) = token_text.extract::<String>() {
                                    final_text.push_str(&t);
                                }
                            }
                        }
                    }
                }
                
                // Clean up context
                let py_none = py.None();
                let none_ref = py_none.bind(py);
                context_ref.call_method1("__exit__", (none_ref, none_ref, none_ref))?;
                
                println!("🛑 Transcription session ended");
                Ok(final_text)
            }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Failed to end session: {}", e)))
        } else {
            println!("🛑 Transcription session ended (demo mode)");
            Ok(String::new())
        }
    }
    
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

/// High-level audio processor with proper separation of concerns
pub struct ImprovedAudioProcessor {
    config: Config,
    audio_capture: Option<AudioCapture>,
    transcriber: Option<Transcriber>,
    processing_handle: Option<thread::JoinHandle<()>>,
    stop_signal: Option<Sender<()>>,
    result_receiver: Option<Receiver<String>>,
}

impl ImprovedAudioProcessor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            audio_capture: None,
            transcriber: None,
            processing_handle: None,
            stop_signal: None,
            result_receiver: None,
        }
    }
    
    pub fn initialize(&mut self) -> VoicyResult<()> {
        // Initialize transcriber with config
        let transcriber = Transcriber::new(
            self.config.model.clone(),
            self.config.streaming.clone()
        )?;
        let target_sample_rate = transcriber.get_sample_rate();
        
        // Initialize audio capture with config sample rate
        let audio_capture = AudioCapture::new(self.config.audio.target_sample_rate)?;
        
        self.transcriber = Some(transcriber);
        self.audio_capture = Some(audio_capture);
        
        Ok(())
    }
    
    pub fn start_recording(&mut self) -> VoicyResult<()> {
        // Ensure initialized
        if self.audio_capture.is_none() || self.transcriber.is_none() {
            self.initialize()?;
        }
        
        // Start audio capture
        if let Some(ref capture) = self.audio_capture {
            capture.start_recording()?;
        }
        
        // Only start transcription session and processing thread if streaming is enabled
        if self.config.streaming.enabled {
            // Start transcription session for streaming
            if let Some(ref transcriber) = self.transcriber {
                transcriber.start_session()?;
            }
            
            // Start processing thread for real-time transcription
            self.start_processing_thread()?;
        }
        // If streaming is disabled, we'll just accumulate audio and process on stop
        
        Ok(())
    }
    
    fn start_processing_thread(&mut self) -> VoicyResult<()> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        
        let capture = self.audio_capture.as_ref().unwrap().clone();
        let transcriber = self.transcriber.as_ref().unwrap().clone();
        let process_interval = Duration::from_millis(self.config.streaming.process_interval_ms as u64);
        let min_audio_ms = self.config.streaming.min_initial_audio_ms;
        let sample_rate = self.config.audio.target_sample_rate;
        let chunk_samples = (self.config.audio.chunk_duration_ms * sample_rate / 1000) as usize;
        
        let handle = thread::spawn(move || {
            let mut accumulated_audio = Vec::new();
            let mut last_process = Instant::now();
            let mut total_audio_ms = 0u32;
            
            loop {
                // Check for stop signal
                if stop_rx.try_recv().is_ok() {
                    break;
                }
                
                // Read available audio based on config chunk size
                let audio = capture.read_audio(chunk_samples);
                if !audio.is_empty() {
                    accumulated_audio.extend(&audio);
                    total_audio_ms += (audio.len() as u32 * 1000) / sample_rate; // Convert samples to ms
                    if audio.len() > chunk_samples / 10 {  // Only log significant chunks
                        println!("🎤 Read {} audio samples, total accumulated: {} ({} ms)", 
                                 audio.len(), accumulated_audio.len(), total_audio_ms);
                    }
                }
                
                // Process based on config interval and minimum audio duration
                let should_process = last_process.elapsed() >= process_interval && 
                                     total_audio_ms >= min_audio_ms &&
                                     !accumulated_audio.is_empty();
                                     
                if should_process {
                    println!("🔊 Processing {} audio samples", accumulated_audio.len());
                    
                    // Send the accumulated audio for transcription
                    match transcriber.process_audio(accumulated_audio.clone()) {
                        Ok(text) => {
                            if !text.is_empty() {
                                println!("💬 Transcribed: '{}'", text);
                                let _ = result_tx.send(text);
                            } else {
                                println!("📝 No text from transcriber yet");
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Transcription error: {}", e);
                        }
                    }
                    
                    // Clear the accumulated buffer after processing
                    accumulated_audio.clear();
                    total_audio_ms = 0;
                    last_process = Instant::now();
                }
                
                thread::sleep(Duration::from_millis(50));
            }
        });
        
        self.processing_handle = Some(handle);
        self.stop_signal = Some(stop_tx);
        self.result_receiver = Some(result_rx);
        
        Ok(())
    }
    
    pub fn stop_recording(&mut self) -> VoicyResult<String> {
        if self.config.streaming.enabled {
            // Streaming mode: stop thread and collect accumulated text
            
            // Stop processing thread
            if let Some(stop) = self.stop_signal.take() {
                let _ = stop.send(());
            }
            
            // Wait for thread to finish
            if let Some(handle) = self.processing_handle.take() {
                let _ = handle.join();
            }
            
            // Stop audio capture
            if let Some(ref capture) = self.audio_capture {
                capture.stop_recording()?;
            }
            
            // End transcription session and get final text
            let final_text = if let Some(ref transcriber) = self.transcriber {
                transcriber.end_session()?
            } else {
                String::new()
            };
            
            // Collect any remaining results
            let mut all_text = String::new();
            if let Some(ref receiver) = self.result_receiver {
                while let Ok(text) = receiver.try_recv() {
                    all_text.push_str(&text);
                }
            }
            all_text.push_str(&final_text);
            
            self.result_receiver = None;
            
            Ok(all_text)
        } else {
            // Non-streaming mode: process all audio at once
            
            // Stop audio capture first
            if let Some(ref capture) = self.audio_capture {
                capture.stop_recording()?;
                
                // Read ALL accumulated audio
                let mut all_audio = Vec::new();
                loop {
                    let chunk = capture.read_audio(16000); // Read 1 second chunks
                    if chunk.is_empty() {
                        break;
                    }
                    all_audio.extend(chunk);
                }
                
                println!("🎯 Processing {} total audio samples at once", all_audio.len());
                
                // Process all audio in one go
                if !all_audio.is_empty() {
                    if let Some(ref transcriber) = self.transcriber {
                        // Start session, process, and end in one go
                        transcriber.start_session()?;
                        let text = transcriber.process_audio(all_audio)?;
                        let final_text = transcriber.end_session()?;
                        
                        let mut result = text;
                        result.push_str(&final_text);
                        return Ok(result);
                    }
                }
            }
            
            Ok(String::new())
        }
    }
    
    pub fn get_live_transcription(&self) -> Option<String> {
        self.result_receiver.as_ref()?.try_recv().ok()
    }
}

// Implement Clone for thread-safe sharing
impl Clone for AudioCapture {
    fn clone(&self) -> Self {
        Self {
            consumer: Arc::clone(&self.consumer),
            is_recording: Arc::clone(&self.is_recording),
            sample_rate: self.sample_rate,
        }
    }
}

impl Clone for Transcriber {
    fn clone(&self) -> Self {
        Self {
            model: Arc::clone(&self.model),
            context: Arc::clone(&self.context),
            sample_rate: self.sample_rate,
            model_config: self.model_config.clone(),
            streaming_config: self.streaming_config.clone(),
        }
    }
}