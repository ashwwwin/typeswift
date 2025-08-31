use std::sync::mpsc::Sender;
use crate::input::HotkeyEvent;

#[link(name = "VoicySwift")]
unsafe extern "C" {
    fn swift_init_keyboard_monitor() -> bool;
    fn swift_shutdown_keyboard_monitor();
    fn swift_register_push_to_talk_callback(callback: extern "C" fn(bool));
}

static mut PUSH_TO_TALK_SENDER: Option<Sender<HotkeyEvent>> = None;

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