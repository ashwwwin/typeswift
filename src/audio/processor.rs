use crate::audio::{AudioCapture, Transcriber};
use crate::config::Config;
use crate::error::VoicyResult;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub struct AudioProcessor {
    config: Config,
    audio_capture: Option<AudioCapture>,
    transcriber: Option<Transcriber>,
    processing_handle: Option<thread::JoinHandle<()>>,
    stop_signal: Option<Sender<()>>,
    result_receiver: Option<Receiver<String>>,
}

impl AudioProcessor {
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
        let sample_rate = capture.get_sample_rate();
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
                    total_audio_ms += (audio.len() as u32 * 1000) / sample_rate;
                }
                
                // Process based on config interval and minimum audio duration
                let should_process = last_process.elapsed() >= process_interval && 
                                     total_audio_ms >= min_audio_ms &&
                                     !accumulated_audio.is_empty();
                                     
                if should_process {
                    // Send the accumulated audio for transcription
                    match transcriber.process_audio(accumulated_audio.clone()) {
                        Ok(text) => {
                            if !text.is_empty() {
                                println!("💬 Live transcription: '{}'", text);
                                let _ = result_tx.send(text);
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
                    all_text.push(' ');
                }
            }
            all_text.push_str(&final_text);
            
            self.result_receiver = None;
            
            Ok(all_text.trim().to_string())
        } else {
            // Non-streaming mode: process all audio at once
            
            // Stop audio capture first
            if let Some(ref capture) = self.audio_capture {
                capture.stop_recording()?;
                
                // Read ALL accumulated audio
                let mut all_audio = Vec::new();
                loop {
                    let chunk = capture.read_audio(16000); // Read 1 second chunks at a time
                    if chunk.is_empty() {
                        break;
                    }
                    all_audio.extend(chunk);
                }
                
                if !all_audio.is_empty() {
                    println!("🎯 Processing {} total audio samples", all_audio.len());
                    
                    if let Some(ref transcriber) = self.transcriber {
                        // Start session, process, and end in one go
                        transcriber.start_session()?;
                        let text = transcriber.process_audio(all_audio)?;
                        let final_text = transcriber.end_session()?;
                        
                        let mut result = text;
                        if !result.is_empty() && !final_text.is_empty() {
                            result.push(' ');
                        }
                        result.push_str(&final_text);
                        return Ok(result.trim().to_string());
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

// Type alias for backward compatibility with main.rs
pub type ImprovedAudioProcessor = AudioProcessor;