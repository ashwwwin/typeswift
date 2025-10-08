import Foundation
import AppKit
import Carbon

@objc public class TypeswiftKeyboardMonitor: NSObject {
    
    private var eventMonitor: Any?
    private var flagsMonitor: Any?
    private var eventTap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private var isMonitoring: Bool = false
    private var isRecording = false
    private var lastModifierFlags: NSEvent.ModifierFlags = []
    
    @objc public static let shared = TypeswiftKeyboardMonitor()
    
    private override init() {
        super.init()
    }
    
    @objc public func startMonitoring() {
        if isMonitoring { return }
        isMonitoring = true
        print("Starting keyboard monitoring for fn key")
        
        // Monitor modifier flags changes (for fn key)
        flagsMonitor = NSEvent.addGlobalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.handleFlagsChanged(event)
        }
        
        // Also monitor local events (when app is in focus)
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.handleFlagsChanged(event)
            return event
        }
        
        print("Keyboard monitoring started")
    }
    
    @objc public func stopMonitoring() {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
            eventMonitor = nil
        }
        if let monitor = flagsMonitor {
            NSEvent.removeMonitor(monitor)
            flagsMonitor = nil
        }
        if let source = runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), source, .commonModes)
            runLoopSource = nil
        }
        if let tap = eventTap {
            CGEvent.tapEnable(tap: tap, enable: false)
            eventTap = nil
        }
        isMonitoring = false
        print("Keyboard monitoring stopped")
    }
    
    private func handleFlagsChanged(_ event: NSEvent) {
        let currentFlags = event.modifierFlags
        
        // Check if fn key state changed
        let fnWasPressed = lastModifierFlags.contains(.function)
        let fnIsPressed = currentFlags.contains(.function)
        
        if fnIsPressed && !fnWasPressed {
            // fn key was just pressed
            if !isRecording {
                isRecording = true
                print("Fn key PRESSED - Starting recording")
                
                // Post notification to Rust side
                DispatchQueue.main.async {
                    NotificationCenter.default.post(
                        name: NSNotification.Name("TypeswiftPushToTalkPressed"),
                        object: nil
                    )
                }
            }
        } else if !fnIsPressed && fnWasPressed {
            // fn key was just released
            if isRecording {
                isRecording = false
                print("Fn key RELEASED - Stopping recording")
                
                // Post notification to Rust side
                DispatchQueue.main.async {
                    NotificationCenter.default.post(
                        name: NSNotification.Name("TypeswiftPushToTalkReleased"),
                        object: nil
                    )
                }
            }
        }
        
        lastModifierFlags = currentFlags
    }
    
    // Alternative method using CGEvent for system-wide monitoring
    @objc public func startCGEventMonitoring() -> Bool {
        if isMonitoring { return true }
        isMonitoring = true
        print("Starting CGEvent monitoring for fn key")
        
        // Request accessibility permissions
        let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
        let trusted = AXIsProcessTrustedWithOptions(options)
        
        if !trusted {
            print("Accessibility permissions required for keyboard monitoring")
            print("Please grant accessibility permissions in System Preferences > Security & Privacy > Privacy > Accessibility")
            return false
        }
        
        // Create event tap for modifier flags
        guard let eventTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: CGEventMask(1 << CGEventType.flagsChanged.rawValue),
            callback: { (proxy, type, event, refcon) -> Unmanaged<CGEvent>? in
                guard let refcon = refcon else { return Unmanaged.passRetained(event) }
                let monitor = Unmanaged<TypeswiftKeyboardMonitor>.fromOpaque(refcon).takeUnretainedValue()
                monitor.handleCGEvent(event)
                return Unmanaged.passRetained(event)
            },
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            print("Failed to create event tap")
            return false
        }
        
        // Add to run loop and retain for cleanup
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0)
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
        CGEvent.tapEnable(tap: eventTap, enable: true)
        self.eventTap = eventTap
        self.runLoopSource = source
        
        print("CGEvent monitoring started")
        return true
    }
    
    private func handleCGEvent(_ event: CGEvent) {
        let flags = event.flags
        
        // Check if fn key is pressed (function flag)
        let fnIsPressed = flags.contains(.maskSecondaryFn)
        
        if fnIsPressed && !isRecording {
            isRecording = true
            print("Fn key PRESSED (CGEvent) - Starting recording")
            
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: NSNotification.Name("TypeswiftPushToTalkPressed"),
                    object: nil
                )
            }
        } else if !fnIsPressed && isRecording {
            isRecording = false
            print("Fn key RELEASED (CGEvent) - Stopping recording")
            
            DispatchQueue.main.async {
                NotificationCenter.default.post(
                    name: NSNotification.Name("TypeswiftPushToTalkReleased"),
                    object: nil
                )
            }
        }
    }
}

// Extension to make the keyboard monitor accessible from Rust FFI
@objc public extension TypeswiftKeyboardMonitor {
    
    @objc static func initializeKeyboardMonitor() -> Bool {
        // Try CGEvent monitoring first (more reliable for fn key)
        if shared.startCGEventMonitoring() {
            return true
        }
        
        // Fall back to NSEvent monitoring
        shared.startMonitoring()
        return true
    }
    
    @objc static func shutdownKeyboardMonitor() {
        shared.stopMonitoring()
    }
}
