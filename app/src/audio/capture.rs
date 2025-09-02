use crate::error::{VoicyError, VoicyResult};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::RwLock;
use ringbuf::{traits::*, HeapRb, HeapCons};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use std::sync::Arc;

pub struct AudioCapture {
    consumer: Arc<parking_lot::Mutex<HeapCons<f32>>>,
    is_recording: Arc<RwLock<bool>>,
    sample_rate: u32,
}

/// Send-safe reader that can be moved to worker threads without carrying the non-Send CPAL stream.
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
                            if overflow_count % 10000 == 0 {
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
        
        // Keep stream alive for program duration by leaking it.
        // This avoids moving a non-Send CoreAudio stream across threads while keeping it running.
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

    /// Create a Send-safe reader snapshot for use in worker threads.
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
