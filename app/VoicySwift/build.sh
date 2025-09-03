#!/bin/bash

echo "Building TypeswiftSwift library..."
swift build -c release --product TypeswiftSwift

if [ $? -eq 0 ]; then
echo "Swift build successful"
    
    # Create symlink for easier testing
    ln -sf .build/release/libTypeswiftSwift.dylib libTypeswiftSwift.dylib
    
    echo "Library available at:"
    echo "  $(pwd)/.build/release/libTypeswiftSwift.dylib"
else
echo "Swift build failed"
    exit 1
fi
