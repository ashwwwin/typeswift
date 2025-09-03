// Use the library crate modules

use voicy::config::Config;
use gpui::{
    div, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds,
    WindowOptions, Timer,
};
use voicy::input::{HotkeyEvent, HotkeyHandler};
use voicy::controller::AppController;
use voicy::state::{AppStateManager, RecordingState};
// use std::sync::{Arc, Mutex};
use voicy::window::WindowManager;
use crossbeam_channel::bounded;
#[cfg(target_os = "macos")]
use voicy::platform::macos::ffi as menubar_ffi;

struct TypeswiftView {
    state: AppStateManager,
    config: std::sync::Arc<parking_lot::RwLock<voicy::config::Config>>,
}

struct PreferencesView {
    config: std::sync::Arc<parking_lot::RwLock<voicy::config::Config>>,
    open_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle_holder: std::sync::Arc<std::sync::Mutex<Option<gpui::WindowHandle<PreferencesView>>>>,
    hotkeys: std::sync::Arc<std::sync::Mutex<voicy::input::HotkeyHandler>>,
    capture_focus: gpui::FocusHandle,
    capturing_ptt: bool,
    rev: u64,
}

impl Drop for PreferencesView {
    fn drop(&mut self) {
        self.open_flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Render for TypeswiftView {
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
                .id("typeswift-main")
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
        let ptt = cfg.hotkeys.push_to_talk.clone();
        drop(cfg);

        

        let typing_row = {
            let config = self.config.clone();
            let handle_holder = self.handle_holder.clone();
            div()
                .w_full()
                .mt(px(6.0))
                .px(px(6.0))
                .pt(px(2.0))
                .pb(px(1.0))
                .rounded_md()
                .hover(|s| s.bg(rgb(0x1f2937)))
                .flex()
                .items_center()
                .justify_between()
                .child(div().py(px(3.0)).child("Enable typing"))
                .child(
                    div()
                        // .rounded_md()
                        .text_color(if typing_enabled { rgb(0x065f46) } else { rgb(0x7f1d1d) })
                        .child(if typing_enabled { "On" } else { "Off" })
                )
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

        

        let add_space_row = {
            let config = self.config.clone();
            let handle_holder2 = self.handle_holder.clone();
            div()
                .w_full()
                .mt(px(3.0))
                .px(px(6.0))
                .pt(px(2.0))
                .pb(px(1.0))
                .rounded_md()
                .hover(|s| s.bg(rgb(0x1f2937)))
                .flex()
                .items_center()
                .justify_between()
                .child(div().py(px(3.0)).child("Add space between utterances"))
                .child(
                    div()
                        .text_color(if add_space { rgb(0x065f46) } else { rgb(0x7f1d1d) })
                        .child(if add_space { "On" } else { "Off" })
                )
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

        // Push-to-talk: capture shortcut inline
        let cfg_arc_cap = self.config.clone();
        let hk_cap = self.hotkeys.clone();
        let ptt_row = {
            let capturing_label_color = if self.capturing_ptt { rgb(0xf59e0b) } else { rgb(0x9ca3af) };
            div()
                .w_full()
                .mt(px(8.0))
                .px(px(6.0))
                .pt(px(2.0))
                .pb(px(1.0))
                .rounded_md()
                .hover(|s| s.bg(rgb(0x1f2937)))
                .flex()
                .items_center()
                .justify_between()
                .track_focus(&self.capture_focus)
                .on_key_down(_cx.listener(move |this, event: &gpui::KeyDownEvent, _window, _app_cx| {
                    if !this.capturing_ptt { return; }
                    let ks = &event.keystroke;
                    let key = ks.key.as_str();
                    if key.eq_ignore_ascii_case("escape") || key.eq_ignore_ascii_case("esc") {
                        this.capturing_ptt = false;
                        this.rev = this.rev.wrapping_add(1);
                        return;
                    }
                    if key.is_empty() { return; }
                    let mut parts: Vec<&str> = Vec::new();
                    if ks.modifiers.platform { parts.push("cmd"); }
                    if ks.modifiers.control { parts.push("ctrl"); }
                    if ks.modifiers.alt { parts.push("opt"); }
                    if ks.modifiers.shift { parts.push("shift"); }
                    let lower = key.to_lowercase();
                    let normalized_key = match lower.as_str() {
                        "meta" => "cmd",
                        "option" => "opt",
                        "return" => "enter",
                        other => other,
                    };
                    let mut composed = String::new();
                    for (i, p) in parts.iter().enumerate() { if i > 0 { composed.push('+'); } composed.push_str(p); }
                    if !parts.is_empty() { composed.push('+'); }
                    composed.push_str(normalized_key);

                    // Persist and apply
                    {
                        let mut cfg = cfg_arc_cap.write();
                        cfg.hotkeys.push_to_talk = composed.clone();
                        let to_save = cfg.clone();
                        drop(cfg);
                        if let Some(path) = voicy::config::Config::config_path() { let _ = to_save.save(path); }
                    }
                    if let Ok(mut hk) = hk_cap.lock() {
                        let _ = hk.register_hotkeys(&cfg_arc_cap.read().hotkeys);
                    }
                    this.capturing_ptt = false;
                    this.rev = this.rev.wrapping_add(1);
                }))
                .on_mouse_down(gpui::MouseButton::Left, _cx.listener(|this, _event, window, _app_cx| {
                    this.capturing_ptt = true;
                    this.rev = this.rev.wrapping_add(1);
                    this.capture_focus.focus(window);
                }))
                .child(div().py(px(3.0)).child("Push-to-talk shortcut"))
                .child(
                    div()
                        .text_color(capturing_label_color)
                        .child(if self.capturing_ptt { "Listening… (press keys or Esc)".to_string() } else { ptt.clone() })
                )
        };

        // Small helper for Fn-only capture
        let cfg_arc_fn = self.config.clone();
        let hk_fn = self.hotkeys.clone();
        let set_fn_button = div()
            .mt(px(4.0))
            .px(px(6.0))
            .py(px(4.0))
            .rounded_sm()
            .border_1()
            .border_color(rgb(0x374151))
            .hover(|s| s.bg(rgb(0x1f2937)))
            .child("Use Fn key")
            .on_mouse_down(gpui::MouseButton::Left, move |_, _window, app_cx| {
                let mut cfg = cfg_arc_fn.write();
                cfg.hotkeys.push_to_talk = "fn".to_string();
                let to_save = cfg.clone();
                drop(cfg);
                if let Some(path) = voicy::config::Config::config_path() { let _ = to_save.save(path); }
                if let Ok(mut hk) = hk_fn.lock() { let _ = hk.register_hotkeys(&to_save.hotkeys); }
                // Trigger a lightweight rerender via handle if present
                // (Preferences window updates via view.rev changes on next interactions)
                let _ = app_cx;
            });

        div()
            .id("typeswift-prefs-window")
            .flex()
            .flex_col()
            .bg(rgb(0x111827))
            .w_full()
            .h_full()
            .px(px(8.0))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x374151))
            .text_xs()
            // Remove inter-row gap to make rows sit flush
            // .gap(px(6.0))
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .w_full()
                    .flex()
                    .pt(px(5.0))
                    .justify_end()
                    .text_color(rgb(0x596678))
                    .child(div().text_xs().child("Typeswift"))
            )
            .child(typing_row)
            .child(add_space_row)
            .child(ptt_row)
            .child(set_fn_button)
            // .child(div().mt(px(6.0)).child(
            //     "Tip: Click a row to toggle. Close this window when done.",
            // ))
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
    std::env::set_var("GPUI_HIDE_DOCK", "1");

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
                    cx.new(|_cx| TypeswiftView { state, config: config_arc })
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
        
        println!("🚀 Typeswift started with global shortcuts:");
        println!(
            "   Push-to-talk: {} (hold to record)",
            config_clone.hotkeys.push_to_talk
        );
        // Toggle window hotkey setting removed from Preferences UI; still supported if present in config file.
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
                                        cx.new(|cx| PreferencesView { config: prefs_config.clone(), open_flag, handle_holder: holder, hotkeys: hk, capture_focus: cx.focus_handle(), capturing_ptt: false, rev: 0 })
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
