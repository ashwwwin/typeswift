import Foundation
import AppKit

// Preferences callback wire-up
private var preferencesCallback: (() -> Void)?

@_cdecl("swift_register_preferences_callback")
public func swift_register_preferences_callback(_ callback: @escaping @convention(c) () -> Void) {
    preferencesCallback = {
        callback()
    }
    // Register for Preferences notifications from the menu
    NotificationCenter.default.addObserver(
        forName: NSNotification.Name("VoicyOpenPreferences"),
        object: nil,
        queue: .main
    ) { _ in
        preferencesCallback?()
    }
}

// FFI exports for menu bar functionality

@_cdecl("voicy_setup_menubar")
public func voicy_setup_menubar() {
    DispatchQueue.main.async {
        VoicyMenuBar.shared.setupMenuBar()
    }
}

@_cdecl("voicy_hide_dock_icon")
public func voicy_hide_dock_icon() {
    // Try both sync and async approaches
    NSApp.setActivationPolicy(.accessory)
    
    DispatchQueue.main.async {
        NSApp.setActivationPolicy(.accessory)
        // Also try to activate the change
        NSApp.activate(ignoringOtherApps: false)
    }
    
    print("🔄 Dock icon hidden, activation policy set to accessory")
}

@_cdecl("voicy_show_dock_icon")
public func voicy_show_dock_icon() {
    DispatchQueue.main.async {
        NSApp.setActivationPolicy(.regular)
    }
}

@_cdecl("voicy_set_menu_status")
public func voicy_set_menu_status(_ text: UnsafePointer<CChar>) {
    let statusText = String(cString: text)
    VoicyMenuBar.shared.setStatusText(statusText)
}

@_cdecl("voicy_show_notification")
public func voicy_show_notification(_ title: UnsafePointer<CChar>, _ message: UnsafePointer<CChar>) {
    let titleStr = String(cString: title)
    let messageStr = String(cString: message)
    VoicyMenuBar.shared.showNotification(title: titleStr, text: messageStr)
}

@_cdecl("voicy_set_recording_state")
public func voicy_set_recording_state(_ isRecording: Bool) {
    DispatchQueue.main.async {
        if isRecording {
            VoicyMenuBar.shared.setStatusIcon(systemName: "mic.circle.fill")
        } else {
            VoicyMenuBar.shared.setStatusIcon(systemName: "mic.fill")
        }
    }
}

@_cdecl("voicy_run_app")
public func voicy_run_app() {
    // Ensure we're on the main thread
    if Thread.isMainThread {
        // Setup as menu bar app
        NSApp.setActivationPolicy(.accessory)
        
        // Setup menu bar
        VoicyMenuBar.shared.setupMenuBar()
        
        // Run the app
        NSApp.run()
    } else {
        DispatchQueue.main.sync {
            NSApp.setActivationPolicy(.accessory)
            VoicyMenuBar.shared.setupMenuBar()
            NSApp.run()
        }
    }
}

@_cdecl("voicy_terminate_app")
public func voicy_terminate_app() {
    DispatchQueue.main.async {
        NSApp.terminate(nil)
    }
}

@_cdecl("voicy_reset_first_launch")
public func voicy_reset_first_launch() {
    // Reset first launch flags for testing
    UserDefaults.standard.removeObject(forKey: "com.voicy.hasLaunchedBefore")
    UserDefaults.standard.removeObject(forKey: "com.voicy.hasAskedAboutLogin")
    UserDefaults.standard.synchronize()
    print("🔄 First launch state reset - next launch will show welcome dialog")
}
