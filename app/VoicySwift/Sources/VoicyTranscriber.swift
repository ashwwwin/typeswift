import Foundation
import FluidAudio

/// Thread-safe transcriber for audio processing
@objc public class VoicyTranscriber: NSObject {
    private var asrManager: AsrManager?
    private var isInitialized = false
    private let initializationQueue = DispatchQueue(label: "com.voicy.initialization")
    private let transcriptionQueue = DispatchQueue(label: "com.voicy.transcription", attributes: .concurrent)
    
    /// Singleton instance for FFI usage
    @objc public static let shared = VoicyTranscriber()
    
    private override init() {
        super.init()
    }
    
    /// Initialize the transcriber with model loading
    @objc public func initialize(modelPath: String?) async -> Int32 {
        return await withCheckedContinuation { continuation in
            initializationQueue.async { [weak self] in
                guard let self = self else {
                    continuation.resume(returning: -1)
                    return
                }
                
                if self.isInitialized {
                    continuation.resume(returning: 0)
                    return
                }
                
                Task {
                    do {
                        // Create ASR Manager with default config
                        self.asrManager = AsrManager(config: .default)
                        
                        // Load models
                        let models: AsrModels
                        if let path = modelPath {
                            // Try loading from specified path
                            let url = URL(fileURLWithPath: path)
                            models = try await AsrModels.load(from: url)
                            print("✅ Models loaded from: \(path)")
                        } else {
                            // Check common local paths first
                            let possiblePaths = [
                                // User's home directory
                                FileManager.default.homeDirectoryForCurrentUser
                                    .appendingPathComponent(".typeswift/models/parakeet-tdt-0.6b-v3-coreml"),
                                // Backward compatibility
                                FileManager.default.homeDirectoryForCurrentUser
                                    .appendingPathComponent(".voicy/models/parakeet-tdt-0.6b-v3-coreml"),
                                // Application Support
                                FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
                                    .appendingPathComponent("Typeswift/models/parakeet-tdt-0.6b-v3-coreml"),
                                // Backward compatibility
                                FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first?
                                    .appendingPathComponent("Voicy/models/parakeet-tdt-0.6b-v3-coreml"),
                                // Development path
                                URL(fileURLWithPath: "/Users/mac/Desktop/voicy/parakeet-tdt-0.6b-v3-coreml")
                            ].compactMap { $0 }
                            
                            var loadedModels: AsrModels? = nil
                            for possiblePath in possiblePaths {
                                if FileManager.default.fileExists(atPath: possiblePath.path) {
                                    do {
                                        loadedModels = try await AsrModels.load(from: possiblePath)
                                        print("✅ Models loaded from: \(possiblePath.path)")
                                        break
                                    } catch {
                                        print("⚠️ Failed to load from \(possiblePath.path): \(error)")
                                        continue
                                    }
                                }
                            }
                            
                            if let existingModels = loadedModels {
                                models = existingModels
                            } else {
                                // Download models if not found locally
                                print("📥 Models not found locally. Downloading...")
                                let downloadedPath = try await AsrModels.download()
                                
                                // Save to Application Support for future use
                                if let appSupportURL = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first {
                                    let modelsDir = appSupportURL
                                        .appendingPathComponent("Typeswift")
                                        .appendingPathComponent("models")
                                    
                                    do {
                                        try FileManager.default.createDirectory(at: modelsDir, withIntermediateDirectories: true)
                                        let targetPath = modelsDir.appendingPathComponent("parakeet-tdt-0.6b-v3-coreml")
                                        
                                        // Copy downloaded models to Application Support
                                        if FileManager.default.fileExists(atPath: targetPath.path) {
                                            try FileManager.default.removeItem(at: targetPath)
                                        }
                                        try FileManager.default.copyItem(at: downloadedPath, to: targetPath)
                                        print("💾 Models saved to: \(targetPath.path)")
                                    } catch {
                                        print("⚠️ Failed to save models to Application Support: \(error)")
                                    }
                                }
                                
                                models = try await AsrModels.load(from: downloadedPath)
                                print("✅ Models downloaded and loaded")
                            }
                        }
                        
                        // Initialize ASR Manager with models
                        try await self.asrManager?.initialize(models: models)
                        self.isInitialized = true
                        print("✅ Transcriber initialized")
                        
                        continuation.resume(returning: 0)
                    } catch {
                        print("❌ Initialization failed: \(error)")
                        continuation.resume(returning: -1)
                    }
                }
            }
        }
    }
    
    /// Transcribe audio samples
    @objc public func transcribe(samples: UnsafePointer<Float>, sampleCount: Int) async -> UnsafeMutablePointer<CChar>? {
        guard isInitialized, let asrManager = asrManager else {
            print("❌ Transcriber not initialized")
            return strdup("")
        }
        
        // Convert unsafe pointer to Swift array
        let audioArray = Array(UnsafeBufferPointer(start: samples, count: sampleCount))
        
        do {
            // Perform transcription (using .system as source)
            let result = try await asrManager.transcribe(audioArray, source: .system)
            
            // Convert Swift String to C string (caller must free)
            let cString = strdup(result.text)
            
            print("📝 Transcribed: \(result.text)")
            print("   Confidence: \(result.confidence)")
            
            return cString
        } catch {
            print("❌ Transcription failed: \(error)")
            return strdup("")
        }
    }
    
    /// Cleanup resources
    @objc public func cleanup() async {
        if let asrManager = asrManager {
            await asrManager.cleanup()
            self.asrManager = nil
            self.isInitialized = false
            print("🧹 Transcriber cleaned up")
        }
    }
    
    /// Check if initialized
    @objc public func isReady() -> Bool {
        return isInitialized
    }
}
