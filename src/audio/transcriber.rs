use crate::config::{ModelConfig, StreamingConfig};
use crate::error::{VoicyError, VoicyResult};
use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;

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
        let model = self.model.lock();
        
        if let Some(ref model_py) = *model {
            Python::with_gil(|py| {
                let model_ref = model_py.bind(py);
                
                let kwargs = PyDict::new(py);
                kwargs.set_item("context_size", (
                    self.model_config.left_context_seconds,
                    self.model_config.right_context_seconds
                ))?;
                
                let context = model_ref
                    .getattr("transcribe_stream")?
                    .call((), Some(&kwargs))?
                    .call_method0("__enter__")?;
                
                *self.context.lock() = Some(context.unbind());
                
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
        let context = self.context.lock();
        
        if let Some(ref context_py) = *context {
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
                
                // Also check draft tokens for live feedback if no finalized text
                if text.is_empty() {
                    if let Ok(draft) = context_ref.getattr("draft_tokens") {
                        if let Ok(token_list) = draft.extract::<Vec<Py<PyAny>>>() {
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
                
                Ok(text)
            }).map_err(|e: pyo3::PyErr| VoicyError::TranscriptionFailed(format!("Processing failed: {}", e)))
        } else {
            // Demo mode - return empty or simulated text
            Ok(String::new())
        }
    }
    
    pub fn end_session(&self) -> VoicyResult<String> {
        let mut context = self.context.lock();
        
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
                
                // Clean up context properly
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