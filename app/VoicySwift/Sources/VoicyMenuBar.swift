import Foundation
import AppKit
import ServiceManagement

/// Menu bar controller for Typeswift
@objc public class TypeswiftMenuBar: NSObject {
    
    private var statusItem: NSStatusItem?
    private var menu: NSMenu?
    
    
    @objc public static let shared = TypeswiftMenuBar()
    
    private override init() {
        super.init()
    }
    
    /// Initialize the menu bar
    @objc public func setupMenuBar() {
        // Check if this is first launch
        checkFirstLaunch()
        // Create status item in system menu bar
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        
        // Set icon (you can use SF Symbols or custom image)
        if let button = statusItem?.button {
            // Using SF Symbol for microphone
            button.image = NSImage(systemSymbolName: "mic.fill", accessibilityDescription: "Typeswift")
            button.image?.size = NSSize(width: 18, height: 18)
            button.image?.isTemplate = true // Makes it adapt to dark/light mode
            
            // Alternative: Use text
            // button.title = "🎙️"
        }
        
        // Create menu
        menu = NSMenu()
        
        // Add menu items
        let titleItem = NSMenuItem(title: "Typeswift - Speech Recognition", action: nil, keyEquivalent: "")
        titleItem.isEnabled = false
        menu?.addItem(titleItem)
        
        menu?.addItem(NSMenuItem.separator())
        
        // Settings
        let settingsItem = NSMenuItem(title: "Preferences", action: #selector(openPreferences), keyEquivalent: "")
        settingsItem.target = self
        menu?.addItem(settingsItem)
        
        // Language info
        let languageItem = NSMenuItem(title: "Language: Auto-detect (25 languages)", action: nil, keyEquivalent: "")
        languageItem.isEnabled = false
        menu?.addItem(languageItem)
        
        menu?.addItem(NSMenuItem.separator())
        
        // About
        let aboutItem = NSMenuItem(title: "About Typeswift", action: #selector(showAbout), keyEquivalent: "")
        aboutItem.target = self
        menu?.addItem(aboutItem)
        
        menu?.addItem(NSMenuItem.separator())
        
        // Quit
        let quitItem = NSMenuItem(title: "Quit Typeswift", action: #selector(quitApp), keyEquivalent: "q")
        quitItem.target = self
        menu?.addItem(quitItem)
        
        // Assign menu to status item
        statusItem?.menu = menu
    }
    
    
    
    @objc private func openPreferences() {
        // Ensure app is active so the Preferences window can become key
        DispatchQueue.main.async {
            NSApp.activate(ignoringOtherApps: true)
        }
        // Notify Rust via registered preferences callback
        NotificationCenter.default.post(name: NSNotification.Name("TypeswiftOpenPreferences"), object: nil)
    }
    
    @objc private func showAbout() {
        let alert = NSAlert()
        alert.messageText = "Typeswift"
        alert.informativeText = """
        Version \(Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "1.0.0")
        
        High-performance local speech recognition for macOS.
        """
        alert.alertStyle = .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()
    }
    
    
    func isLaunchAtStartupEnabled() -> Bool {
        // Check if launch agent exists and is loaded
        let launchAgentPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/com.typeswift.app.plist")
        return FileManager.default.fileExists(atPath: launchAgentPath.path)
    }
    
    func enableLaunchAtStartup() {
        // Modern way using ServiceManagement (macOS 13+)
        if #available(macOS 13.0, *) {
            do {
                try SMAppService.mainApp.register()
            } catch {
                print("Failed to register login item: \(error)")
                // Fall back to LaunchAgent method
                installLaunchAgent()
            }
        } else {
            // Use LaunchAgent for older macOS versions
            installLaunchAgent()
        }
    }
    
    func disableLaunchAtStartup() {
        // Modern way using ServiceManagement (macOS 13+)
        if #available(macOS 13.0, *) {
            do {
                try SMAppService.mainApp.unregister()
            } catch {
                print("Failed to unregister login item: \(error)")
                // Fall back to LaunchAgent method
                uninstallLaunchAgent()
            }
        } else {
            // Use LaunchAgent for older macOS versions
            uninstallLaunchAgent()
        }
    }
    
    private func installLaunchAgent() {
        let launchAgentDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents")
        
        // Create LaunchAgents directory if it doesn't exist
        try? FileManager.default.createDirectory(at: launchAgentDir, withIntermediateDirectories: true)
        
        let launchAgentPath = launchAgentDir.appendingPathComponent("com.typeswift.app.plist")
        
        // Get the app bundle path
        let appPath = Bundle.main.bundlePath
        let executablePath = Bundle.main.executablePath ?? "\(appPath)/Contents/MacOS/voicy"
        
        // Create launch agent plist
        let plistContent = """
        <?xml version="1.0" encoding="UTF-8"?>
        <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
        <plist version="1.0">
        <dict>
            <key>Label</key>
            <string>com.typeswift.app</string>
            <key>ProgramArguments</key>
            <array>
                <string>\(executablePath)</string>
            </array>
            <key>RunAtLoad</key>
            <true/>
            <key>LSUIElement</key>
            <true/>
        </dict>
        </plist>
        """
        
        do {
            try plistContent.write(to: launchAgentPath, atomically: true, encoding: .utf8)
            
            // Load the launch agent
            let task = Process()
            task.launchPath = "/bin/launchctl"
            task.arguments = ["load", launchAgentPath.path]
            task.launch()
            task.waitUntilExit()
            
            print("✅ Launch agent installed at: \(launchAgentPath.path)")
        } catch {
            print("Failed to install launch agent: \(error)")
        }
    }
    
    private func uninstallLaunchAgent() {
        let launchAgentPath = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/com.typeswift.app.plist")
        
        if FileManager.default.fileExists(atPath: launchAgentPath.path) {
            // Unload the launch agent
            let task = Process()
            task.launchPath = "/bin/launchctl"
            task.arguments = ["unload", launchAgentPath.path]
            task.launch()
            task.waitUntilExit()
            
            // Remove the plist file
            try? FileManager.default.removeItem(at: launchAgentPath)
            
            print("✅ Launch agent removed")
        }
    }
    
    @objc private func quitApp() {
        NSApplication.shared.terminate(nil)
    }
    
    // MARK: - First Launch Experience
    
    private func checkFirstLaunch() {
        let defaults = UserDefaults.standard
        let hasLaunchedBeforeKey = "com.typeswift.hasLaunchedBefore"
        let hasAskedAboutLoginKey = "com.typeswift.hasAskedAboutLogin"
        
        // Backward compatibility: also check old Voicy keys
        let hasLaunchedBefore = defaults.bool(forKey: hasLaunchedBeforeKey) || defaults.bool(forKey: "com.voicy.hasLaunchedBefore")
        let hasAskedAboutLogin = defaults.bool(forKey: hasAskedAboutLoginKey) || defaults.bool(forKey: "com.voicy.hasAskedAboutLogin")
        
        if !hasLaunchedBefore {
            // First time launch - show welcome
            showWelcomeDialog()
            defaults.set(true, forKey: hasLaunchedBeforeKey)
        } else if !hasAskedAboutLogin {
            // Not first launch, but haven't asked about login yet
            // This handles users who had the app before this feature was added
            DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
                self.askAboutLaunchAtLogin()
                defaults.set(true, forKey: hasAskedAboutLoginKey)
            }
        }
    }
    
    private func showWelcomeDialog() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            let alert = NSAlert()
            alert.messageText = "Welcome to Typeswift! 🎙️"
            alert.informativeText = """
            Typeswift runs in your menu bar for quick access to speech recognition.
            
            • Press and hold the hotkey to record
            • Supports 25 European languages automatically
            • Transcriptions are typed where your cursor is
            
            Would you like Typeswift to start automatically when you log in?
            (You can change this later in the menu)
            """
            alert.alertStyle = .informational
            
            // Add buttons
            alert.addButton(withTitle: "Start at Login")
            alert.addButton(withTitle: "Not Now")
            alert.addButton(withTitle: "Don't Ask Again")
            
            // Show welcome image if available
            if let image = NSImage(systemSymbolName: "mic.circle.fill", accessibilityDescription: "Typeswift") {
                alert.icon = image
            }
            
            let response = alert.runModal()
            
            switch response {
            case .alertFirstButtonReturn:
                // User wants to start at login
                self.enableLaunchAtStartup()
                self.showNotification(
                    title: "Launch at Login Enabled",
                    text: "Typeswift will start automatically when you log in"
                )
                
                // Update menu item if it exists
                if let menuItem = self.menu?.item(withTitle: "Launch at Login") {
                    menuItem.state = .on
                }
                
            case .alertSecondButtonReturn:
                // Not now - just close quietly
                break
                
            default:
                // Don't ask again - mark as asked
                break
            }
            
            // Mark that we've asked about login
            UserDefaults.standard.set(true, forKey: "com.typeswift.hasAskedAboutLogin")
        }
    }
    
    private func askAboutLaunchAtLogin() {
        // Simpler prompt for existing users
        let alert = NSAlert()
        alert.messageText = "Start Typeswift at Login?"
        alert.informativeText = "Would you like Typeswift to start automatically when you log in? This ensures it's always ready in your menu bar."
        alert.alertStyle = .informational
        
        alert.addButton(withTitle: "Enable")
        alert.addButton(withTitle: "No Thanks")
        
        let response = alert.runModal()
        
        if response == .alertFirstButtonReturn {
            self.enableLaunchAtStartup()
            
            // Update menu item
            if let menuItem = self.menu?.item(withTitle: "Launch at Login") {
                menuItem.state = .on
            }
            
            self.showNotification(
                title: "Launch at Login Enabled",
                text: "Typeswift will start automatically"
            )
        }
    }
    
    /// Update status text
    @objc public func setStatusText(_ text: String) {
        DispatchQueue.main.async { [weak self] in
            self?.statusItem?.button?.title = text
        }
    }
    
    /// Update status icon
    @objc public func setStatusIcon(systemName: String) {
        DispatchQueue.main.async { [weak self] in
            if let button = self?.statusItem?.button {
                button.image = NSImage(systemSymbolName: systemName, accessibilityDescription: "Typeswift")
                button.image?.isTemplate = true
            }
        }
    }
    
    /// Show notification
    @objc public func showNotification(title: String, text: String) {
        DispatchQueue.main.async {
            let notification = NSUserNotification()
            notification.title = title
            notification.informativeText = text
            notification.soundName = NSUserNotificationDefaultSoundName
            NSUserNotificationCenter.default.deliver(notification)
        }
    }
    
    /// Add custom menu item
    @objc public func addMenuItem(title: String, action: Selector, target: AnyObject) {
        DispatchQueue.main.async { [weak self] in
            guard let menu = self?.menu else { return }
            
            let newItem = NSMenuItem(title: title, action: action, keyEquivalent: "")
            newItem.target = target
            
            // Insert before separator (above Quit)
            let insertIndex = menu.items.count - 2
            menu.insertItem(newItem, at: insertIndex)
        }
    }
    
    /// Remove all custom menu items
    @objc public func clearCustomMenuItems() {
        DispatchQueue.main.async { [weak self] in
            guard let menu = self?.menu else { return }
            
            // Keep only default items
            while menu.items.count > 9 { // Adjust based on your default items count
                menu.removeItem(at: menu.items.count - 3)
            }
        }
    }
}

// MARK: - Dock Icon Control
extension TypeswiftMenuBar {
    
    /// Hide dock icon (already done via LSUIElement in Info.plist)
    @objc public func hideDockIcon() {
        NSApp.setActivationPolicy(.accessory)
    }
    
    /// Show dock icon (if needed for preferences window)
    @objc public func showDockIcon() {
        NSApp.setActivationPolicy(.regular)
    }
    
    /// Toggle dock icon visibility
    @objc public func toggleDockIcon() {
        if NSApp.activationPolicy() == .regular {
            hideDockIcon()
        } else {
            showDockIcon()
        }
    }
}
