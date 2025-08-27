use crate::config::Config;
use crate::error::{VoicyError, VoicyResult};
use anyhow::Result;
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

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub full_text: String,
    pub tokens: Vec<Token>,
    pub draft_token_count: usize,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub start: f32,
    pub end: f32,
}

pub struct AudioStream {
    consumer: Arc<Mutex<HeapCons<f32>>>,
    sample_rate: u32,
    is_playing: Arc<Mutex<bool>>,
}

impl AudioStream {
    pub fn new(target_sample_rate: u32) -> VoicyResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| VoicyError::AudioInitFailed("No input device available".to_string()))?;

        let supported_config = device.default_input_config()
            .map_err(|e| VoicyError::AudioInitFailed(format!("Failed to get device config: {}", e)))?;
        
        let device_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        
        println!("📊 Audio device: {} Hz, {} channels → {} Hz", 
                 device_sample_rate, channels, target_sample_rate);

        let ring_buffer_size = target_sample_rate as usize * 10;
        let rb = HeapRb::<f32>::new(ring_buffer_size);
        let (mut producer, consumer) = rb.split();

        let config = supported_config.into();
        let is_playing = Arc::new(Mutex::new(false));
        let is_playing_clone = is_playing.clone();
        
        let resample_ratio = target_sample_rate as f64 / device_sample_rate as f64;
        let channels_usize = channels as usize;
        
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        
        let chunk_size = 1024;
        let mut resampler = SincFixedIn::<f32>::new(
            resample_ratio, 2.0, params, chunk_size, 1
        ).map_err(|e| VoicyError::AudioInitFailed(format!("Failed to create resampler: {}", e)))?;
        
        let mut input_buffer = Vec::new();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if *is_playing_clone.lock().unwrap() {
                    let mono_data: Vec<f32> = if channels_usize > 1 {
                        data.chunks(channels_usize)
                            .map(|frame| frame.iter().sum::<f32>() / channels_usize as f32)
                            .collect()
                    } else {
                        data.to_vec()
                    };
                    
                    input_buffer.extend(mono_data);
                    
                    while input_buffer.len() >= chunk_size {
                        let input_chunk: Vec<f32> = input_buffer.drain(..chunk_size).collect();
                        
                        if let Ok(resampled) = resampler.process(&[input_chunk], None) {
                            for sample in &resampled[0] {
                                if producer.try_push(*sample).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    
                    if input_buffer.len() > device_sample_rate as usize {
                        input_buffer.clear();
                    }
                }
            },
            |err| eprintln!("❌ Audio error: {}", err),
            None,
        ).map_err(|e| VoicyError::AudioInitFailed(format!("Failed to build stream: {}", e)))?;

        stream.play().map_err(|e| VoicyError::AudioInitFailed(format!("Failed to start stream: {}", e)))?;
        Box::leak(Box::new(stream));
        
        Ok(Self {
            consumer: Arc::new(Mutex::new(consumer)),
            sample_rate: target_sample_rate,
            is_playing,
        })
    }

    pub fn start(&self) -> VoicyResult<()> {
        *self.is_playing.lock().unwrap() = true;
        println!("🎤 Audio stream started");
        Ok(())
    }

    pub fn read_chunk(&self, chunk_size: usize) -> Vec<f32> {
        let mut consumer = self.consumer.lock().unwrap();
        let mut chunk = Vec::with_capacity(chunk_size);

        while chunk.len() < chunk_size {
            if let Some(sample) = consumer.try_pop() {
                chunk.push(sample);
            } else {
                break;
            }
        }
        
        chunk
    }

    pub fn stop(&self) {
        *self.is_playing.lock().unwrap() = false;
        println!("🎤 Audio stream stopped");
    }
}

impl Clone for AudioStream {
    fn clone(&self) -> Self {
        Self {
            consumer: Arc::clone(&self.consumer),
            sample_rate: self.sample_rate,
            is_playing: Arc::clone(&self.is_playing),
        }
    }
}

#[derive(Clone)]
pub struct MLXModel {
    model: Arc<Mutex<Py<PyAny>>>,
    transcriber: Arc<Mutex<Option<Py<PyAny>>>>,
    sample_rate: u32,
    last_finalized_count: Arc<Mutex<usize>>,
}

impl MLXModel {
    pub fn new() -> VoicyResult<Self> {
        Python::with_gil(|py| {
            let parakeet_mlx = py.import("parakeet_mlx")?;
            let numpy = py.import("numpy")?;

            py.import("builtins")?.setattr("np", numpy)?;

            println!("🚀 Loading MLX Parakeet model...");
            let model = parakeet_mlx
                .getattr("from_pretrained")?
                .call1(("mlx-community/parakeet-tdt-0.6b-v2",))?;

            let preprocessor_config = model.getattr("preprocessor_config")?;
            let sample_rate: u32 = preprocessor_config.getattr("sample_rate")?.extract()?;

            println!("✅ Model loaded! Sample rate: {} Hz", sample_rate);

            Ok(Self {
                model: Arc::new(Mutex::new(model.into())),
                transcriber: Arc::new(Mutex::new(None)),
                sample_rate,
                last_finalized_count: Arc::new(Mutex::new(0)),
            })
        }).map_err(|e: pyo3::PyErr| VoicyError::ModelLoadFailed(format!("Python error: {}", e)))
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn start_streaming(&self, left_context: usize, right_context: usize) -> VoicyResult<()> {
        Python::with_gil(|py| {
            let model = self.model.lock().unwrap();
            let model_ref = model.bind(py);

            let kwargs = PyDict::new(py);
            kwargs.set_item("context_size", (left_context, right_context))?;

            let transcriber = model_ref
                .getattr("transcribe_stream")?
                .call((), Some(&kwargs))?
                .call_method0("__enter__")?;

            *self.transcriber.lock().unwrap() = Some(transcriber.unbind());
            *self.last_finalized_count.lock().unwrap() = 0;
            
            println!("🎙️ MLX streaming started with context: ({}, {})", left_context, right_context);
            Ok(())
        }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Failed to start streaming: {}", e)))
    }

    pub fn process_audio_chunk(&self, mut audio_data: Vec<f32>) -> VoicyResult<TranscriptionResult> {
        let result = Python::with_gil(|py| -> Result<TranscriptionResult, VoicyError> {
            let transcriber_lock = self.transcriber.lock().unwrap();

            if let Some(ref transcriber) = *transcriber_lock {
                let transcriber_ref = transcriber.bind(py);
                let numpy = py.import("numpy").map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to import numpy: {}", e)))?;
                let mlx = py.import("mlx.core").map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to import mlx.core: {}", e)))?;

                let max_amp = audio_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                
                if max_amp > 1.5 {
                    let scale = 0.99 / max_amp;
                    for sample in &mut audio_data {
                        *sample *= scale;
                    }
                }

                let numpy_array = numpy
                    .getattr("array").map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to get array: {}", e)))?
                    .call1((audio_data,)).map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to call array: {}", e)))?
                    .call_method1("astype", ("float32",)).map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to set dtype: {}", e)))?;

                let audio_array = mlx.getattr("array").map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to get mlx array: {}", e)))?.call1((numpy_array,)).map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to create mlx array: {}", e)))?;
                transcriber_ref.call_method1("add_audio", (audio_array,)).map_err(|e| VoicyError::TranscriptionFailed(format!("Failed to add audio: {}", e)))?;
                
                let mut new_text = String::new();
                let mut full_text = String::new();
                let mut tokens = Vec::new();
                
                if let Ok(finalized) = transcriber_ref.getattr("finalized_tokens") {
                    if let Ok(token_list) = finalized.extract::<Vec<Py<PyAny>>>() {
                        let mut last_count = self.last_finalized_count.lock().unwrap();
                        let current_count = token_list.len();
                        
                        for token_obj in token_list.iter() {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(text) = token_text.extract::<String>() {
                                    full_text.push_str(&text);
                                }
                            }
                        }
                        
                        for token_obj in token_list.iter().skip(*last_count) {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(text) = token_text.extract::<String>() {
                                    new_text.push_str(&text);
                                    
                                    let start = token_bound.getattr("start").and_then(|s| s.extract::<f32>()).unwrap_or(0.0);
                                    let end = token_bound.getattr("end").and_then(|e| e.extract::<f32>()).unwrap_or(0.0);

                                    tokens.push(Token { text: text.clone(), start, end });
                                }
                            }
                        }
                        
                        *last_count = current_count;
                    }
                }

                let draft_count = transcriber_ref
                    .getattr("draft_tokens")
                    .and_then(|d| d.call_method0("__len__"))
                    .and_then(|l| l.extract::<usize>())
                    .unwrap_or(0);

                Ok(TranscriptionResult {
                    text: new_text,
                    full_text,
                    tokens,
                    draft_token_count: draft_count,
                })
            } else {
                Err(VoicyError::TranscriptionFailed("Streaming not started".to_string()))
            }
        });
        result
    }

    pub fn stop_streaming(&self) -> VoicyResult<String> {
        Python::with_gil(|py| {
            let mut transcriber_lock = self.transcriber.lock().unwrap();

            if let Some(transcriber) = transcriber_lock.take() {
                let transcriber_ref = transcriber.bind(py);
                let mut final_new_text = String::new();
                
                if let Ok(finalized) = transcriber_ref.getattr("finalized_tokens") {
                    if let Ok(token_list) = finalized.extract::<Vec<Py<PyAny>>>() {
                        let last_count = *self.last_finalized_count.lock().unwrap();
                        
                        for token_obj in token_list.iter().skip(last_count) {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(text) = token_text.extract::<String>() {
                                    final_new_text.push_str(&text);
                                }
                            }
                        }
                    }
                }

                let py_none = py.None();
                let none_ref = py_none.bind(py);
                transcriber_ref.call_method1("__exit__", (none_ref, none_ref, none_ref))?;
                
                *self.last_finalized_count.lock().unwrap() = 0;
                println!("🛑 MLX streaming stopped");
                Ok(final_new_text)
            } else {
                Ok(String::new())
            }
        }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Failed to stop streaming: {}", e)))
    }
}

pub struct AudioProcessor {
    config: Config,
    audio_stream: Option<AudioStream>,
    mlx_model: Option<MLXModel>,
    processing_thread: Option<thread::JoinHandle<()>>,
    stop_signal: Option<Sender<()>>,
    transcription_receiver: Option<Receiver<String>>,
}

impl AudioProcessor {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            audio_stream: None,
            mlx_model: None,
            processing_thread: None,
            stop_signal: None,
            transcription_receiver: None,
        }
    }

    pub fn initialize(&mut self) -> VoicyResult<()> {
        if self.mlx_model.is_none() {
            println!("🚀 Attempting to load MLX model...");
            
            match MLXModel::new() {
                Ok(model) => {
                    let sample_rate = model.get_sample_rate();
                    println!("✅ MLX model loaded successfully");
                    
                    match AudioStream::new(sample_rate) {
                        Ok(audio_stream) => {
                            println!("✅ Audio stream created");
                            self.mlx_model = Some(model);
                            self.audio_stream = Some(audio_stream);
                        }
                        Err(e) => {
                            println!("❌ Audio stream failed: {}", e);
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ MLX model loading failed: {}", e);
                    println!("📝 This is expected if Python dependencies aren't installed");
                    println!("🔄 Running in demo mode - will simulate transcription");
                    // For now, return error - we can add demo mode later if needed
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }

    pub fn start_recording(&mut self) -> VoicyResult<()> {
        // Try to initialize if not already done (fallback)
        if self.mlx_model.is_none() {
            println!("🔄 Audio system not initialized, attempting now...");
            self.initialize()?;
        }
        
        if let (Some(stream), Some(model)) = (&self.audio_stream, &self.mlx_model) {
            stream.start()?;
            
            let left_context = self.config.model.left_context_seconds;
            let right_context = self.config.model.right_context_seconds;
            model.start_streaming(left_context, right_context)?;
            
            // Start background processing thread
            self.start_processing_loop()?;
            
            println!("🎙️ Recording started successfully");
        } else {
            return Err(VoicyError::AudioInitFailed("Audio system not properly initialized".to_string()));
        }
        
        Ok(())
    }

    fn start_processing_loop(&mut self) -> VoicyResult<()> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (transcription_tx, transcription_rx) = mpsc::channel();
        
        let stream = self.audio_stream.as_ref().unwrap().clone();
        let model = self.mlx_model.as_ref().unwrap().clone();
        
        let processing_thread = thread::spawn(move || {
            let chunk_size = 1024; // Process in small chunks
            
            loop {
                // Check for stop signal
                if stop_rx.try_recv().is_ok() {
                    println!("🛑 Processing loop stopping...");
                    break;
                }
                
                // Read audio chunk
                let audio_chunk = stream.read_chunk(chunk_size);
                
                if !audio_chunk.is_empty() {
                    // Process audio chunk and get transcription
                    match model.process_audio_chunk(audio_chunk) {
                        Ok(result) => {
                            if !result.text.is_empty() {
                                println!("💬 Partial transcription: '{}'", result.text);
                                let _ = transcription_tx.send(result.text);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Audio processing error: {}", e);
                        }
                    }
                }
                
                // Small delay to prevent overwhelming the system
                thread::sleep(Duration::from_millis(50));
            }
        });
        
        self.processing_thread = Some(processing_thread);
        self.stop_signal = Some(stop_tx);
        self.transcription_receiver = Some(transcription_rx);
        
        Ok(())
    }

    pub fn stop_recording(&mut self) -> VoicyResult<String> {
        let mut accumulated_text = String::new();
        
        // Stop the processing thread first
        if let Some(stop_signal) = self.stop_signal.take() {
            let _ = stop_signal.send(());
        }
        
        // Collect any remaining transcriptions from the processing loop
        if let Some(receiver) = &self.transcription_receiver {
            while let Ok(text) = receiver.try_recv() {
                accumulated_text.push_str(&text);
            }
        }
        
        // Wait for processing thread to finish
        if let Some(thread) = self.processing_thread.take() {
            let _ = thread.join();
        }
        
        if let Some(stream) = &self.audio_stream {
            stream.stop();
        }
        
        if let Some(model) = &self.mlx_model {
            let final_text = model.stop_streaming()?;
            accumulated_text.push_str(&final_text);
            println!("✅ Recording stopped");
        }
        
        // Clean up
        self.transcription_receiver = None;
        
        Ok(accumulated_text)
    }

    pub fn get_live_transcription(&self) -> Option<String> {
        if let Some(receiver) = &self.transcription_receiver {
            receiver.try_recv().ok()
        } else {
            None
        }
    }

    pub fn process_chunk(&self, chunk_size: usize) -> VoicyResult<Option<TranscriptionResult>> {
        if let (Some(stream), Some(model)) = (&self.audio_stream, &self.mlx_model) {
            let audio_chunk = stream.read_chunk(chunk_size);
            
            if !audio_chunk.is_empty() {
                let result = model.process_audio_chunk(audio_chunk)?;
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
}