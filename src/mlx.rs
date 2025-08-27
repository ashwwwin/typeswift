use anyhow::Result;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MLXParakeet {
    model: Arc<Mutex<Py<PyAny>>>,
    transcriber: Arc<Mutex<Option<Py<PyAny>>>>,
    sample_rate: u32,
}

impl MLXParakeet {
    pub fn new() -> Result<Self> {
        Python::with_gil(|py| {
            // Import required modules
            let parakeet_mlx = py.import("parakeet_mlx")?;
            let numpy = py.import("numpy")?;

            // Store numpy in builtins so we can access it later
            py.import("builtins")?.setattr("np", numpy)?;

            // Load the model
            println!("🚀 Loading MLX Parakeet model...");
            let model = parakeet_mlx
                .getattr("from_pretrained")?
                .call1(("mlx-community/parakeet-tdt-0.6b-v2",))?;

            // Get sample rate from preprocessor config
            let preprocessor_config = model.getattr("preprocessor_config")?;
            let sample_rate: u32 = preprocessor_config.getattr("sample_rate")?.extract()?;

            println!("✅ Model loaded! Sample rate: {} Hz", sample_rate);

            Ok(Self {
                model: Arc::new(Mutex::new(model.into())),
                transcriber: Arc::new(Mutex::new(None)),
                sample_rate,
            })
        })
    }

    pub fn start_streaming(&self, left_context: usize, right_context: usize) -> Result<()> {
        Python::with_gil(|py| {
            let model = self.model.lock().unwrap();
            let model_ref = model.bind(py);

            // Create context tuple
            let kwargs = PyDict::new(py);
            kwargs.set_item("context_size", (left_context, right_context))?;

            // Start streaming context
            let transcriber = model_ref
                .getattr("transcribe_stream")?
                .call((), Some(&kwargs))?
                .call_method0("__enter__")?;

            *self.transcriber.lock().unwrap() = Some(transcriber.unbind());
            println!(
                "🎙️ Streaming started with context: ({}, {})",
                left_context, right_context
            );

            Ok(())
        })
    }

    pub fn process_audio_chunk(&self, mut audio_data: Vec<f32>) -> Result<TranscriptionResult> {
        Python::with_gil(|py| {
            let transcriber_lock = self.transcriber.lock().unwrap();

            if let Some(ref transcriber) = *transcriber_lock {
                let transcriber_ref = transcriber.bind(py);
                let numpy = py.import("numpy")?;
                let mlx = py.import("mlx.core")?;

                // Log input audio characteristics
                let max_amp = audio_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                let min_val = audio_data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                let max_val = audio_data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

                println!(
                    "    🎵 MLX input: {} samples, range [{:.4}, {:.4}], max_amp: {:.4}",
                    audio_data.len(),
                    min_val,
                    max_val,
                    max_amp
                );

                // IMPORTANT: Send raw audio as-is to the model
                // The model expects raw PCM and does ALL preprocessing:
                // - Pre-emphasis (0.97)
                // - Mel-spectrogram conversion  
                // - Per-feature normalization
                // - Windowing (25ms Hann windows)
                // Only clip if absolutely necessary to prevent overflow
                if max_amp > 1.5 {
                    println!(
                        "    ⚠️  Extreme clipping! Scaling from {:.4} to 0.99",
                        max_amp
                    );
                    let scale = 0.99 / max_amp;
                    for sample in &mut audio_data {
                        *sample *= scale;
                    }
                }

                // Convert Vec<f32> to numpy array first
                let numpy_array = numpy
                    .getattr("array")?
                    .call1((audio_data,))?
                    .call_method1("astype", ("float32",))?;

                // Convert numpy array to MLX array
                let audio_array = mlx.getattr("array")?.call1((numpy_array,))?;

                // Add audio to the transcriber
                transcriber_ref.call_method1("add_audio", (audio_array,))?;
                
                // Get current result
                let result = transcriber_ref.getattr("result")?;

                // Extract text
                let text: String = result
                    .getattr("text")?
                    .extract()
                    .unwrap_or_else(|_| String::new());

                // Extract tokens with timestamps
                let mut tokens = Vec::new();
                if let Ok(finalized) = transcriber_ref.getattr("finalized_tokens") {
                    if let Ok(token_list) = finalized.extract::<Vec<Py<PyAny>>>() {
                        for token_obj in token_list {
                            let token_bound = token_obj.bind(py);
                            if let Ok(token_text) = token_bound.getattr("text") {
                                if let Ok(text) = token_text.extract::<String>() {
                                    let start = token_bound
                                        .getattr("start")
                                        .and_then(|s| s.extract::<f32>())
                                        .unwrap_or(0.0);
                                    let end = token_bound
                                        .getattr("end")
                                        .and_then(|e| e.extract::<f32>())
                                        .unwrap_or(0.0);

                                    tokens.push(Token {
                                        text: text.clone(),
                                        start,
                                        end,
                                    });
                                }
                            }
                        }
                    }
                }

                // Get draft tokens count for progress
                let draft_count = transcriber_ref
                    .getattr("draft_tokens")
                    .and_then(|d| d.call_method0("__len__"))
                    .and_then(|l| l.extract::<usize>())
                    .unwrap_or(0);

                Ok(TranscriptionResult {
                    text,
                    tokens,
                    draft_token_count: draft_count,
                })
            } else {
                Err(anyhow::anyhow!("Streaming not started"))
            }
        })
    }

    pub fn stop_streaming(&self) -> Result<String> {
        Python::with_gil(|py| {
            let mut transcriber_lock = self.transcriber.lock().unwrap();

            if let Some(transcriber) = transcriber_lock.take() {
                let transcriber_ref = transcriber.bind(py);

                // Get final result before closing
                let result = transcriber_ref.getattr("result")?;
                let final_text: String = result.getattr("text")?.extract()?;

                // Exit the context manager with proper None handling
                let py_none = py.None();
                let none_ref = py_none.bind(py);
                transcriber_ref.call_method1("__exit__", (none_ref, none_ref, none_ref))?;

                println!("🛑 Streaming stopped");
                Ok(final_text)
            } else {
                Ok(String::new())
            }
        })
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub tokens: Vec<Token>,
    #[allow(dead_code)]
    pub draft_token_count: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Token {
    pub text: String,
    pub start: f32,
    pub end: f32,
}
