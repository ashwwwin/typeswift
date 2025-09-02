// Use the library crate modules

use voicy::config::Config;
use gpui::{
    div, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds,
    WindowOptions, Timer,
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
    config: std::sync::Arc<parking_lot::RwLock<voicy::config::Config>>,
}

struct PreferencesView {
    config: std::sync::Arc<parking_lot::RwLock<voicy::config::Config>>,
    open_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle_holder: std::sync::Arc<std::sync::Mutex<Option<gpui::WindowHandle<PreferencesView>>>>,
    hotkeys: std::sync::Arc<std::sync::Mutex<voicy::input::HotkeyHandler>>,
    rev: u64,
}

impl Drop for PreferencesView {
    fn drop(&mut self) {
        self.open_flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Render for VoicyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        {
            // Status view
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
}

impl Render for PreferencesView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cfg = self.config.read();
        let typing_enabled = cfg.output.enable_typing;
        let add_space = cfg.output.add_space_between_utterances;
        let streaming_enabled = cfg.streaming.enabled;
        let ptt = cfg.hotkeys.push_to_talk.clone();
        let toggle = cfg.hotkeys.toggle_window.clone();
        drop(cfg);

        let toggle_typing = {
            let config = self.config.clone();
            move || {
                let mut cfg = config.write();
                cfg.output.enable_typing = !cfg.output.enable_typing;
                if let Some(path) = voicy::config::Config::config_path() {
                    let _ = cfg.save(path);
                }
            }
        };
        let toggle_add_space = {
            let config = self.config.clone();
            move || {
                let mut cfg = config.write();
                cfg.output.add_space_between_utterances = !cfg.output.add_space_between_utterances;
                if let Some(path) = voicy::config::Config::config_path() {
                    let _ = cfg.save(path);
                }
            }
        };
        let toggle_streaming = {
            let config = self.config.clone();
            move || {
                let mut cfg = config.write();
                cfg.streaming.enabled = !cfg.streaming.enabled;
                if let Some(path) = voicy::config::Config::config_path() {
                    let _ = cfg.save(path);
                }
            }
        };

        let handle_holder = self.handle_holder.clone();

        let typing_row = {
            let config = self.config.clone();
            let handle_holder = self.handle_holder.clone();
            div()
                .w_full()
                .p(px(6.0))
                .border_b_1()
                .border_color(rgb(0x374151))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x9ca3af))
                        .bg(if typing_enabled { rgb(0x2563eb) } else { rgb(0x111827) })
                        .child(if typing_enabled { "✓" } else { "" }),
                )
                .child("Enable typing")
                .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                    // Update in-memory config
                    let mut cfg = config.write();
                    cfg.output.enable_typing = !cfg.output.enable_typing;
                    let to_save = cfg.clone();
                    drop(cfg);
                    // Save async
                    if let Some(path) = voicy::config::Config::config_path() {
                        std::thread::spawn(move || { let _ = to_save.save(path); });
                    }
                    // Re-render
                    if let Some(handle) = handle_holder.lock().unwrap().clone() {
                        let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                    }
                })
        };

        let handle_holder2 = self.handle_holder.clone();

        let add_space_row = {
            let config = self.config.clone();
            let handle_holder2 = self.handle_holder.clone();
            div()
                .w_full()
                .p(px(6.0))
                .border_b_1()
                .border_color(rgb(0x374151))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x9ca3af))
                        .bg(if add_space { rgb(0x2563eb) } else { rgb(0x111827) })
                        .child(if add_space { "✓" } else { "" }),
                )
                .child("Add space between utterances")
                .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                    let mut cfg = config.write();
                    cfg.output.add_space_between_utterances = !cfg.output.add_space_between_utterances;
                    let to_save = cfg.clone();
                    drop(cfg);
                    if let Some(path) = voicy::config::Config::config_path() {
                        std::thread::spawn(move || { let _ = to_save.save(path); });
                    }
                    if let Some(handle) = handle_holder2.lock().unwrap().clone() {
                        let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                    }
                })
        };

        let handle_holder3 = self.handle_holder.clone();

        let streaming_row = {
            let config = self.config.clone();
            let handle_holder3 = self.handle_holder.clone();
            div()
                .w_full()
                .p(px(6.0))
                .border_b_1()
                .border_color(rgb(0x374151))
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .size(px(16.0))
                        .rounded_sm()
                        .border_1()
                        .border_color(rgb(0x9ca3af))
                        .bg(if streaming_enabled { rgb(0x2563eb) } else { rgb(0x111827) })
                        .child(if streaming_enabled { "✓" } else { "" }),
                )
                .child("Enable streaming")
                .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                    let mut cfg = config.write();
                    cfg.streaming.enabled = !cfg.streaming.enabled;
                    let to_save = cfg.clone();
                    drop(cfg);
                    if let Some(path) = voicy::config::Config::config_path() {
                        std::thread::spawn(move || { let _ = to_save.save(path); });
                    }
                    if let Some(handle) = handle_holder3.lock().unwrap().clone() {
                        let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                    }
                })
        };

        // Hotkey helpers (reopen window after saving)
        let handle_holder4 = self.handle_holder.clone();
        let cfg_arc4 = self.config.clone();
        let hk4 = self.hotkeys.clone();
        let set_ptt_fn = div()
            .px(px(6.0))
            .py(px(4.0))
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x374151))
            .cursor_pointer()
            .child("Set Fn")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc4.write();
                cfg.hotkeys.push_to_talk = "fn".to_string();
                if let Some(path) = voicy::config::Config::config_path() { let _ = cfg.save(path); }
                // Apply hotkeys immediately
                if let Ok(mut hk) = hk4.lock() {
                    let _ = hk.register_hotkeys(&cfg.hotkeys);
                }
                if let Some(handle) = handle_holder4.lock().unwrap().clone() {
                    let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                }
            });

        let handle_holder5 = self.handle_holder.clone();
        let cfg_arc5 = self.config.clone();
        let hk5 = self.hotkeys.clone();
        let set_ptt_cmd_space = div()
            .px(px(6.0)).py(px(4.0)).rounded_sm().border_1().border_color(rgb(0x374151)).cursor_pointer()
            .child("Set Cmd+Space")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc5.write();
                cfg.hotkeys.push_to_talk = "cmd+space".to_string();
                if let Some(path) = voicy::config::Config::config_path() { let _ = cfg.save(path); }
                if let Ok(mut hk) = hk5.lock() {
                    let _ = hk.register_hotkeys(&cfg.hotkeys);
                }
                if let Some(handle) = handle_holder5.lock().unwrap().clone() {
                    let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                }
            });

        let handle_holder6 = self.handle_holder.clone();
        let cfg_arc6 = self.config.clone();
        let hk6 = self.hotkeys.clone();
        let set_ptt_opt_space = div()
            .px(px(6.0)).py(px(4.0)).rounded_sm().border_1().border_color(rgb(0x374151)).cursor_pointer()
            .child("Set Opt+Space")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc6.write();
                cfg.hotkeys.push_to_talk = "opt+space".to_string();
                if let Some(path) = voicy::config::Config::config_path() { let _ = cfg.save(path); }
                if let Ok(mut hk) = hk6.lock() {
                    let _ = hk.register_hotkeys(&cfg.hotkeys);
                }
                if let Some(handle) = handle_holder6.lock().unwrap().clone() {
                    let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                }
            });

        // Toggle window preset
        let handle_holder7 = self.handle_holder.clone();
        let cfg_arc7 = self.config.clone();
        let hk7 = self.hotkeys.clone();
        let set_toggle_cmd_shift_bslash = div()
            .px(px(6.0)).py(px(4.0)).rounded_sm().border_1().border_color(rgb(0x374151)).cursor_pointer()
            .child("Toggle: Cmd+Shift+\\")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc7.write();
                cfg.hotkeys.toggle_window = Some("cmd+shift+\\".to_string());
                if let Some(path) = voicy::config::Config::config_path() { let _ = cfg.save(path); }
                if let Ok(mut hk) = hk7.lock() {
                    let _ = hk.register_hotkeys(&cfg.hotkeys);
                }
                if let Some(handle) = handle_holder7.lock().unwrap().clone() {
                    let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                }
            });

        let handle_holder8 = self.handle_holder.clone();
        let cfg_arc8 = self.config.clone();
        let hk8 = self.hotkeys.clone();
        let clear_toggle = div()
            .px(px(6.0)).py(px(4.0)).rounded_sm().border_1().border_color(rgb(0x374151)).cursor_pointer()
            .child("Clear Toggle")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc8.write();
                cfg.hotkeys.toggle_window = None;
                if let Some(path) = voicy::config::Config::config_path() { let _ = cfg.save(path); }
                if let Ok(mut hk) = hk8.lock() {
                    let _ = hk.register_hotkeys(&cfg.hotkeys);
                }
                if let Some(handle) = handle_holder8.lock().unwrap().clone() {
                    let _ = handle.update(app_cx, |view, _w, _cx| { view.rev = view.rev.wrapping_add(1); });
                }
            });


        div()
            .id("voicy-prefs-window")
            .flex()
            .flex_col()
            .bg(rgb(0x111827))
            .w_full()
            .h_full()
            .p(px(8.0))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x374151))
            .text_xs()
            .gap(px(6.0))
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .child(div().text_sm().child("Voicy Preferences"))
            )
            .child(typing_row)
            .child(add_space_row)
            .child(streaming_row)
            .child(div().mt(px(8.0)).child(format!("Push-to-talk: {}", ptt)))
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .child(set_ptt_fn)
                    .child(set_ptt_cmd_space)
                    .child(set_ptt_opt_space),
            )
            .child(div().mt(px(8.0)).child(format!(
                "Toggle window: {}",
                toggle.clone().unwrap_or_else(|| "None".into())
            )))
            .child(div().flex().gap(px(6.0)).child(set_toggle_cmd_shift_bslash).child(clear_toggle))
            .child(div().mt(px(6.0)).child(
                "Tip: Click a row to toggle. Close this window when done.",
            ))
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

    // Wrap handler to allow live re-registration
    let hotkey_handler = std::sync::Arc::new(std::sync::Mutex::new(hotkey_handler));
    // Start the hotkey event loop
    let hotkey_receiver = hotkey_handler.lock().unwrap().start_event_loop();

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

        // Create event channels for the controller and UI
        let (event_tx, event_rx) = bounded::<HotkeyEvent>(256);
        let (ui_tx, ui_rx) = bounded::<HotkeyEvent>(64);
        #[cfg(target_os = "macos")]
        {
            // Wire Preferences menu item to controller via callback
            use std::sync::mpsc;
            let (prefs_tx, prefs_rx) = mpsc::channel::<HotkeyEvent>();
            menubar_ffi::register_preferences_callback(prefs_tx);
            let event_tx_clone = event_tx.clone();
            let ui_tx_prefs = ui_tx.clone();
            std::thread::spawn(move || {
                while let Ok(ev) = prefs_rx.recv() {
                    let _ = event_tx_clone.send(ev);
                    let _ = ui_tx_prefs.send(ev);
                }
            });
        }

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
                    let config_arc = std::sync::Arc::new(parking_lot::RwLock::new(
                        voicy::config::Config::load().unwrap_or_default(),
                    ));
                    cx.new(|_cx| VoicyView { state, config: config_arc })
                },
            )
            .unwrap();

        let _window_for_callback = window.clone();

        // Forward hotkeys to controller and UI
        let tx_for_hotkeys = event_tx.clone();
        let ui_tx_hotkeys = ui_tx.clone();
        std::thread::spawn(move || {
            println!("🔄 Hotkey forwarder started");
            while let Ok(event) = hotkey_receiver.recv() {
                let _ = tx_for_hotkeys.send(event);
                let _ = ui_tx_hotkeys.send(event);
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
        let config_handle_for_view = controller.config_handle();
        let prefs_config_handle = config_handle_for_view.clone();
        let _ = window.update(cx, move |voicy_view, _window, _cx| {
            voicy_view.state = state_for_view.clone();
            voicy_view.config = config_handle_for_view.clone();
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

        // Removed file watcher: config changes now apply immediately where edited (Preferences window and hotkey presets).

        // Run controller in background, consuming forwarded events
        controller.start(event_rx);

        // Preferences window opener: open separate window on OpenPreferences events
        let prefs_config = prefs_config_handle.clone();
        let prefs_open = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let prefs_open_for_view = prefs_open.clone();
        let hotkey_handler_for_prefs_outer = hotkey_handler.clone();
        cx.spawn(async move |cx| {
            use std::time::Duration;
            loop {
                if let Ok(ev) = ui_rx.try_recv() {
                    if let HotkeyEvent::OpenPreferences = ev {
                        if !prefs_open.load(std::sync::atomic::Ordering::SeqCst) {
                            prefs_open.store(true, std::sync::atomic::Ordering::SeqCst);
                            let prefs_config = prefs_config.clone();
                            let prefs_open_for_view = prefs_open_for_view.clone();
                            let hk_for_update = hotkey_handler_for_prefs_outer.clone();
                            let _ = cx.update(|cx| {
                                let bounds = Bounds::centered(
                                    None,
                                    size(px(320.0), px(220.0)),
                                    cx,
                                );
                                let handle_holder_outer: std::sync::Arc<std::sync::Mutex<Option<gpui::WindowHandle<PreferencesView>>>> =
                                    std::sync::Arc::new(std::sync::Mutex::new(None));
                                let holder_for_create = handle_holder_outer.clone();
                                let handle = cx.open_window(
                                    WindowOptions {
                                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                                        titlebar: Some(gpui::TitlebarOptions { appears_transparent: true, ..Default::default() }),
                                        focus: true,
                                        ..Default::default()
                                    },
                                    move |_, cx| {
                                        let open_flag = prefs_open_for_view.clone();
                                        let holder = holder_for_create.clone();
                                        let hk = hk_for_update.clone();
                                        cx.new(|_| PreferencesView { config: prefs_config.clone(), open_flag, handle_holder: holder, hotkeys: hk, rev: 0 })
                                    },
                                )
                                .unwrap();
                                *handle_holder_outer.lock().unwrap() = Some(handle.clone());
                                // Ensure the Preferences window is brought to front on first open
                                if let Err(e) = voicy::window::WindowManager::focus_preferences() {
                                    eprintln!("⚠️ Could not focus preferences window: {}", e);
                                }
                            });
                        } else {
                            // Already open: bring the Preferences window to front
                            if let Err(e) = voicy::window::WindowManager::focus_preferences() {
                                eprintln!("⚠️ Could not focus preferences window: {}", e);
                            }
                        }
                    }
                }
                Timer::after(Duration::from_millis(100)).await;
            }
        }).detach();
    });
}
