use cpal::Stream;
use std::sync::Arc;

/// Wrapper to safely hold the audio stream
/// The stream is leaked into memory to ensure it lives for the program duration
pub struct StreamHolder;

impl StreamHolder {
    pub fn new(stream: Stream) -> Arc<()> {
        // Leak the stream into memory - it will live for the entire program
        // This is safe because:
        // 1. We only create one audio stream per program
        // 2. The stream needs to live for the entire program duration
        // 3. The OS will clean up when the program exits
        Box::leak(Box::new(stream));
        
        // Return a dummy handle to maintain the API
        Arc::new(())
    }
}