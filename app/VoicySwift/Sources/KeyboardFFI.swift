import Foundation

private var pushToTalkCallback: ((Bool) -> Void)?

@_cdecl("swift_init_keyboard_monitor")
public func swift_init_keyboard_monitor() -> Bool {
    return VoicyKeyboardMonitor.initializeKeyboardMonitor()
}

@_cdecl("swift_shutdown_keyboard_monitor")
public func swift_shutdown_keyboard_monitor() {
    VoicyKeyboardMonitor.shutdownKeyboardMonitor()
}

@_cdecl("swift_register_push_to_talk_callback")
public func swift_register_push_to_talk_callback(callback: @escaping @convention(c) (Bool) -> Void) {
    pushToTalkCallback = { isPressed in
        callback(isPressed)
    }
    
    // Register for notifications
    NotificationCenter.default.addObserver(
        forName: NSNotification.Name("VoicyPushToTalkPressed"),
        object: nil,
        queue: .main
    ) { _ in
        pushToTalkCallback?(true)
    }
    
    NotificationCenter.default.addObserver(
        forName: NSNotification.Name("VoicyPushToTalkReleased"),
        object: nil,
        queue: .main
    ) { _ in
        pushToTalkCallback?(false)
    }
}