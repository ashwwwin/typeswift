use std::sync::mpsc::Sender;
use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int};

// ===== Keyboard FFI =====
use crate::input::HotkeyEvent;

#[link(name = "VoicySwift")]
unsafe extern "C" {
    fn swift_init_keyboard_monitor() -> bool;
    fn swift_shutdown_keyboard_monitor();
    fn swift_register_push_to_talk_callback(callback: extern "C" fn(bool));
    fn swift_register_preferences_callback(callback: extern "C" fn());
}

static mut PUSH_TO_TALK_SENDER: Option<Sender<HotkeyEvent>> = None;
static mut PREFERENCES_SENDER: Option<Sender<HotkeyEvent>> = None;

pub fn init_keyboard_monitor() -> bool {
    unsafe { swift_init_keyboard_monitor() }
}

pub fn shutdown_keyboard_monitor() {
    unsafe {
        swift_shutdown_keyboard_monitor();
        PUSH_TO_TALK_SENDER = None;
    }
}

pub fn register_push_to_talk_callback(sender: Sender<HotkeyEvent>) {
    unsafe {
        PUSH_TO_TALK_SENDER = Some(sender);
        swift_register_push_to_talk_callback(handle_push_to_talk_event);
    }
}

extern "C" fn handle_push_to_talk_event(is_pressed: bool) {
    unsafe {
        if let Some(ref sender) = PUSH_TO_TALK_SENDER {
            let event = if is_pressed {
                HotkeyEvent::PushToTalkPressed
            } else {
                HotkeyEvent::PushToTalkReleased
            };
            let _ = sender.send(event);
        }
    }
}

pub fn register_preferences_callback(sender: Sender<HotkeyEvent>) {
    unsafe {
        PREFERENCES_SENDER = Some(sender);
        swift_register_preferences_callback(handle_open_preferences);
    }
}

extern "C" fn handle_open_preferences() {
    unsafe {
        if let Some(ref sender) = PREFERENCES_SENDER {
            let _ = sender.send(HotkeyEvent::OpenPreferences);
        }
    }
}

// ===== Menubar FFI =====

unsafe extern "C" {
    fn voicy_setup_menubar();
    fn voicy_hide_dock_icon();
    fn voicy_show_dock_icon();
    fn voicy_set_menu_status(text: *const c_char);
    fn voicy_show_notification(title: *const c_char, message: *const c_char);
    fn voicy_set_recording_state(is_recording: bool);
    fn voicy_run_app();
    fn voicy_terminate_app();
}

pub struct MenuBarController;

impl MenuBarController {
    pub fn setup() {
        unsafe { voicy_setup_menubar() }
    }
    pub fn hide_dock_icon() {
        unsafe { voicy_hide_dock_icon() }
    }
    pub fn show_dock_icon() {
        unsafe { voicy_show_dock_icon() }
    }
    pub fn set_status(text: &str) {
        let c_text = CString::new(text).unwrap();
        unsafe { voicy_set_menu_status(c_text.as_ptr()) }
    }
    pub fn show_notification(title: &str, message: &str) {
        let c_title = CString::new(title).unwrap();
        let c_message = CString::new(message).unwrap();
        unsafe { voicy_show_notification(c_title.as_ptr(), c_message.as_ptr()) }
    }
    pub fn set_recording(is_recording: bool) {
        unsafe { voicy_set_recording_state(is_recording) }
    }
    pub fn run_app() {
        unsafe { voicy_run_app() }
    }
    pub fn quit() {
        unsafe { voicy_terminate_app() }
    }

}

// ===== Swift Transcriber FFI =====

#[link(name = "VoicySwift")]
unsafe extern "C" {
    fn voicy_init(model_path: *const c_char) -> c_int;
    fn voicy_transcribe(samples: *const c_float, sample_count: c_int) -> *mut c_char;
    fn voicy_free_string(str: *mut c_char);
    fn voicy_cleanup();
    fn voicy_is_ready() -> bool;
}

pub struct SwiftTranscriber {
    initialized: bool,
}

impl SwiftTranscriber {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub fn initialize(&mut self, model_path: Option<&str>) -> Result<(), String> {
        let c_path = model_path
            .map(|p| CString::new(p).expect("Invalid model path"))
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null());

        let result = unsafe { voicy_init(c_path) };
        if result == 0 {
            self.initialized = true;
            Ok(())
        } else {
            Err("Failed to initialize Swift transcriber".to_string())
        }
    }

    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        if !self.initialized {
            return Err("Transcriber not initialized".to_string());
        }
        if samples.is_empty() {
            return Ok(String::new());
        }
        let c_str = unsafe { voicy_transcribe(samples.as_ptr() as *const c_float, samples.len() as c_int) };
        if c_str.is_null() {
            return Err("Transcription failed".to_string());
        }
        let result = unsafe {
            let rust_str = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
            voicy_free_string(c_str);
            rust_str
        };
        Ok(result)
    }

    pub fn is_ready(&self) -> bool {
        unsafe { voicy_is_ready() }
    }

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

use parking_lot::Mutex;
use std::sync::Arc;

pub struct SharedSwiftTranscriber {
    inner: Arc<Mutex<SwiftTranscriber>>,
}

impl SharedSwiftTranscriber {
    pub fn new() -> Self {
        Self { inner: Arc::new(Mutex::new(SwiftTranscriber::new())) }
    }
    pub fn initialize(&self, model_path: Option<&str>) -> Result<(), String> {
        self.inner.lock().initialize(model_path)
    }
    pub fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.inner.lock().transcribe(samples)
    }
    pub fn is_ready(&self) -> bool { self.inner.lock().is_ready() }
    pub fn cleanup(&self) { self.inner.lock().cleanup() }
}

impl Clone for SharedSwiftTranscriber {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}
