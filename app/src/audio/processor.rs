use crate::audio::{AudioCapture, Transcriber};
use crate::config::Config;
use crate::error::VoicyResult;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

/// Optimized audio processor with reduced allocations and lower latency
pub struct AudioProcessor {
    config: Config,
    audio_capture: Option<AudioCapture>,
    transcriber: Option<Transcriber>,
    processing_handle: Option<thread::JoinHandle<()>>,
    stop_signal: Option<Sender<()>>,
    result_receiver: Option<Receiver<String>>,
    // Pre-allocated buffers for better performance
    audio_buffer: Vec<f32>,
}

impl AudioProcessor {
    pub fn new(config: Config) -> Self {
        // Pre-allocate buffer for 30 seconds of audio at 16kHz
        let buffer_capacity = 16000 * 30;
        
        Self {
            config,
            audio_capture: None,
            transcriber: None,
            processing_handle: None,
            stop_signal: None,
            result_receiver: None,
            audio_buffer: Vec::with_capacity(buffer_capacity),
        }
    }
    
    pub fn initialize(&mut self) -> VoicyResult<()> {
        // Initialize transcriber with config
        let transcriber = Transcriber::new(
            self.config.model.clone(),
            self.config.streaming.clone()
        )?;
        let target_sample_rate = transcriber.get_sample_rate();
        
        // Initialize audio capture
        let audio_capture = AudioCapture::new(target_sample_rate)?;
        
        self.transcriber = Some(transcriber);
        self.audio_capture = Some(audio_capture);
        
        println!("✅ Audio processor initialized");
        Ok(())
    }
    
    pub fn start_recording(&mut self) -> VoicyResult<()> {
        // Ensure initialized
        if self.audio_capture.is_none() || self.transcriber.is_none() {
            self.initialize()?;
        }
        
        // Clear buffer for new recording
        self.audio_buffer.clear();
        
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
            
            // Start optimized processing thread
            self.start_optimized_processing_thread()?;
        }
        
        Ok(())
    }
    
    fn start_optimized_processing_thread(&mut self) -> VoicyResult<()> {
        let (stop_tx, stop_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        
        let capture = self.audio_capture.as_ref().unwrap().clone();
        let transcriber = self.transcriber.as_ref().unwrap().clone();
        let sample_rate = capture.get_sample_rate();
        
        // Optimized settings for lower latency
        let process_interval = Duration::from_millis(
            (self.config.streaming.process_interval_ms / 2) as u64 // Half the interval for faster feedback
        );
        let min_samples_for_processing = 
            (self.config.streaming.min_initial_audio_ms * sample_rate / 1000) as usize;
        
        // Larger chunk size for more efficient reading
        let read_chunk_size = (sample_rate / 10) as usize; // 100ms chunks
        
        let handle = thread::spawn(move || {
            // Pre-allocated buffers to avoid allocations in hot loop
            let mut accumulated_audio = Vec::with_capacity(sample_rate as usize * 10); // 10 seconds
            let mut processing_buffer = Vec::with_capacity(sample_rate as usize * 10);
            let mut last_process = Instant::now();
            let mut total_samples_processed = 0usize;
            
            loop {
                // Check for stop signal
                match stop_rx.try_recv() {
                    Ok(_) | Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => {}
                }
                
                // Read available audio more efficiently
                let audio = capture.read_audio(read_chunk_size);
                if !audio.is_empty() {
                    accumulated_audio.extend_from_slice(&audio);
                }
                
                // Process with lower latency - check if we have enough new audio
                let new_samples = accumulated_audio.len() - total_samples_processed;
                let should_process = 
                    new_samples >= min_samples_for_processing && 
                    last_process.elapsed() >= process_interval;
                                     
                if should_process && !accumulated_audio.is_empty() {
                    // Copy only the new samples to avoid re-processing
                    processing_buffer.clear();
                    processing_buffer.extend_from_slice(&accumulated_audio[total_samples_processed..]);
                    
                    // Process without cloning
                    match transcriber.process_audio(processing_buffer.clone()) {
                        Ok(text) => {
                            if !text.is_empty() {
                                println!("💬 Live: '{}'", text);
                                let _ = result_tx.send(text);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Transcription error: {}", e);
                        }
                    }
                    
                    total_samples_processed = accumulated_audio.len();
                    last_process = Instant::now();
                }
                
                // Shorter sleep for lower latency
                thread::sleep(Duration::from_millis(10));
            }
        });
        
        self.processing_handle = Some(handle);
        self.stop_signal = Some(stop_tx);
        self.result_receiver = Some(result_rx);
        
        Ok(())
    }
    
    pub fn stop_recording(&mut self) -> VoicyResult<String> {
        if self.config.streaming.enabled {
            // Streaming mode: just clean up, don't return text (it was already typed live)
            
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
            
            // End transcription session (cleanup only)
            if let Some(ref transcriber) = self.transcriber {
                let _ = transcriber.end_session()?;
            }
            
            // Drain any remaining results (but don't return them - they were already typed)
            if let Some(ref receiver) = self.result_receiver {
                while receiver.try_recv().is_ok() {
                    // Just drain the channel
                }
            }
            
            self.result_receiver = None;
            
            // Return empty string since everything was already typed live
            Ok(String::new())
        } else {
            // Non-streaming mode: optimized for single-shot processing
            
            // Stop audio capture first
            if let Some(ref capture) = self.audio_capture {
                capture.stop_recording()?;
                
                // Efficiently drain ALL audio from buffer
                self.audio_buffer.clear();
                loop {
                    // Use larger chunks for efficiency
                    let chunk = capture.read_audio(8000); // 0.5 second chunks
                    if chunk.is_empty() {
                        break;
                    }
                    self.audio_buffer.extend_from_slice(&chunk);
                }
                
                if !self.audio_buffer.is_empty() {
                    println!("🎯 Processing {} samples ({}s @ 16kHz)", 
                             self.audio_buffer.len(), 
                             self.audio_buffer.len() / 16000);
                    
                    if let Some(ref transcriber) = self.transcriber {
                        // Process in single session
                        transcriber.start_session()?;
                        
                        // Pass reference to avoid clone if possible
                        // Note: We need to clone here due to Python API requirements
                        let _ = transcriber.process_audio(self.audio_buffer.clone())?;
                        
                        // Get final transcription
                        let final_text = transcriber.end_session()?;
                        
                        return Ok(final_text.trim().to_string());
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

// Type alias for backward compatibility
pub type ImprovedAudioProcessor = AudioProcessor;