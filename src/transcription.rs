// src/transcription.rs
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Clone)]
pub struct WhisperTranscriber {
    context: Arc<WhisperContext>,
}

impl WhisperTranscriber {
    /// Initialize a new WhisperTranscriber with the model
    pub fn new() -> Result<Self> {
        let model_path = Self::get_model_path();
        println!("Loading Whisper model from: {}", model_path);

        let context =
            WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())?;

        println!("✓ Whisper model loaded successfully!");

        Ok(Self {
            context: Arc::new(context),
        })
    }

    /// Get the path to the whisper model
    fn get_model_path() -> String {
        // Try multiple locations
        let possible_paths = vec![
            // User's home directory
            dirs::home_dir()
                .map(|p| p.join("models/ggml-base.en.bin"))
                .unwrap_or_else(|| PathBuf::from("models/ggml-base.en.bin")),
            // Current directory
            PathBuf::from("models/ggml-base.en.bin"),
            // System directory
            PathBuf::from("/usr/local/share/whisper/ggml-base.en.bin"),
        ];

        for path in possible_paths {
            if path.exists() {
                return path.to_string_lossy().to_string();
            }
        }

        // Default fallback
        "models/ggml-base.en.bin".to_string()
    }

    /// Transcribe audio data to text
    pub fn transcribe(&self, audio_data: Vec<f32>) -> Result<String> {
        // Validate input
        if audio_data.is_empty() {
            return Ok(String::new());
        }

        println!("🎵 Processing audio for transcription...");
        println!("  - Input samples: {}", audio_data.len());
        
        // Preprocess audio: normalize and remove DC offset
        let processed_audio = self.preprocess_audio(audio_data);
        
        // Create a new state for this transcription
        let mut state = self.context.create_state()?;

        // Configure parameters
        let params = self.create_params();

        // Run transcription
        println!("🤖 Running Whisper transcription...");
        state.full(params, &processed_audio)?;

        // Extract text
        self.extract_text(&mut state)
    }

    /// Create transcription parameters
    fn create_params(&self) -> FullParams {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Language settings
        params.set_language(Some("en"));
        params.set_translate(false);

        // Context settings
        params.set_no_context(true);
        params.set_single_segment(false);

        // Output settings
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        // Quality settings
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);

        // Adjust for better accuracy
        params.set_temperature(0.0); // Deterministic
        params.set_no_speech_thold(0.6);
        params.set_token_timestamps(false);
        
        // Additional quality settings
        params.set_entropy_thold(2.4);

        params
    }

    /// Extract text from transcription state
    fn extract_text(&self, state: &mut whisper_rs::WhisperState) -> Result<String> {
        let num_segments = state.full_n_segments()?;

        if num_segments == 0 {
            return Ok(String::new());
        }

        let mut segments = Vec::new();

        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)?;
            let trimmed = segment.trim();

            if !trimmed.is_empty() {
                segments.push(trimmed.to_string());
            }
        }

        Ok(segments.join(" "))
    }

    /// Preprocess audio: normalize, remove DC offset, and apply voice activity detection
    fn preprocess_audio(&self, audio_data: Vec<f32>) -> Vec<f32> {
        if audio_data.is_empty() {
            return audio_data;
        }
        
        // Remove DC offset (center around zero)
        let mean = audio_data.iter().sum::<f32>() / audio_data.len() as f32;
        let mut centered: Vec<f32> = audio_data.iter().map(|&x| x - mean).collect();
        
        // Find max amplitude for normalization
        let max_amplitude = centered
            .iter()
            .map(|&x| x.abs())
            .fold(0.0f32, f32::max);
        
        println!("  - DC offset removed: {:.6}", mean);
        println!("  - Max amplitude: {:.4}", max_amplitude);
        
        // Normalize if the audio is too quiet or too loud
        if max_amplitude > 0.0 && (max_amplitude < 0.1 || max_amplitude > 1.0) {
            let scale = 0.95 / max_amplitude;
            for sample in centered.iter_mut() {
                *sample *= scale;
            }
            println!("  - Normalized with scale factor: {:.4}", scale);
        }
        
        // Apply simple noise gate to reduce low-level noise
        let noise_threshold = 0.01;
        for sample in centered.iter_mut() {
            if sample.abs() < noise_threshold {
                *sample *= 0.1; // Reduce very quiet sounds
            }
        }
        
        centered
    }

    /// Get the underlying WhisperContext (for advanced use)
    pub fn context(&self) -> Arc<WhisperContext> {
        self.context.clone()
    }
}

/// Async transcription wrapper for use with threads
pub struct AsyncTranscriber {
    transcriber: Arc<WhisperTranscriber>,
}

impl AsyncTranscriber {
    pub fn new(transcriber: WhisperTranscriber) -> Self {
        Self {
            transcriber: Arc::new(transcriber),
        }
    }

    /// Transcribe in a separate thread
    pub fn transcribe_async<F>(&self, audio_data: Vec<f32>, callback: F)
    where
        F: FnOnce(Result<String>) + Send + 'static,
    {
        let transcriber = self.transcriber.clone();

        std::thread::spawn(move || {
            let result = transcriber.transcribe(audio_data);
            callback(result);
        });
    }
}

/// Configuration for transcription
#[derive(Debug, Clone)]
pub struct TranscriptionConfig {
    pub language: String,
    pub translate: bool,
    pub max_len: i32,
    pub temperature: f32,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            translate: false,
            max_len: 0,
            temperature: 0.0,
        }
    }
}

/// Advanced transcriber with custom configuration
pub struct ConfigurableTranscriber {
    context: Arc<WhisperContext>,
    config: TranscriptionConfig,
}

impl ConfigurableTranscriber {
    pub fn new(config: TranscriptionConfig) -> Result<Self> {
        let model_path = WhisperTranscriber::get_model_path();
        let context = WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())?;

        Ok(Self {
            context: Arc::new(context),
            config,
        })
    }

    pub fn transcribe(&self, audio_data: Vec<f32>) -> Result<String> {
        let mut state = self.context.create_state()?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // Apply configuration
        params.set_language(Some(&self.config.language));
        params.set_translate(self.config.translate);
        params.set_max_len(self.config.max_len);
        params.set_temperature(self.config.temperature);

        state.full(params, &audio_data)?;

        // Extract text
        let num_segments = state.full_n_segments()?;
        let mut result = String::new();

        for i in 0..num_segments {
            if let Ok(text) = state.full_get_segment_text(i) {
                result.push_str(text.trim());
                result.push(' ');
            }
        }

        Ok(result.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_path() {
        let path = WhisperTranscriber::get_model_path();
        assert!(!path.is_empty());
    }

    #[test]
    fn test_empty_audio() {
        if let Ok(transcriber) = WhisperTranscriber::new() {
            let result = transcriber.transcribe(vec![]);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "");
        }
    }
}
