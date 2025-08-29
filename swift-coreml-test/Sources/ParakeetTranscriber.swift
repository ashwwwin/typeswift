import Foundation
import FluidAudio
import AVFoundation

@main
struct ParakeetTranscriber {
    static func main() async {
        print("🎤 Parakeet CoreML Transcription")
        print("=================================\n")
        
        do {
            // Load audio
            print("Loading audio...")
            let audioPath = "/Users/mac/Desktop/voicy/swift-coreml-test/test.wav"
            let audioData = try Data(contentsOf: URL(fileURLWithPath: audioPath))
            
            // Skip WAV header
            guard audioData.count > 44 else {
                print("❌ Invalid WAV file")
                return
            }
            
            let audioBytes = audioData.subdata(in: 44..<audioData.count)
            let sampleCount = audioBytes.count / 2
            
            var audioSamples = [Float](repeating: 0, count: sampleCount)
            audioBytes.withUnsafeBytes { bytes in
                let int16Pointer = bytes.bindMemory(to: Int16.self)
                for i in 0..<sampleCount {
                    audioSamples[i] = Float(int16Pointer[i]) / 32768.0
                }
            }
            print("✅ Loaded \(audioSamples.count) samples (\(String(format: "%.1f", Double(audioSamples.count)/16000.0)) seconds)")
            
            // Initialize ASR
            print("\nInitializing ASR Manager...")
            let asrManager = AsrManager()
            
            // Try loading from local directory first
            print("Attempting to load models from local directory...")
            let localModelsPath = URL(fileURLWithPath: "/Users/mac/Desktop/voicy/parakeet-tdt-0.6b-v3-coreml")
            
            do {
                let models = try await AsrModels.load(from: localModelsPath)
                print("✅ Models loaded from local directory")
                
                try await asrManager.initialize(models: models)
                print("✅ ASR Manager initialized")
                
            } catch {
                print("⚠️  Local loading failed: \(error)")
                print("\nTrying to download models...")
                
                let models = try await AsrModels.downloadAndLoad()
                print("✅ Models downloaded")
                
                try await asrManager.initialize(models: models)
                print("✅ ASR Manager initialized")
            }
            
            // Transcribe
            print("\n🤖 Starting transcription...")
            let startTime = Date()
            
            let result = try await asrManager.transcribe(audioSamples)
            
            let processingTime = Date().timeIntervalSince(startTime)
            
            // Results
            print("\n" + String(repeating: "=", count: 60))
            print("📝 TRANSCRIPTION RESULT")
            print(String(repeating: "=", count: 60))
            print("\n\(result.text)\n")
            print(String(repeating: "=", count: 60))
            
            print("\n📊 Statistics:")
            print("   Processing time: \(String(format: "%.2f", processingTime)) seconds")
            print("   Confidence: \(String(format: "%.1f%%", result.confidence * 100))")
            print("   Real-time factor: \(String(format: "%.1fx", (Double(audioSamples.count)/16000.0) / processingTime))")
            
            // Cleanup
            await asrManager.cleanup()
            
        } catch {
            print("\n❌ Fatal error: \(error)")
            
            // Detailed error info
            if let nsError = error as NSError? {
                print("\nError details:")
                print("  Domain: \(nsError.domain)")
                print("  Code: \(nsError.code)")
                print("  Description: \(nsError.localizedDescription)")
            }
        }
    }
}