use std::sync::mpsc::Sender;
use once_cell::sync::Lazy;
use parking_lot::Mutex as ParkingMutex;
use std::ffi::CString;
use std::os::raw::{c_char, c_float, c_int};

// ===== Keyboard FFI =====
use crate::input::HotkeyEvent;

#[link(name = "TypeswiftSwift")]
unsafe extern "C" {
    fn swift_init_keyboard_monitor() -> bool;
    fn swift_shutdown_keyboard_monitor();
    fn swift_register_push_to_talk_callback(callback: extern "C" fn(bool));
    fn swift_register_preferences_callback(callback: extern "C" fn());
}

static PUSH_TO_TALK_SENDER: Lazy<ParkingMutex<Option<Sender<HotkeyEvent>>>> = Lazy::new(|| ParkingMutex::new(None));
static PREFERENCES_SENDER: Lazy<ParkingMutex<Option<Sender<HotkeyEvent>>>> = Lazy::new(|| ParkingMutex::new(None));

pub fn init_keyboard_monitor() -> bool {
    unsafe { swift_init_keyboard_monitor() }
}

pub fn shutdown_keyboard_monitor() {
    unsafe { swift_shutdown_keyboard_monitor(); }
    PUSH_TO_TALK_SENDER.lock().take();
}

pub fn register_push_to_talk_callback(sender: Sender<HotkeyEvent>) {
    {
        *PUSH_TO_TALK_SENDER.lock() = Some(sender);
    }
    unsafe { swift_register_push_to_talk_callback(handle_push_to_talk_event) };
}

extern "C" fn handle_push_to_talk_event(is_pressed: bool) {
    if let Some(ref sender) = *PUSH_TO_TALK_SENDER.lock() {
        let event = if is_pressed {
            HotkeyEvent::PushToTalkPressed
        } else {
            HotkeyEvent::PushToTalkReleased
        };
        let _ = sender.send(event);
    }
}

pub fn register_preferences_callback(sender: Sender<HotkeyEvent>) {
    {
        *PREFERENCES_SENDER.lock() = Some(sender);
    }
    unsafe { swift_register_preferences_callback(handle_open_preferences) };
}

extern "C" fn handle_open_preferences() {
    if let Some(ref sender) = *PREFERENCES_SENDER.lock() {
        let _ = sender.send(HotkeyEvent::OpenPreferences);
    }
}

// ===== Menubar FFI =====

unsafe extern "C" {
    fn typeswift_setup_menubar();
    fn typeswift_hide_dock_icon();
    fn typeswift_show_dock_icon();
    fn typeswift_set_menu_status(text: *const c_char);
    fn typeswift_show_notification(title: *const c_char, message: *const c_char);
    fn typeswift_set_recording_state(is_recording: bool);
    fn typeswift_run_app();
    fn typeswift_terminate_app();
    fn typeswift_is_launch_at_login_enabled() -> bool;
    fn typeswift_set_launch_at_login_enabled(enabled: bool);
}

pub struct MenuBarController;

impl MenuBarController {
    pub fn setup() {
        unsafe { typeswift_setup_menubar() }
    }
    pub fn hide_dock_icon() {
        unsafe { typeswift_hide_dock_icon() }
    }
    pub fn show_dock_icon() {
        unsafe { typeswift_show_dock_icon() }
    }
    pub fn set_status(text: &str) {
        let c_text = CString::new(text).unwrap();
        unsafe { typeswift_set_menu_status(c_text.as_ptr()) }
    }
    pub fn show_notification(title: &str, message: &str) {
        let c_title = CString::new(title).unwrap();
        let c_message = CString::new(message).unwrap();
        unsafe { typeswift_show_notification(c_title.as_ptr(), c_message.as_ptr()) }
    }
    pub fn set_recording(is_recording: bool) {
        unsafe { typeswift_set_recording_state(is_recording) }
    }
    pub fn run_app() {
        unsafe { typeswift_run_app() }
    }
    pub fn quit() {
        unsafe { typeswift_terminate_app() }
    }
    pub fn is_launch_at_login_enabled() -> bool {
        unsafe { typeswift_is_launch_at_login_enabled() }
    }
    pub fn set_launch_at_login_enabled(enabled: bool) {
        unsafe { typeswift_set_launch_at_login_enabled(enabled) }
    }

}

// ===== Swift Transcriber FFI =====

#[link(name = "TypeswiftSwift")]
unsafe extern "C" {
    fn typeswift_init(model_path: *const c_char) -> c_int;
    fn typeswift_transcribe(samples: *const c_float, sample_count: c_int) -> *mut c_char;
    fn typeswift_free_string(str: *mut c_char);
    fn typeswift_cleanup();
    fn typeswift_is_ready() -> bool;
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

        let result = unsafe { typeswift_init(c_path) };
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
        let c_str = unsafe { typeswift_transcribe(samples.as_ptr() as *const c_float, samples.len() as c_int) };
        if c_str.is_null() {
            return Err("Transcription failed".to_string());
        }
        let result = unsafe {
            let rust_str = std::ffi::CStr::from_ptr(c_str).to_string_lossy().into_owned();
            typeswift_free_string(c_str);
            rust_str
        };
        Ok(result)
    }

    pub fn is_ready(&self) -> bool {
        unsafe { typeswift_is_ready() }
    }

    pub fn cleanup(&mut self) {
        if self.initialized {
            unsafe { typeswift_cleanup() };
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

// ===== Modifier State Utilities (macOS) =====

#[allow(non_upper_case_globals)]
mod modifiers {
    use std::thread;
    use std::time::{Duration, Instant};

    // kCGEventSourceStateCombinedSessionState
    const COMBINED_SESSION_STATE: u32 = 0;

    // Virtual key codes (Carbon)
    const kVK_CommandL: u16 = 0x37; // 55
    const kVK_CommandR: u16 = 0x36; // 54
    const kVK_ShiftL: u16 = 0x38;   // 56
    const kVK_ShiftR: u16 = 0x3C;   // 60
    const kVK_OptionL: u16 = 0x3A;  // 58
    const kVK_OptionR: u16 = 0x3D;  // 61
    const kVK_ControlL: u16 = 0x3B; // 59
    const kVK_ControlR: u16 = 0x3E; // 62

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGEventSourceKeyState(state_id: u32, key: u16) -> bool;
    }

    fn is_key_down(key: u16) -> bool {
        unsafe { CGEventSourceKeyState(COMBINED_SESSION_STATE, key) }
    }

    fn snapshot() -> [bool; 8] {
        [
            is_key_down(kVK_CommandL),
            is_key_down(kVK_CommandR),
            is_key_down(kVK_ShiftL),
            is_key_down(kVK_ShiftR),
            is_key_down(kVK_OptionL),
            is_key_down(kVK_OptionR),
            is_key_down(kVK_ControlL),
            is_key_down(kVK_ControlR),
        ]
    }

    fn any_down(s: &[bool; 8]) -> bool {
        s.iter().copied().any(|b| b)
    }

    fn fmt_snapshot(s: &[bool; 8]) -> String {
        let names = [
            ("CmdL", s[0]), ("CmdR", s[1]), ("ShiftL", s[2]), ("ShiftR", s[3]),
            ("OptL", s[4]), ("OptR", s[5]), ("CtrlL", s[6]), ("CtrlR", s[7]),
        ];
        let pressed: Vec<&str> = names.iter().filter_map(|(n, p)| if *p { Some(*n) } else { None }).collect();
        if pressed.is_empty() { "<none>".to_string() } else { pressed.join(",") }
    }

    pub fn wait_modifiers_released(timeout_ms: u64) -> bool {
        let start = Instant::now();
        let initial = snapshot();
        if !any_down(&initial) {
            tracing::info!("Modifiers already released");
            return true;
        }
        tracing::info!("Waiting for modifiers to release: {}", fmt_snapshot(&initial));

        let deadline = start + Duration::from_millis(timeout_ms);
        loop {
            let now = Instant::now();
            if now >= deadline {
                let last = snapshot();
                tracing::warn!(
                    "Modifier wait timeout after {}ms, still down: {}",
                    (now - start).as_millis(),
                    fmt_snapshot(&last)
                );
                return !any_down(&last);
            }
            let snap = snapshot();
            if !any_down(&snap) {
                tracing::info!("Modifiers released after {}ms", (now - start).as_millis());
                return true;
            }
            thread::sleep(Duration::from_millis(8));
        }
    }
}

pub fn wait_modifiers_released(timeout_ms: u64) -> bool {
    modifiers::wait_modifiers_released(timeout_ms)
}
