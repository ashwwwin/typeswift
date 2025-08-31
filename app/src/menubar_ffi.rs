/// FFI bindings for macOS menu bar functionality

use std::ffi::CString;
use std::os::raw::c_char;

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

// Declaration for the reset function
unsafe extern "C" {
    pub fn voicy_reset_first_launch();
}

/// Menu bar controller for Voicy
pub struct MenuBarController;

impl MenuBarController {
    /// Initialize the menu bar
    pub fn setup() {
        unsafe {
            voicy_setup_menubar();
        }
    }
    
    /// Hide the dock icon (make it menu bar only)
    pub fn hide_dock_icon() {
        unsafe {
            voicy_hide_dock_icon();
        }
    }
    
    /// Show the dock icon (if needed for preferences)
    pub fn show_dock_icon() {
        unsafe {
            voicy_show_dock_icon();
        }
    }
    
    /// Update the menu bar status text
    pub fn set_status(text: &str) {
        let c_text = CString::new(text).unwrap();
        unsafe {
            voicy_set_menu_status(c_text.as_ptr());
        }
    }
    
    /// Show a macOS notification
    pub fn show_notification(title: &str, message: &str) {
        let c_title = CString::new(title).unwrap();
        let c_message = CString::new(message).unwrap();
        unsafe {
            voicy_show_notification(c_title.as_ptr(), c_message.as_ptr());
        }
    }
    
    /// Update recording state indicator
    pub fn set_recording(is_recording: bool) {
        unsafe {
            voicy_set_recording_state(is_recording);
        }
    }
    
    /// Run the app (blocks until terminated)
    pub fn run_app() {
        unsafe {
            voicy_run_app();
        }
    }
    
    /// Terminate the app
    pub fn quit() {
        unsafe {
            voicy_terminate_app();
        }
    }
}

// Example usage in main.rs:
// ```rust
// use menubar_ffi::MenuBarController;
// 
// fn main() {
//     // Initialize as menu bar app
//     MenuBarController::hide_dock_icon();
//     MenuBarController::setup();
//     
//     // Start recording
//     MenuBarController::set_recording(true);
//     MenuBarController::show_notification("Voicy", "Recording started");
//     
//     // Run the app
//     MenuBarController::run_app();
// }
// ```