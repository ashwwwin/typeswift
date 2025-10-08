import Foundation

// MARK: - C FFI Exports

/// Initialize the transcriber
/// Returns 0 on success, -1 on failure
@_cdecl("typeswift_init")
public func typeswift_init(_ model_path: UnsafePointer<CChar>?) -> Int32 {
    let semaphore = DispatchSemaphore(value: 0)
    var result: Int32 = -1
    
    Task {
        let path: String? = model_path.flatMap { String(cString: $0) }
        result = await TypeswiftTranscriber.shared.initialize(modelPath: path)
        semaphore.signal()
    }
    
    semaphore.wait()
    return result
}

/// Transcribe audio samples
/// Returns C string that caller must free, or NULL on error
@_cdecl("typeswift_transcribe")
public func typeswift_transcribe(
    _ samples: UnsafePointer<Float>?,
    _ sample_count: Int32
) -> UnsafeMutablePointer<CChar>? {
    guard let samples = samples, sample_count > 0 else {
        return strdup("")
    }
    
    let semaphore = DispatchSemaphore(value: 0)
    var result: UnsafeMutablePointer<CChar>? = nil
    
    Task {
        result = await TypeswiftTranscriber.shared.transcribe(
            samples: samples,
            sampleCount: Int(sample_count)
        )
        semaphore.signal()
    }
    
    semaphore.wait()
    return result
}

/// Free a C string returned by transcribe
@_cdecl("typeswift_free_string")
public func typeswift_free_string(_ str: UnsafeMutablePointer<CChar>?) {
    if let str = str {
        free(str)
    }
}

/// Cleanup resources
@_cdecl("typeswift_cleanup")
public func typeswift_cleanup() {
    let semaphore = DispatchSemaphore(value: 0)
    
    Task {
        await TypeswiftTranscriber.shared.cleanup()
        semaphore.signal()
    }
    
    semaphore.wait()
}

/// Check if transcriber is ready
@_cdecl("typeswift_is_ready")
public func typeswift_is_ready() -> Bool {
    return TypeswiftTranscriber.shared.isReady()
}
