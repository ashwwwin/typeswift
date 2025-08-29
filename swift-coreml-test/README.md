# Parakeet CoreML Transcription

Swift implementation for transcribing audio using Parakeet CoreML models with FluidAudio.

## Requirements

- macOS 13.0+
- Swift 5.9+
- CoreML models in `/Users/mac/Desktop/voicy/parakeet-coreml/`

## Usage

### Quick Run
```bash
./build_and_run.sh
```

### Manual Build & Run
```bash
swift build -c release
./.build/release/ParakeetTranscription
```

## Files

- `Sources/ParakeetTranscriber.swift` - Main transcription implementation
- `Package.swift` - Swift package configuration with FluidAudio dependency
- `test.wav` - Sample audio file for testing
- `build_and_run.sh` - Build and run script

## How It Works

1. Loads audio from WAV file (skips 44-byte header)
2. Converts 16-bit PCM samples to float32 normalized values
3. Loads Parakeet models using FluidAudio's AsrManager
4. Performs transcription using the loaded models
5. Outputs transcribed text with statistics

## Performance

- Real-time factor: ~45x (processes 20 seconds of audio in 0.46 seconds)
- Confidence: 100%
- Uses Apple Neural Engine for acceleration