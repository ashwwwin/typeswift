use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{traits::*, HeapRb, HeapCons};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};
use std::sync::{Arc, Mutex};

pub struct AudioStream {
    consumer: Arc<Mutex<HeapCons<f32>>>,
    sample_rate: u32,
    is_playing: Arc<Mutex<bool>>,
    // Stream is not Send, so we manage it differently
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

impl AudioStream {
    pub fn new(target_sample_rate: u32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        // Get the device's default configuration
        let supported_config = device.default_input_config()?;
        let device_sample_rate = supported_config.sample_rate().0;
        let channels = supported_config.channels();
        
        println!("📊 Device configuration:");
        println!("  - Sample rate: {} Hz", device_sample_rate);
        println!("  - Channels: {}", channels);
        println!("  - Target rate: {} Hz", target_sample_rate);

        // Create ring buffer for lock-free audio transfer
        // Size it based on the target sample rate (after resampling)
        let ring_buffer_size = target_sample_rate as usize * 10; // 10 seconds buffer
        let rb = HeapRb::<f32>::new(ring_buffer_size);
        let (mut producer, consumer) = rb.split();

        // Use the device's native configuration
        let config = supported_config.into();

        let is_playing = Arc::new(Mutex::new(false));
        let is_playing_clone = is_playing.clone();
        
        // Calculate resampling ratio
        let resample_ratio = target_sample_rate as f64 / device_sample_rate as f64;
        let channels_usize = channels as usize;
        
        // Create high-quality resampler with sinc interpolation
        let params = SincInterpolationParameters {
            sinc_len: 128,  // Balanced quality/performance
            f_cutoff: 0.95, // Anti-aliasing filter cutoff
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        
        // Use a standard chunk size that aligns with common audio processing
        let chunk_size = 1024; // Standard power-of-2 size
        let mut resampler = SincFixedIn::<f32>::new(
            resample_ratio,
            2.0,  // Max delay
            params,
            chunk_size,
            1,    // Single channel
        ).expect("Failed to create resampler");
        
        // Buffer for accumulating samples for resampling
        let mut input_buffer = Vec::new();

        // Build input stream
        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &_| {
                if *is_playing_clone.lock().unwrap() {
                    // Convert to mono if needed
                    let mono_data: Vec<f32> = if channels_usize > 1 {
                        data.chunks(channels_usize)
                            .map(|frame| {
                                frame.iter().sum::<f32>() / channels_usize as f32
                            })
                            .collect()
                    } else {
                        data.to_vec()
                    };
                    
                    // Add to input buffer
                    input_buffer.extend(mono_data);
                    
                    // Process in chunks of the resampler's required size
                    while input_buffer.len() >= chunk_size {
                        // Take exactly chunk_size samples
                        let input_chunk: Vec<f32> = input_buffer.drain(..chunk_size).collect();
                        
                        // Check input audio
                        let input_max = input_chunk.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                        
                        // Resample using high-quality sinc interpolation
                        let resampled = resampler.process(&[input_chunk], None).unwrap();
                        
                        // Check output audio
                        if !resampled[0].is_empty() {
                            let output_max = resampled[0].iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
                            
                            // Log if there's a significant change
                            if (input_max - output_max).abs() > 0.1 {
                                println!("🔄 Resampler: input_max={:.4} -> output_max={:.4} (Δ={:.4})", 
                                    input_max, output_max, output_max - input_max);
                            }
                        }
                        
                        // Push resampled audio to ring buffer
                        for sample in &resampled[0] {
                            if producer.try_push(*sample).is_err() {
                                // Buffer full - this means we're losing audio!
                                eprintln!("⚠️  Ring buffer full - dropping {} samples!", resampled[0].len());
                                break;
                            }
                        }
                    }
                    
                    // Prevent buffer from growing too large
                    if input_buffer.len() > device_sample_rate as usize {
                        input_buffer.clear();
                    }
                }
            },
            |err| eprintln!("❌ Audio error: {}", err),
            None,
        )?;

        // Start the stream immediately but control recording with is_playing flag
        stream.play()?;
        
        // Keep the stream alive for the duration of the program
        // This is necessary because Stream is !Send and can't be stored in Arc
        Box::leak(Box::new(stream));
        
        Ok(Self {
            consumer: Arc::new(Mutex::new(consumer)),
            sample_rate: target_sample_rate,
            is_playing,
        })
    }

    pub fn start(&self) -> Result<()> {
        *self.is_playing.lock().unwrap() = true;
        println!("🎤 Audio stream started");
        Ok(())
    }

    pub fn read_chunk(&self, chunk_size: usize) -> Vec<f32> {
        let mut consumer = self.consumer.lock().unwrap();
        let mut chunk = Vec::with_capacity(chunk_size);

        // Read up to chunk_size samples
        while chunk.len() < chunk_size {
            if let Some(sample) = consumer.try_pop() {
                chunk.push(sample);
            } else {
                break; // No more samples available
            }
        }
        
        // Debug: Show if we're getting data
        if chunk.len() > 0 && chunk.len() % 8000 == 0 {  // Log every ~0.5s worth of data
            let max_val = chunk.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
            if max_val > 0.001 {
                println!("  📊 Stream: Read {} samples, max amplitude: {:.4}", chunk.len(), max_val);
            }
        }

        chunk
    }

    pub fn stop(&self) {
        *self.is_playing.lock().unwrap() = false;
        println!("🎤 Audio stream stopped");
    }
}