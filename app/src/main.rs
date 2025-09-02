// Use the library crate modules

use voicy::config::Config;
use gpui::{
    div, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds,
    WindowOptions,
};
use voicy::input::{HotkeyEvent, HotkeyHandler};
use voicy::controller::AppController;
use voicy::state::{AppStateManager, RecordingState};
use std::sync::{Arc, Mutex};
use voicy::window::WindowManager;
use crossbeam_channel::bounded;
#[cfg(target_os = "macos")]
use voicy::platform::macos::ffi as menubar_ffi;

struct VoicyView {
    state: AppStateManager,
}

impl Render for VoicyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Just render, no polling here

        let recording_state = self.state.get_recording_state();
        let transcription = self.state.get_transcription();

        let status_text = match recording_state {
            RecordingState::Idle => "Ready".to_string(),
            RecordingState::Recording => {
                if transcription.is_empty() {
                    "Listening...".to_string()
                } else {
                    transcription.clone()
                }
            }
            RecordingState::Processing => "Processing...".to_string(),
        };

        let bg_color = match recording_state {
            RecordingState::Idle => rgb(0x1f2937),
            RecordingState::Recording => rgb(0xdc2626),
            RecordingState::Processing => rgb(0x3b82f6),
        };

        div()
            .id("voicy-main")
            .flex()
            .flex_col()
            .bg(bg_color)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(match recording_state {
                RecordingState::Idle => rgb(0x374151),
                RecordingState::Recording => rgb(0xef4444),
                RecordingState::Processing => rgb(0x60a5fa),
            })
            .text_xs()
            .text_color(rgb(0xffffff))
            .child(status_text)
    }
}

fn main() {
    // Initialize logging
    {
        use tracing_subscriber::{EnvFilter, fmt};
        let _ = fmt().with_env_filter(EnvFilter::from_default_env()).try_init();
    }

    // Load configuration
    let config = Config::load().unwrap_or_default();

    // Initialize hotkey handler
    let mut hotkey_handler = HotkeyHandler::new().expect("Failed to create hotkey handler");

    // Register hotkeys
    if let Err(e) = hotkey_handler.register_hotkeys(&config.hotkeys) {
        eprintln!("⚠️ Failed to register hotkeys: {}", e);
        return;
    }

    // Start the hotkey event loop
    let hotkey_receiver = hotkey_handler.start_event_loop();

    // Clone config for the closure
    let config_clone = config.clone();

    // Set environment variable to hide dock icon
    unsafe {
        std::env::set_var("GPUI_HIDE_DOCK", "1");
    }

    Application::new().run(move |cx: &mut App| {
        // Initialize menu bar and hide dock icon AFTER GPUI starts
        // Try multiple times to ensure it sticks
        std::thread::spawn(|| {
            for i in 0..5 {
                std::thread::sleep(std::time::Duration::from_millis(100 * i));
                menubar_ffi::MenuBarController::hide_dock_icon();
                if i == 0 {
                    menubar_ffi::MenuBarController::setup();
                }
            }
        });

        let window_size = size(
            px(config_clone.ui.window_width),
            px(config_clone.ui.window_height),
        );
        let gap_from_bottom = px(config_clone.ui.gap_from_bottom);

        // Get the primary display
        let displays = cx.displays();
        let screen = displays.first().expect("No displays found");

        // Calculate position for bottom center with gap
        let bounds = Bounds {
            origin: point(
                screen.bounds().center().x - window_size.width / 2.,
                screen.bounds().size.height - window_size.height - gap_from_bottom,
            ),
            size: window_size,
        };

        // Create a single event channel for the controller
        let (event_tx, event_rx) = bounded::<HotkeyEvent>(256);

        let window = cx
            .open_window(
                WindowOptions {
                    is_movable: false,
                    titlebar: None,
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    display_id: Some(screen.id()),
                    focus: false,
                    show: false, // Must be visible for render to be called
                    kind: gpui::WindowKind::PopUp,
                    ..Default::default()
                },
                move |_window, cx| {
                    let state = AppStateManager::new();
                    cx.new(|_cx| VoicyView { state })
                },
            )
            .unwrap();

        let _window_for_callback = window.clone();

        // Forward hotkeys to controller channel directly
        let tx_for_hotkeys = event_tx.clone();
        std::thread::spawn(move || {
            println!("🔄 Hotkey forwarder started");
            while let Ok(event) = hotkey_receiver.recv() {
                let _ = tx_for_hotkeys.send(event);
            }
            println!("🛑 Hotkey forwarder stopped");
        });

        // Set up window properties
        if let Err(e) = WindowManager::setup_properties() {
            eprintln!("⚠️ Failed to setup window properties: {}", e);
        }

        // Start the controller after window setup so show/hide works
        let controller = AppController::new(config_clone.clone());
        // Share state between UI and controller
        let state_for_view = controller.state();
        // Replace the VoicyView's state with the controller's state
        // by re-rendering the window content with the shared state.
        window.update(cx, move |voicy_view, _window, _cx| {
            voicy_view.state = state_for_view.clone();
        });

        // Apply window properties (always-on-top, etc.)
        
        println!("🚀 Voicy started with global shortcuts:");
        println!(
            "   Push-to-talk: {} (hold to record)",
            config_clone.hotkeys.push_to_talk
        );
        if let Some(ref key) = config_clone.hotkeys.toggle_window {
            println!("   Toggle window: {}", key);
        }
        println!("✅ Hotkeys forwarding independently of UI");

        // Run controller in background, consuming forwarded events
        controller.start(event_rx);
    });
}
