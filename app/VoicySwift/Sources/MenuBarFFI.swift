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
        forName: NSNotification.Name("TypeswiftOpenPreferences"),
        object: nil,
        queue: .main
    ) { _ in
        preferencesCallback?()
    }
}

// FFI exports for menu bar functionality

@_cdecl("typeswift_setup_menubar")
public func typeswift_setup_menubar() {
    DispatchQueue.main.async {
        TypeswiftMenuBar.shared.setupMenuBar()
    }
}

@_cdecl("typeswift_hide_dock_icon")
public func typeswift_hide_dock_icon() {
    // Try both sync and async approaches
    NSApp.setActivationPolicy(.accessory)
    
    DispatchQueue.main.async {
        NSApp.setActivationPolicy(.accessory)
        // Also try to activate the change
        NSApp.activate(ignoringOtherApps: false)
    }
    
    print("Dock icon hidden, activation policy set to accessory")
}

@_cdecl("typeswift_show_dock_icon")
public func typeswift_show_dock_icon() {
    DispatchQueue.main.async {
        NSApp.setActivationPolicy(.regular)
    }
}

@_cdecl("typeswift_set_menu_status")
public func typeswift_set_menu_status(_ text: UnsafePointer<CChar>) {
    let statusText = String(cString: text)
    TypeswiftMenuBar.shared.setStatusText(statusText)
}

@_cdecl("typeswift_show_notification")
public func typeswift_show_notification(_ title: UnsafePointer<CChar>, _ message: UnsafePointer<CChar>) {
    let titleStr = String(cString: title)
    let messageStr = String(cString: message)
    TypeswiftMenuBar.shared.showNotification(title: titleStr, text: messageStr)
}

@_cdecl("typeswift_set_recording_state")
public func typeswift_set_recording_state(_ isRecording: Bool) {
    DispatchQueue.main.async {
        TypeswiftMenuBar.shared.setRecordingState(isRecording)
    }
}

@_cdecl("typeswift_run_app")
public func typeswift_run_app() {
    // Ensure we're on the main thread
    if Thread.isMainThread {
        // Setup as menu bar app
        NSApp.setActivationPolicy(.accessory)
        
        // Setup menu bar
        TypeswiftMenuBar.shared.setupMenuBar()
        
        // Run the app
        NSApp.run()
    } else {
        DispatchQueue.main.sync {
            NSApp.setActivationPolicy(.accessory)
            TypeswiftMenuBar.shared.setupMenuBar()
            NSApp.run()
        }
    }
}

@_cdecl("typeswift_terminate_app")
public func typeswift_terminate_app() {
    DispatchQueue.main.async {
        NSApp.terminate(nil)
    }
}

@_cdecl("typeswift_reset_first_launch")
public func typeswift_reset_first_launch() {
    // Reset first launch flags for testing
    UserDefaults.standard.removeObject(forKey: "com.voicy.hasLaunchedBefore")
    UserDefaults.standard.removeObject(forKey: "com.voicy.hasAskedAboutLogin")
    UserDefaults.standard.removeObject(forKey: "com.typeswift.hasLaunchedBefore")
    UserDefaults.standard.removeObject(forKey: "com.typeswift.hasAskedAboutLogin")
    UserDefaults.standard.synchronize()
    print("First launch state reset - next launch will show welcome dialog")
}

// MARK: - Launch at Login control (for Preferences window)

@_cdecl("typeswift_is_launch_at_login_enabled")
public func typeswift_is_launch_at_login_enabled() -> Bool {
    return TypeswiftMenuBar.shared.isLaunchAtStartupEnabled()
}

@_cdecl("typeswift_set_launch_at_login_enabled")
public func typeswift_set_launch_at_login_enabled(_ enabled: Bool) {
    if enabled {
        TypeswiftMenuBar.shared.enableLaunchAtStartup()
    } else {
        TypeswiftMenuBar.shared.disableLaunchAtStartup()
    }
}
