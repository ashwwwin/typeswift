use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct AudioRecorder {
    audio_buffer: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    stop_sender: Option<mpsc::Sender<()>>,
    sample_rate: u32,
    device_sample_rate: u32,
    channels: u16,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(Mutex::new(false)),
            stop_sender: None,
            sample_rate: 16000, // Whisper expects 16kHz
            device_sample_rate: 48000, // Will be updated with actual device rate
            channels: 1, // Will be updated with actual channel count
        }
    }

    pub fn start_recording(&mut self) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        let config = device.default_input_config()?;
        
        // Store the actual device sample rate and channels
        let device_sample_rate = config.sample_rate().0;
        let channels = config.channels();
        self.device_sample_rate = device_sample_rate;
        self.channels = channels;
        
        println!("📊 Device info:");
        println!("  - Device: {:?}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
        println!("  - Sample rate: {} Hz", device_sample_rate);
        println!("  - Channels: {}", channels);
        println!("  - Sample format: {:?}", config.sample_format());

        let buffer = self.audio_buffer.clone();
        let is_recording = self.is_recording.clone();

        *is_recording.lock().unwrap() = true;
        buffer.lock().unwrap().clear();

        let (tx, rx) = mpsc::channel();
        self.stop_sender = Some(tx);

        // Start recording in a separate thread
        thread::spawn(move || {
            let channels_clone = channels;
            let stream = device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        if *is_recording.lock().unwrap() {
                            // Convert to mono if needed
                            let mono_data = if channels_clone > 1 {
                                // Average all channels to create mono
                                let frames = data.len() / channels_clone as usize;
                                let mut mono = Vec::with_capacity(frames);
                                for i in 0..frames {
                                    let mut sum = 0.0;
                                    for ch in 0..channels_clone as usize {
                                        sum += data[i * channels_clone as usize + ch];
                                    }
                                    mono.push(sum / channels_clone as f32);
                                }
                                mono
                            } else {
                                data.to_vec()
                            };
                            buffer.lock().unwrap().extend(mono_data);
                        }
                    },
                    |err| eprintln!("Audio stream error: {}", err),
                    None,
                )
                .expect("Failed to build input stream");

            stream.play().expect("Failed to play stream");

            // Block until stop signal received
            let _ = rx.recv();
        });

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Vec<f32> {
        *self.is_recording.lock().unwrap() = false;
        
        // Send stop signal to recording thread
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
        
        thread::sleep(Duration::from_millis(100)); // Let the stream finish

        let buffer = self.audio_buffer.lock().unwrap();
        let raw_audio = buffer.clone();
        
        println!("🎧 Raw audio stats:");
        println!("  - Samples recorded: {}", raw_audio.len());
        println!("  - Duration at {} Hz: {:.2} seconds", self.device_sample_rate, raw_audio.len() as f32 / self.device_sample_rate as f32);
        
        if raw_audio.len() > 0 {
            let max = raw_audio.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let min = raw_audio.iter().cloned().fold(f32::INFINITY, f32::min);
            let avg = raw_audio.iter().sum::<f32>() / raw_audio.len() as f32;
            println!("  - Max amplitude: {:.4}", max);
            println!("  - Min amplitude: {:.4}", min);
            println!("  - Average: {:.6}", avg);
        }
        
        // Resample to 16kHz for Whisper
        let resampled = self.resample_to_16khz(raw_audio);
        println!("  - Resampled samples: {} (16kHz)", resampled.len());
        
        resampled
    }
    
    fn resample_to_16khz(&self, input: Vec<f32>) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        
        let input_rate = self.device_sample_rate as f32;
        let output_rate = 16000.0;
        let ratio = output_rate / input_rate;
        
        let output_len = (input.len() as f32 * ratio) as usize;
        let mut output = Vec::with_capacity(output_len);
        
        // Simple linear interpolation resampling
        for i in 0..output_len {
            let src_idx = i as f32 / ratio;
            let src_idx_int = src_idx as usize;
            let frac = src_idx - src_idx_int as f32;
            
            if src_idx_int + 1 < input.len() {
                // Linear interpolation between two samples
                let sample = input[src_idx_int] * (1.0 - frac) + input[src_idx_int + 1] * frac;
                output.push(sample);
            } else if src_idx_int < input.len() {
                output.push(input[src_idx_int]);
            }
        }
        
        output
    }
}