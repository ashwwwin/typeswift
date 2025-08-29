use crate::config::{ModelConfig, StreamingConfig};
use crate::error::{VoicyError, VoicyResult};
use crate::swift_ffi::SharedSwiftTranscriber;
use parking_lot::Mutex;
use std::sync::Arc;

/// Swift-based transcriber using FluidAudio and CoreML
pub struct Transcriber {
    swift_transcriber: SharedSwiftTranscriber,
    sample_rate: u32,
    model_config: ModelConfig,
    streaming_config: StreamingConfig,
    // Accumulator for batch mode (since FluidAudio doesn't support streaming yet)
    audio_buffer: Arc<Mutex<Vec<f32>>>,
}

impl Transcriber {
    pub fn new(model_config: ModelConfig, streaming_config: StreamingConfig) -> VoicyResult<Self> {
        let swift_transcriber = SharedSwiftTranscriber::new();
        
        // Initialize with model path if provided
        let model_path = if model_config.model_name.starts_with("/") {
            Some(model_config.model_name.as_str())
        } else {
            None // Use default path
        };
        
        swift_transcriber.initialize(model_path)
            .map_err(|e| VoicyError::ModelLoadFailed(format!("Swift transcriber init failed: {}", e)))?;
        
        // FluidAudio works at 16kHz
        let sample_rate = 16000;
        
        println!("✅ Swift transcriber initialized ({}Hz)", sample_rate);
        
        Ok(Self {
            swift_transcriber,
            sample_rate,
            model_config,
            streaming_config,
            audio_buffer: Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 30))),
        })
    }
    
    pub fn start_session(&self) -> VoicyResult<()> {
        // Clear buffer for new session
        self.audio_buffer.lock().clear();
        
        // Note: FluidAudio doesn't have session concept, it's batch-only
        // We'll accumulate audio in buffer for batch processing
        println!("🎙️ Transcription session started (batch mode)");
        Ok(())
    }
    
    pub fn process_audio(&self, audio: Vec<f32>) -> VoicyResult<String> {
        // Since FluidAudio doesn't support streaming yet, we accumulate audio
        // and return empty string until end_session is called
        let mut buffer = self.audio_buffer.lock();
        
        // Normalize audio to prevent clipping
        let max_amp = audio.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        
        if max_amp > 1.5 {
            let scale = 0.99 / max_amp;
            for sample in audio.iter() {
                buffer.push(sample * scale);
            }
        } else {
            buffer.extend_from_slice(&audio);
        }
        
        // Return empty for now (batch mode accumulation)
        // In future when FluidAudio supports streaming, we can return partial results
        Ok(String::new())
    }
    
    
    pub fn end_session(&self) -> VoicyResult<String> {
        // Get accumulated audio and transcribe it
        let audio = {
            let mut buffer = self.audio_buffer.lock();
            let audio = buffer.clone();
            buffer.clear();
            audio
        };
        
        if audio.is_empty() {
            println!("🛑 Transcription session ended (no audio)");
            return Ok(String::new());
        }
        
        println!("🎯 Processing {} samples ({}s)", audio.len(), audio.len() / self.sample_rate as usize);
        
        // Transcribe using Swift/FluidAudio
        let text = self.swift_transcriber.transcribe(&audio)
            .map_err(|e| VoicyError::TranscriptionFailed(format!("Swift transcription failed: {}", e)))?;
        
        println!("🛑 Transcription session ended");
        Ok(text.trim().to_string())
    }
    
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl Clone for Transcriber {
    fn clone(&self) -> Self {
        Self {
            swift_transcriber: self.swift_transcriber.clone(),
            sample_rate: self.sample_rate,
            model_config: self.model_config.clone(),
            streaming_config: self.streaming_config.clone(),
            audio_buffer: Arc::clone(&self.audio_buffer),
        }
    }
}