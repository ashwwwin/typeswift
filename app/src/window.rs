use crate::error::{VoicyError, VoicyResult};
use parking_lot::RwLock;
use std::sync::Arc;

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use dispatch::Queue;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    Hidden,
    Visible,
}

pub struct WindowManager {
    state: Arc<RwLock<WindowState>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(WindowState::Hidden)),
        }
    }
}

impl Clone for WindowManager {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl WindowManager {
    pub fn setup_properties() -> VoicyResult<()> {
        #[cfg(target_os = "macos")]
        {
            setup_window_properties_macos()
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            println!("⚠️ Window setup not implemented for this platform");
            Ok(())
        }
    }
    
    pub fn show_without_focus(&self) -> VoicyResult<()> {
        println!("🪟 Showing window without focus");
        
        #[cfg(target_os = "macos")]
        {
            let state = self.state.clone();
            
            Queue::main().exec_async(move || {
                if let Err(e) = show_window_macos() {
                    eprintln!("❌ Failed to show window: {}", e);
                    return;
                }
                // Explicitly deactivate so we never steal focus
                if let Err(e) = deactivate_app_macos() {
                    eprintln!("⚠️ Failed to deactivate app after show: {}", e);
                }
                *state.write() = WindowState::Visible;
                println!("✅ Window shown (no focus steal)");
            });
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            *self.state.write() = WindowState::Visible;
            println!("✅ Window shown (simulated)");
        }
        
        Ok(())
    }
    
    pub fn hide(&self) -> VoicyResult<()> {
        println!("🪟 Hiding window");
        
        #[cfg(target_os = "macos")]
        {
            let state = self.state.clone();
            
            Queue::main().exec_async(move || {
                if let Err(e) = hide_window_macos() {
                    eprintln!("❌ Failed to hide window: {}", e);
                    return;
                }
                *state.write() = WindowState::Hidden;
                println!("✅ Window hidden");
            });
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            *self.state.write() = WindowState::Hidden;
            println!("✅ Window hidden (simulated)");
        }
        
        Ok(())
    }

    // Hide window and deactivate the app, blocking until done on the main thread
    pub fn hide_and_deactivate_blocking(&self) -> VoicyResult<()> {
        println!("🪟 Hiding window and deactivating app (blocking)");

        #[cfg(target_os = "macos")]
        {
            use std::sync::mpsc;
            use std::time::Duration;

            let (tx, rx) = mpsc::channel::<()>();
            let state = self.state.clone();

            Queue::main().exec_async(move || {
                if let Err(e) = hide_window_macos() {
                    eprintln!("❌ Failed to hide window: {}", e);
                    let _ = tx.send(());
                    return;
                }
                // Deactivate the app so the previous app regains focus
                if let Err(e) = deactivate_app_macos() {
                    eprintln!("⚠️ Failed to deactivate app: {}", e);
                }
                *state.write() = WindowState::Hidden;
                println!("✅ Window hidden and app deactivated");
                let _ = tx.send(());
            });

            // Wait briefly for the hide/deactivate to complete
            let _ = rx.recv_timeout(Duration::from_millis(250));
        }

        #[cfg(not(target_os = "macos"))]
        {
            *self.state.write() = WindowState::Hidden;
        }

        Ok(())
    }
    
    pub fn hide_direct(&self) -> VoicyResult<()> {
        #[cfg(target_os = "macos")]
        {
            hide_window_macos()?;
            *self.state.write() = WindowState::Hidden;
        }
        
        #[cfg(not(target_os = "macos"))]
        {
            *self.state.write() = WindowState::Hidden;
        }
        
        Ok(())
    }
    
    pub fn is_visible(&self) -> bool {
        *self.state.read() == WindowState::Visible
    }
    
    pub fn get_state(&self) -> WindowState {
        *self.state.read()
    }

    pub fn focus_preferences() -> VoicyResult<()> {
        #[cfg(target_os = "macos")]
        {
            Queue::main().exec_async(move || {
                if let Err(e) = focus_preferences_window_macos() {
                    eprintln!("❌ Failed to focus preferences window: {}", e);
                }
            });
        }

        #[cfg(not(target_os = "macos"))]
        {
            // No-op on non-macOS
        }

        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn setup_window_properties_macos() -> VoicyResult<()> {
    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        
        if count > 0 {
            let window: id = msg_send![windows, objectAtIndex:0];
            
            // Set window level to floating (always on top)
            const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
            let _: () = msg_send![window, setLevel:NS_FLOATING_WINDOW_LEVEL];
            
            // Make window non-resizable
            let style_mask: i64 = msg_send![window, styleMask];
            let new_style = style_mask & !8; // Remove NSWindowStyleMaskResizable
            let _: () = msg_send![window, setStyleMask:new_style];
            
            // Ensure window stays on all spaces/desktops
            let collection_behavior: i64 = 1 << 0 | 1 << 8; // NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorStationary
            let _: () = msg_send![window, setCollectionBehavior:collection_behavior];
            
            // DO NOT ignore mouse events - we need rendering to work
            // const YES: bool = true;
            // let _: () = msg_send![window, setIgnoresMouseEvents:YES];
            
            println!("✅ Window configured: always on top, non-interactive, no focus steal");
        }
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_window_macos() -> VoicyResult<()> {
    unsafe {
        let app: id = NSApp();
        if app.is_null() {
            return Err(VoicyError::WindowOperationFailed("Failed to get NSApp".to_string()));
        }
        
        let windows: id = msg_send![app, windows];
        if windows.is_null() {
            return Err(VoicyError::WindowOperationFailed("No windows available".to_string()));
        }
        
        let count: usize = msg_send![windows, count];
        if count > 0 {
            let window: id = msg_send![windows, objectAtIndex:0];
            
            // Set floating level
            const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
            let _: () = msg_send![window, setLevel:NS_FLOATING_WINDOW_LEVEL];
            
            // Show without stealing focus
            let _: () = msg_send![window, orderFrontRegardless];
        }
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn hide_window_macos() -> VoicyResult<()> {
    unsafe {
        let app: id = NSApp();
        if app.is_null() {
            return Ok(());
        }
        
        let windows: id = msg_send![app, windows];
        if windows.is_null() {
            return Ok(());
        }
        
        let count: usize = msg_send![windows, count];
        if count > 0 {
            let window: id = msg_send![windows, objectAtIndex:0];
            let _: () = msg_send![window, orderOut:nil];
        }
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn deactivate_app_macos() -> VoicyResult<()> {
    unsafe {
        let app: id = NSApp();
        if app.is_null() {
            return Ok(());
        }
        let _: () = msg_send![app, deactivate];
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn focus_preferences_window_macos() -> VoicyResult<()> {
    unsafe {
        let app: id = NSApp();
        if app.is_null() { return Ok(()); }

        let windows: id = msg_send![app, windows];
        if windows.is_null() { return Ok(()); }

        let count: usize = msg_send![windows, count];
        if count == 0 { return Ok(()); }

        // NSWindowStyleMaskTitled == 1 << 0
        const NS_WINDOW_STYLE_MASK_TITLED: i64 = 1;

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex:i];
            let style_mask: i64 = msg_send![window, styleMask];
            let has_title = (style_mask & NS_WINDOW_STYLE_MASK_TITLED) != 0;
            // Skip floating pop-up/status windows (recording state window)
            let level: i64 = msg_send![window, level];
            const NS_FLOATING_WINDOW_LEVEL: i64 = 3;
            let is_floating = level == NS_FLOATING_WINDOW_LEVEL;
            if has_title && !is_floating {
                // Bring to front and make key
                let _: () = msg_send![window, makeKeyAndOrderFront:nil];
                // Activate the app to ensure visibility
                let _: () = msg_send![app, activateIgnoringOtherApps:true];
                break;
            }
        }
    }

    Ok(())
}
