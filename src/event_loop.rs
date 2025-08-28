use crate::error::VoicyResult;
use crate::input::HotkeyEvent;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub type EventCallback = Arc<Mutex<dyn FnMut(HotkeyEvent) -> VoicyResult<()> + Send>>;

/// Dedicated event loop that runs independently of UI rendering
pub struct EventLoop {
    receiver: Receiver<HotkeyEvent>,
    callback: EventCallback,
    running: Arc<Mutex<bool>>,
}

impl EventLoop {
    pub fn new(receiver: Receiver<HotkeyEvent>, callback: EventCallback) -> Self {
        Self {
            receiver,
            callback,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Start the event loop in a dedicated thread
    pub fn start(self) -> Arc<Mutex<bool>> {
        let running = self.running.clone();
        *running.lock().unwrap() = true;
        
        let running_clone = running.clone();
        
        thread::spawn(move || {
            println!("🔄 Event loop started");
            
            while *running_clone.lock().unwrap() {
                match self.receiver.recv_timeout(Duration::from_millis(10)) {
                    Ok(event) => {
                        println!("⚡ Event loop processing: {:?}", event);
                        
                        if let Ok(mut callback) = self.callback.lock() {
                            if let Err(e) = callback(event) {
                                eprintln!("❌ Event processing error: {}", e);
                            }
                        } else {
                            eprintln!("❌ Failed to lock event callback");
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        // This is fine, just continue polling
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        eprintln!("⚠️ Event channel disconnected, stopping event loop");
                        break;
                    }
                }
            }
            
            println!("🛑 Event loop stopped");
        });
        
        running
    }
    
    pub fn stop(running: &Arc<Mutex<bool>>) {
        *running.lock().unwrap() = false;
    }
}