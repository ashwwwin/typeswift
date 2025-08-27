use crate::output::TypingQueue;
use std::sync::{Arc, RwLock};

/// Manages incremental typing during streaming transcription
pub struct StreamingManager {
    typing_queue: TypingQueue,
    typed_text: Arc<RwLock<String>>,
    pending_text: Arc<RwLock<String>>,
}

impl StreamingManager {
    pub fn new(typing_queue: TypingQueue) -> Self {
        Self {
            typing_queue,
            typed_text: Arc::new(RwLock::new(String::new())),
            pending_text: Arc::new(RwLock::new(String::new())),
        }
    }
    
    /// Process new transcription text and type only the new parts
    pub fn process_live_text(&self, new_full_text: &str) {
        let typed = self.typed_text.read().unwrap();
        
        // Find what's new compared to what we've already typed
        if new_full_text.len() > typed.len() {
            // Check if the new text starts with what we've already typed
            if new_full_text.starts_with(typed.as_str()) {
                // Extract only the new part
                let new_part = &new_full_text[typed.len()..];
                
                // Type the new part
                if !new_part.is_empty() {
                    println!("⌨️ Live typing: '{}'", new_part);
                    if self.typing_queue.queue_typing(new_part.to_string(), false).is_ok() {
                        // Update what we've typed
                        drop(typed);
                        let mut typed_mut = self.typed_text.write().unwrap();
                        *typed_mut = new_full_text.to_string();
                    }
                }
            } else {
                // Text changed in a way that's not just appending
                // This might happen if the model corrects earlier text
                println!("🔄 Text correction detected, will handle on release");
                let mut pending = self.pending_text.write().unwrap();
                *pending = new_full_text.to_string();
            }
        }
    }
    
    /// Clear the typed text buffer when starting a new recording
    pub fn reset(&self) {
        let mut typed = self.typed_text.write().unwrap();
        typed.clear();
        let mut pending = self.pending_text.write().unwrap();
        pending.clear();
    }
    
    /// Get any pending corrections that need to be handled
    pub fn get_pending_corrections(&self) -> Option<String> {
        let pending = self.pending_text.read().unwrap();
        if !pending.is_empty() {
            Some(pending.clone())
        } else {
            None
        }
    }
}

impl Clone for StreamingManager {
    fn clone(&self) -> Self {
        Self {
            typing_queue: self.typing_queue.clone(),
            typed_text: Arc::clone(&self.typed_text),
            pending_text: Arc::clone(&self.pending_text),
        }
    }
}