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
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            audio_buffer: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(Mutex::new(false)),
            stop_sender: None,
            sample_rate: 16000, // Whisper expects 16kHz
        }
    }

    pub fn start_recording(&mut self) -> anyhow::Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No input device available"))?;

        let config = device.default_input_config()?;

        let buffer = self.audio_buffer.clone();
        let is_recording = self.is_recording.clone();

        *is_recording.lock().unwrap() = true;
        buffer.lock().unwrap().clear();

        let (tx, rx) = mpsc::channel();
        self.stop_sender = Some(tx);

        // Start recording in a separate thread
        thread::spawn(move || {
            let stream = device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        if *is_recording.lock().unwrap() {
                            buffer.lock().unwrap().extend_from_slice(data);
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
        buffer.clone()
    }
}