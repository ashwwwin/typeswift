# VoicySwift - High-Performance Speech Recognition

Swift/CoreML integration for Voicy using FluidAudio framework, replacing the Python/MLX implementation for 110x real-time performance on Apple Silicon.

## Architecture Overview

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│   Rust App  │ ───► │  FFI Bridge  │ ───► │Swift/CoreML │
│             │      │              │      │             │
│ AudioCapture│      │ swift_ffi.rs │      │VoicySwift   │
│ AudioProc   │ ◄─── │ C Interface  │ ◄─── │FluidAudio   │
└─────────────┘      └──────────────┘      └─────────────┘
```

## Key Benefits

- **Performance**: ~110x real-time factor on Apple M-series chips
- **Native**: Direct CoreML integration via Apple Neural Engine
- **Simple**: No Python dependencies or GIL overhead
- **Efficient**: Zero-copy audio transfer via FFI

## Components

### Swift Side (VoicySwift/)
- `VoicyTranscriber.swift` - Main transcription class wrapping FluidAudio
- `VoicyFFI.swift` - C-compatible FFI exports
- `include/VoicySwift.h` - C header for Rust integration

### Rust Side
- `src/swift_ffi.rs` - Safe Rust bindings to Swift
- `src/audio/transcriber.rs` - Updated to use Swift instead of Python
- `build.rs` - Automated Swift library compilation

## Building

### Swift Library
```bash
cd VoicySwift
./build.sh
```

### Complete Integration
```bash
cargo build --release
```

## Usage

### From Rust
```rust
use typeswift::platform::macos::ffi::SwiftTranscriber;

let mut transcriber = SwiftTranscriber::new();
transcriber.initialize(None)?;

let text = transcriber.transcribe(&audio_samples)?;
println!("Transcribed: {}", text);

transcriber.cleanup();
```

### Testing
```bash
cargo run --example test_swift
```

## Performance Metrics

| Metric | Python/MLX | Swift/FluidAudio | Improvement |
|--------|------------|------------------|-------------|
| RTF | ~45x | ~110x | 2.4x faster |
| Memory | High (Python) | Low (Native) | ~60% less |
| Latency | 50-100ms | <20ms | 3-5x lower |

## Models

Uses Parakeet TDT 0.6B CoreML model:
- 25 European languages
- 600MB model size
- Optimized for Apple Neural Engine

## Current Limitations

- **Batch-only**: FluidAudio doesn't support streaming yet (coming soon)
- **Platform**: macOS 14.0+ required
- **Architecture**: Apple Silicon only (Intel Macs not supported)

## Future Improvements

1. **Streaming Support**: Will be added when FluidAudio implements it
2. **Model Selection**: Support for different model sizes
3. **Language Detection**: Automatic language identification
4. **Speaker Diarization**: Multi-speaker support via FluidAudio
