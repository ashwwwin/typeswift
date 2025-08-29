#!/bin/bash

echo "Building VoicySwift library..."
swift build -c release --product VoicySwift

if [ $? -eq 0 ]; then
    echo "✅ Swift build successful"
    
    # Create symlink for easier testing
    ln -sf .build/release/libVoicySwift.dylib libVoicySwift.dylib
    
    echo "Library available at:"
    echo "  $(pwd)/.build/release/libVoicySwift.dylib"
else
    echo "❌ Swift build failed"
    exit 1
fi