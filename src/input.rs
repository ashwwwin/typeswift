use crate::config::HotkeyConfig;
use crate::error::{VoicyError, VoicyResult};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotkeyEvent {
    PushToTalkPressed,
    PushToTalkReleased,
    ToggleWindow,
}

pub struct HotkeyHandler {
    manager: GlobalHotKeyManager,
    toggle_hotkey: Option<HotKey>,
    push_to_talk_hotkey: Option<HotKey>,
}

impl HotkeyHandler {
    pub fn new() -> VoicyResult<Self> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| VoicyError::HotkeyRegistrationFailed(format!("Failed to create manager: {}", e)))?;

        Ok(Self {
            manager,
            toggle_hotkey: None,
            push_to_talk_hotkey: None,
        })
    }

    pub fn register_hotkeys(&mut self, config: &HotkeyConfig) -> VoicyResult<()> {
        // Clear existing hotkeys individually
        if let Some(ref hotkey) = self.toggle_hotkey {
            let _ = self.manager.unregister(hotkey.clone());
        }
        if let Some(ref hotkey) = self.push_to_talk_hotkey {
            let _ = self.manager.unregister(hotkey.clone());
        }

        let push_to_talk_hotkey = parse_hotkey(&config.push_to_talk)?;
        self.manager.register(push_to_talk_hotkey.clone())
            .map_err(|e| VoicyError::HotkeyRegistrationFailed(format!("Failed to register push-to-talk: {}", e)))?;
        self.push_to_talk_hotkey = Some(push_to_talk_hotkey);
        println!("✅ Registered push-to-talk: {} (hold to record)", config.push_to_talk);

        if let Some(ref toggle_key) = config.toggle_window {
            let toggle_hotkey = parse_hotkey(toggle_key)?;
            self.manager.register(toggle_hotkey.clone())
                .map_err(|e| VoicyError::HotkeyRegistrationFailed(format!("Failed to register toggle: {}", e)))?;
            self.toggle_hotkey = Some(toggle_hotkey);
            println!("✅ Registered toggle window: {}", toggle_key);
        }

        Ok(())
    }

    pub fn start_event_loop(&self) -> Receiver<HotkeyEvent> {
        let (sender, receiver) = channel();
        let toggle_hotkey = self.toggle_hotkey.clone();
        let push_to_talk_hotkey = self.push_to_talk_hotkey.clone();
        let is_push_to_talk_active = Arc::new(Mutex::new(false));

        thread::spawn(move || {
            println!("🚀 Starting hotkey event loop thread");
            loop {
                if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                    println!("🔑 Received hotkey event: {:?}", event);
                    
                    match event.state {
                        HotKeyState::Pressed => {
                            if let Some(hotkey_event) = handle_hotkey_press(
                                event.id,
                                &toggle_hotkey,
                                &push_to_talk_hotkey,
                                &is_push_to_talk_active,
                            ) {
                                println!("📤 Sending event: {:?}", hotkey_event);
                                if let Err(e) = sender.send(hotkey_event) {
                                    eprintln!("❌ Failed to send hotkey event: {}", e);
                                }
                            }
                        }
                        HotKeyState::Released => {
                            if let Some(hotkey_event) = handle_hotkey_release(
                                event.id,
                                &push_to_talk_hotkey,
                                &is_push_to_talk_active,
                            ) {
                                println!("📤 Sending event: {:?}", hotkey_event);
                                if let Err(e) = sender.send(hotkey_event) {
                                    eprintln!("❌ Failed to send hotkey event: {}", e);
                                }
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(10));
            }
        });

        receiver
    }
}

fn handle_hotkey_press(
    hotkey_id: u32,
    toggle_hotkey: &Option<HotKey>,
    push_to_talk_hotkey: &Option<HotKey>,
    is_push_to_talk_active: &Arc<Mutex<bool>>,
) -> Option<HotkeyEvent> {
    if let Some(ptt) = push_to_talk_hotkey {
        if ptt.id() == hotkey_id {
            let mut is_active = is_push_to_talk_active.lock().unwrap();
            if !*is_active {
                *is_active = true;
                println!("🎙️ Push-to-talk PRESSED");
                return Some(HotkeyEvent::PushToTalkPressed);
            }
        }
    }

    if let Some(toggle) = toggle_hotkey {
        if toggle.id() == hotkey_id {
            println!("🔄 Toggle window hotkey pressed");
            return Some(HotkeyEvent::ToggleWindow);
        }
    }
    
    None
}

fn handle_hotkey_release(
    hotkey_id: u32,
    push_to_talk_hotkey: &Option<HotKey>,
    is_push_to_talk_active: &Arc<Mutex<bool>>,
) -> Option<HotkeyEvent> {
    if let Some(ptt) = push_to_talk_hotkey {
        if ptt.id() == hotkey_id {
            let mut is_active = is_push_to_talk_active.lock().unwrap();
            if *is_active {
                *is_active = false;
                println!("🛑 Push-to-talk RELEASED");
                return Some(HotkeyEvent::PushToTalkReleased);
            }
        }
    }
    
    None
}

fn parse_hotkey(hotkey_str: &str) -> VoicyResult<HotKey> {
    let parts: Vec<&str> = hotkey_str.split('+').collect();
    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "cmd" | "command" | "meta" => {
                #[cfg(target_os = "macos")]
                {
                    modifiers |= Modifiers::SUPER;
                }
                #[cfg(not(target_os = "macos"))]
                {
                    modifiers |= Modifiers::CONTROL;
                }
            }
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" | "opt" => modifiers |= Modifiers::ALT,  // Support Option key
            "shift" => modifiers |= Modifiers::SHIFT,
            "super" | "win" => modifiers |= Modifiers::SUPER,
            key => {
                key_code = Some(parse_key_code(key)?);
            }
        }
    }

    let key_code = key_code.ok_or_else(|| {
        VoicyError::HotkeyRegistrationFailed("No key specified in hotkey".to_string())
    })?;

    Ok(HotKey::new(Some(modifiers), key_code))
}

fn parse_key_code(key: &str) -> VoicyResult<Code> {
    let code = match key.to_lowercase().as_str() {
        "a" => Code::KeyA, "b" => Code::KeyB, "c" => Code::KeyC, "d" => Code::KeyD,
        "e" => Code::KeyE, "f" => Code::KeyF, "g" => Code::KeyG, "h" => Code::KeyH,
        "i" => Code::KeyI, "j" => Code::KeyJ, "k" => Code::KeyK, "l" => Code::KeyL,
        "m" => Code::KeyM, "n" => Code::KeyN, "o" => Code::KeyO, "p" => Code::KeyP,
        "q" => Code::KeyQ, "r" => Code::KeyR, "s" => Code::KeyS, "t" => Code::KeyT,
        "u" => Code::KeyU, "v" => Code::KeyV, "w" => Code::KeyW, "x" => Code::KeyX,
        "y" => Code::KeyY, "z" => Code::KeyZ,
        "0" => Code::Digit0, "1" => Code::Digit1, "2" => Code::Digit2, "3" => Code::Digit3,
        "4" => Code::Digit4, "5" => Code::Digit5, "6" => Code::Digit6, "7" => Code::Digit7,
        "8" => Code::Digit8, "9" => Code::Digit9,
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "backspace" => Code::Backspace,
        "delete" => Code::Delete,
        "f1" => Code::F1, "f2" => Code::F2, "f3" => Code::F3, "f4" => Code::F4,
        "f5" => Code::F5, "f6" => Code::F6, "f7" => Code::F7, "f8" => Code::F8,
        "f9" => Code::F9, "f10" => Code::F10, "f11" => Code::F11, "f12" => Code::F12,
        "f13" => Code::F13, "f14" => Code::F14, "f15" => Code::F15, "f16" => Code::F16,
        "f17" => Code::F17, "f18" => Code::F18, "f19" => Code::F19, "f20" => Code::F20,
        "f21" => Code::F21, "f22" => Code::F22, "f23" => Code::F23, "f24" => Code::F24,
        "globe" | "fn" | "function" => Code::Fn,
        "left" | "arrowleft" => Code::ArrowLeft,
        "right" | "arrowright" => Code::ArrowRight,
        "up" | "arrowup" => Code::ArrowUp,
        "down" | "arrowdown" => Code::ArrowDown,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" => Code::PageUp,
        "pagedown" => Code::PageDown,
        "insert" => Code::Insert,
        "capslock" => Code::CapsLock,
        "numlock" => Code::NumLock,
        "scrolllock" => Code::ScrollLock,
        "pause" => Code::Pause,
        "printscreen" => Code::PrintScreen,
        "comma" | "," => Code::Comma,
        "period" | "." => Code::Period,
        "slash" | "/" => Code::Slash,
        "semicolon" | ";" => Code::Semicolon,
        "quote" | "'" => Code::Quote,
        "bracket_left" | "[" => Code::BracketLeft,
        "bracket_right" | "]" => Code::BracketRight,
        "backslash" | "\\" => Code::Backslash,
        "minus" | "-" => Code::Minus,
        "equal" | "=" => Code::Equal,
        "backquote" | "`" => Code::Backquote,
        _ => return Err(VoicyError::HotkeyRegistrationFailed(format!("Unknown key: {}", key))),
    };
    Ok(code)
}