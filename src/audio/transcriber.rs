use crate::config::{ModelConfig, StreamingConfig};
use crate::error::{VoicyError, VoicyResult};
use parking_lot::Mutex;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::Arc;

/// Optimized transcriber with better performance and reliability
pub struct Transcriber {
    model: Arc<Mutex<Option<Py<PyAny>>>>,
    context: Arc<Mutex<Option<TranscriptionContext>>>,
    sample_rate: u32,
    model_config: ModelConfig,
    streaming_config: StreamingConfig,
}

/// Cached Python objects for better performance
struct TranscriptionContext {
    context: Py<PyAny>,
    numpy: Py<PyAny>,
    mlx: Py<PyAny>,
    // Pre-allocated buffer for normalization to avoid cloning
    normalized_buffer: Vec<f32>,
}

impl Transcriber {
    pub fn new(model_config: ModelConfig, streaming_config: StreamingConfig) -> VoicyResult<Self> {
        let (model, sample_rate) = Self::load_and_validate_model(&model_config.model_name)?;
        
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            context: Arc::new(Mutex::new(None)),
            sample_rate,
            model_config,
            streaming_config,
        })
    }
    
    /// Load model with proper validation and error handling
    fn load_and_validate_model(model_name: &str) -> VoicyResult<(Option<Py<PyAny>>, u32)> {
        Python::with_gil(|py| {
            // Try to import the required package
            let parakeet_mlx = match py.import("parakeet_mlx") {
                Ok(module) => module,
                Err(_) => {
                    eprintln!("⚠️ parakeet_mlx not installed - running in demo mode");
                    eprintln!("   Install with: pip install parakeet-mlx");
                    return Ok((None, 16000));
                }
            };
            
            // Cache numpy import for later use
            let numpy = py.import("numpy")?;
            py.import("builtins")?.setattr("np", numpy)?;
            
            // Load the specific model
            println!("🚀 Loading model: {}", model_name);
            let model = parakeet_mlx
                .getattr("from_pretrained")?
                .call1((model_name,))?;
            
            // Validate model has required methods
            model.getattr("transcribe_stream")?;
            
            // Get sample rate from preprocessor config
            let preprocessor_config = model.getattr("preprocessor_config")?;
            let sample_rate: u32 = preprocessor_config
                .getattr("sample_rate")?
                .extract()
                .unwrap_or(16000); // Fallback to 16kHz if not specified
            
            println!("✅ Model loaded successfully ({}Hz)", sample_rate);
            
            Ok((Some(model.into()), sample_rate))
        }).map_err(|e: pyo3::PyErr| {
            VoicyError::ModelLoadFailed(format!("Model initialization failed: {}", e))
        })
    }
    
    pub fn start_session(&self) -> VoicyResult<()> {
        let model = self.model.lock();
        
        if let Some(ref model_py) = *model {
            Python::with_gil(|py| {
                let model_ref = model_py.bind(py);
                
                // Create context with configuration
                let kwargs = PyDict::new(py);
                kwargs.set_item("context_size", (
                    self.model_config.left_context_seconds,
                    self.model_config.right_context_seconds
                ))?;
                
                let context = model_ref
                    .getattr("transcribe_stream")?
                    .call((), Some(&kwargs))?
                    .call_method0("__enter__")?;
                
                // Cache Python modules for better performance
                let numpy = py.import("numpy")?;
                let mlx = py.import("mlx.core")?;
                
                // Pre-allocate normalization buffer (30 seconds at 16kHz)
                let buffer_capacity = self.sample_rate as usize * 30;
                
                *self.context.lock() = Some(TranscriptionContext {
                    context: context.unbind(),
                    numpy: numpy.into(),
                    mlx: mlx.into(),
                    normalized_buffer: Vec::with_capacity(buffer_capacity),
                });
                
                println!("🎙️ Transcription session started");
                Ok(())
            }).map_err(|e: pyo3::PyErr| {
                VoicyError::TranscriptionFailed(format!("Session start failed: {}", e))
            })
        } else {
            println!("🎙️ Transcription session started (demo mode)");
            Ok(())
        }
    }
    
    pub fn process_audio(&self, audio: Vec<f32>) -> VoicyResult<String> {
        let mut context_guard = self.context.lock();
        
        if let Some(ref mut context) = *context_guard {
            Python::with_gil(|py| {
                // Use cached Python objects
                let context_ref = context.context.bind(py);
                let numpy = context.numpy.bind(py);
                let mlx = context.mlx.bind(py);
                
                // Reuse normalized buffer to avoid allocation
                context.normalized_buffer.clear();
                context.normalized_buffer.extend_from_slice(&audio);
                
                // In-place normalization
                let max_amp = context.normalized_buffer
                    .iter()
                    .map(|&x| x.abs())
                    .fold(0.0f32, f32::max);
                
                if max_amp > 1.5 {
                    let scale = 0.99 / max_amp;
                    for sample in &mut context.normalized_buffer {
                        *sample *= scale;
                    }
                }
                
                // Create numpy array from normalized buffer
                let numpy_array = numpy
                    .getattr("array")?
                    .call1((&context.normalized_buffer,))?
                    .call_method1("astype", ("float32",))?;
                
                // Convert to MLX array and add to context
                let audio_array = mlx.getattr("array")?.call1((numpy_array,))?;
                context_ref.call_method1("add_audio", (audio_array,))?;
                
                // Extract tokens efficiently
                let text = self.extract_tokens_from_context(py, &context_ref)?;
                
                Ok(text)
            }).map_err(|e: pyo3::PyErr| {
                VoicyError::TranscriptionFailed(format!("Processing failed: {}", e))
            })
        } else {
            // Demo mode
            Ok(String::new())
        }
    }
    
    /// Centralized token extraction logic (DRY principle)
    fn extract_tokens_from_context(&self, py: Python, context: &Bound<PyAny>) -> PyResult<String> {
        let mut text = String::with_capacity(512); // Pre-allocate reasonable size
        
        // Try finalized tokens first
        if let Ok(finalized) = context.getattr("finalized_tokens") {
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
        
        // Only check draft tokens if we have no finalized text (streaming mode)
        if text.is_empty() && self.streaming_config.enabled {
            if let Ok(draft) = context.getattr("draft_tokens") {
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
    }
    
    pub fn end_session(&self) -> VoicyResult<String> {
        let mut context_guard = self.context.lock();
        
        if let Some(context) = context_guard.take() {
            Python::with_gil(|py| {
                let context_ref = context.context.bind(py);
                
                // Get final text using shared extraction logic
                let final_text = self.extract_tokens_from_context(py, &context_ref)?;
                
                // Properly clean up the context
                let py_none = py.None();
                let none_ref = py_none.bind(py);
                context_ref.call_method1("__exit__", (none_ref, none_ref, none_ref))?;
                
                println!("🛑 Transcription session ended");
                Ok(final_text)
            }).map_err(|e: pyo3::PyErr| {
                VoicyError::TranscriptionFailed(format!("Session end failed: {}", e))
            })
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