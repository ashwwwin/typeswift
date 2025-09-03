use crate::config::Config;
use crate::error::{VoicyError, VoicyResult};
use parking_lot::RwLock;
use ringbuf::{traits::*, HeapCons, HeapRb};
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::Arc;

// ===== Audio capture (cpal) =====
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioCapture {
    consumer: Arc<parking_lot::Mutex<HeapCons<f32>>>,
    is_recording: Arc<RwLock<bool>>,
    sample_rate: u32,
}

#[derive(Clone)]
pub struct AudioReader {
    consumer: Arc<parking_lot::Mutex<HeapCons<f32>>>,
    is_recording: Arc<RwLock<bool>>,
    sample_rate: u32,
}

impl AudioCapture {
    pub fn new(target_sample_rate: u32) -> VoicyResult<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| VoicyError::AudioInitFailed("No input device available".to_string()))?;

        let supported_config = device.default_input_config().map_err(|e| {
            VoicyError::AudioInitFailed(format!("Failed to get device config: {}", e))
        })?;

        let device_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels() as usize;

        println!(
            "📊 Audio device: {} Hz, {} channels → {} Hz",
            device_sample_rate, channels, target_sample_rate
        );

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

            Some(SincFixedIn::<f32>::new(resample_ratio, 2.0, params, 1024, 1).map_err(|e| {
                VoicyError::AudioInitFailed(format!("Failed to create resampler: {}", e))
            })?)
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
                                    if overflow_count % 10000 == 0 {
                                        eprintln!(
                                            "⚠️ Audio buffer overflow: {} samples dropped",
                                            overflow_count
                                        );
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
                            if overflow_count % 10000 == 0 {
                                eprintln!(
                                    "⚠️ Audio buffer overflow: {} samples dropped",
                                    overflow_count
                                );
                            }
                        }
                    }
                }
            },
            |err| eprintln!("❌ Audio stream error: {}", err),
            None,
        )
        .map_err(|e| VoicyError::AudioInitFailed(format!("Failed to build stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| VoicyError::AudioInitFailed(format!("Failed to start stream: {}", e)))?;

        // Keep stream alive for program duration by leaking it (CoreAudio stream is !Send)
        let _leaked_stream: &'static mut cpal::Stream = Box::leak(Box::new(stream));

        Ok(Self {
            consumer: Arc::new(parking_lot::Mutex::new(consumer)),
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
        let mut consumer = self.consumer.lock();
        let mut samples = Vec::with_capacity(max_samples);

        while samples.len() < max_samples {
            if let Some(sample) = consumer.try_pop() {
                samples.push(sample);
            } else {
                break;
            }
        }

        samples
    }

    pub fn is_recording(&self) -> bool {
        *self.is_recording.read()
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn reader(&self) -> AudioReader {
        AudioReader {
            consumer: Arc::clone(&self.consumer),
            is_recording: Arc::clone(&self.is_recording),
            sample_rate: self.sample_rate,
        }
    }
}

impl Clone for AudioCapture {
    fn clone(&self) -> Self {
        Self {
            consumer: Arc::clone(&self.consumer),
            is_recording: Arc::clone(&self.is_recording),
            sample_rate: self.sample_rate,
        }
    }
}

impl AudioReader {
    pub fn read_audio(&self, max_samples: usize) -> Vec<f32> {
        let mut consumer = self.consumer.lock();
        let mut samples = Vec::with_capacity(max_samples);
        while samples.len() < max_samples {
            if let Some(sample) = consumer.try_pop() {
                samples.push(sample);
            } else {
                break;
            }
        }
        samples
    }

    pub fn is_recording(&self) -> bool {
        *self.is_recording.read()
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

// ===== Swift transcriber wrapper =====
use crate::platform::macos::ffi::SharedSwiftTranscriber;
use crate::config::ModelConfig;

pub struct Transcriber {
    swift_transcriber: SharedSwiftTranscriber,
    sample_rate: u32,
    model_config: ModelConfig,
    audio_buffer: Arc<parking_lot::Mutex<Vec<f32>>>,
}

impl Transcriber {
    pub fn new(model_config: ModelConfig) -> VoicyResult<Self> {
        let swift_transcriber = SharedSwiftTranscriber::new();

        // Initialize with model path if provided
        let model_path = if model_config.model_name.starts_with('/') {
            Some(model_config.model_name.as_str())
        } else {
            None // Use default path
        };

        swift_transcriber.initialize(model_path).map_err(|e| {
            VoicyError::ModelLoadFailed(format!("Swift transcriber init failed: {}", e))
        })?;

        // FluidAudio works at 16kHz
        let sample_rate = 16000;
        println!("✅ Swift transcriber initialized ({}Hz)", sample_rate);

        Ok(Self {
            swift_transcriber,
            sample_rate,
            model_config,
            audio_buffer: Arc::new(parking_lot::Mutex::new(Vec::with_capacity(
                sample_rate as usize * 30,
            ))),
        })
    }

    pub fn start_session(&self) -> VoicyResult<()> {
        self.audio_buffer.lock().clear();
        println!("🎙️ Transcription session started (batch mode)");
        Ok(())
    }

    pub fn process_audio(&self, audio: Vec<f32>) -> VoicyResult<String> {
        // Accumulate audio; Swift side is batch-only for now
        let mut buffer = self.audio_buffer.lock();
        let max_amp = audio.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        if max_amp > 1.5 {
            let scale = 0.99 / max_amp;
            for sample in audio.iter() {
                buffer.push(sample * scale);
            }
        } else {
            buffer.extend_from_slice(&audio);
        }
        Ok(String::new())
    }

    pub fn end_session(&self) -> VoicyResult<String> {
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

        println!(
            "🎯 Processing {} samples ({}s)",
            audio.len(),
            audio.len() / self.sample_rate as usize
        );

        let text = self.swift_transcriber.transcribe(&audio).map_err(|e| {
            VoicyError::TranscriptionFailed(format!("Swift transcription failed: {}", e))
        })?;

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
            audio_buffer: Arc::clone(&self.audio_buffer),
        }
    }
}

// ===== Audio processor (orchestrates capture + transcriber) =====
pub struct AudioProcessor {
    config: Config,
    audio_capture: Option<AudioCapture>,
    transcriber: Option<Transcriber>,
    audio_buffer: Vec<f32>,
}

impl AudioProcessor {
    pub fn new(config: Config) -> Self {
        // Pre-allocate buffer for 30 seconds of audio at 16kHz
        let buffer_capacity = 16000 * 30;
        Self { config, audio_capture: None, transcriber: None, audio_buffer: Vec::with_capacity(buffer_capacity) }
    }

    pub fn initialize(&mut self) -> VoicyResult<()> {
        let transcriber = Transcriber::new(self.config.model.clone())?;
        let target_sample_rate = transcriber.get_sample_rate();
        let audio_capture = AudioCapture::new(target_sample_rate)?;
        self.transcriber = Some(transcriber);
        self.audio_capture = Some(audio_capture);
        println!("✅ Audio processor initialized");
        Ok(())
    }

    pub fn start_recording(&mut self) -> VoicyResult<()> {
        if self.audio_capture.is_none() || self.transcriber.is_none() {
            self.initialize()?;
        }
        self.audio_buffer.clear();
        if let Some(ref capture) = self.audio_capture {
            capture.start_recording()?;
        }
        // Streaming removed: batch mode only
        Ok(())
    }

    pub fn stop_recording(&mut self) -> VoicyResult<String> {
        if let Some(ref capture) = self.audio_capture {
            capture.stop_recording()?;
            self.audio_buffer.clear();
            loop {
                let chunk = capture.read_audio(8000);
                if chunk.is_empty() {
                    break;
                }
                self.audio_buffer.extend_from_slice(&chunk);
            }
            if !self.audio_buffer.is_empty() {
                println!(
                    "🎯 Processing {} samples ({}s @ 16kHz)",
                    self.audio_buffer.len(),
                    self.audio_buffer.len() / 16000
                );
                if let Some(ref transcriber) = self.transcriber {
                    transcriber.start_session()?;
                    let _ = transcriber.process_audio(self.audio_buffer.clone())?;
                    let final_text = transcriber.end_session()?;
                    return Ok(final_text.trim().to_string());
                }
            }
        }
        Ok(String::new())
    }
}

pub type ImprovedAudioProcessor = AudioProcessor;
