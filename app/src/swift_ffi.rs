use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_float, c_int};
use std::ptr;

// FFI bindings to Swift/VoicySwift
#[link(name = "VoicySwift")]
unsafe extern "C" {
    fn voicy_init(model_path: *const c_char) -> c_int;
    fn voicy_transcribe(samples: *const c_float, sample_count: c_int) -> *mut c_char;
    fn voicy_free_string(str: *mut c_char);
    fn voicy_cleanup();
    fn voicy_is_ready() -> bool;
}

/// Safe Rust wrapper for Swift transcriber
pub struct SwiftTranscriber {
    initialized: bool,
}

impl SwiftTranscriber {
    /// Create a new Swift transcriber instance
    pub fn new() -> Self {
        Self { initialized: false }
    }

    /// Initialize the transcriber with optional model path
    pub fn initialize(&mut self, model_path: Option<&str>) -> Result<(), String> {
        let c_path = model_path
            .map(|p| CString::new(p).expect("Invalid model path"))
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null());

        let result = unsafe { voicy_init(c_path) };

        if result == 0 {
            self.initialized = true;
            Ok(())
        } else {
            Err("Failed to initialize Swift transcriber".to_string())
        }
    }

    /// Transcribe audio samples to text
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if !self.initialized {
            return Err("Transcriber not initialized".to_string());
        }

        if samples.is_empty() {
            return Ok(String::new());
        }

        let c_str = unsafe {
            voicy_transcribe(
                samples.as_ptr() as *const c_float,
                samples.len() as c_int,
            )
        };

        if c_str.is_null() {
            return Err("Transcription failed".to_string());
        }

        let result = unsafe {
            let rust_str = CStr::from_ptr(c_str)
                .to_string_lossy()
                .into_owned();
            voicy_free_string(c_str);
            rust_str
        };

        Ok(result)
    }

    /// Check if transcriber is ready
    pub fn is_ready(&self) -> bool {
        unsafe { voicy_is_ready() }
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        if self.initialized {
            unsafe { voicy_cleanup() };
            self.initialized = false;
        }
    }
}

impl Drop for SwiftTranscriber {
    fn drop(&mut self) {
        self.cleanup();
    }
}

// Thread-safe wrapper for shared access
use parking_lot::Mutex;
use std::sync::Arc;

pub struct SharedSwiftTranscriber {
    inner: Arc<Mutex<SwiftTranscriber>>,
}

impl SharedSwiftTranscriber {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SwiftTranscriber::new())),
        }
    }

    pub fn initialize(&self, model_path: Option<&str>) -> Result<(), String> {
        self.inner.lock().initialize(model_path)
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.inner.lock().transcribe(samples)
    }

    pub fn is_ready(&self) -> bool {
        self.inner.lock().is_ready()
    }

    pub fn cleanup(&self) {
        self.inner.lock().cleanup()
    }
}

impl Clone for SharedSwiftTranscriber {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}