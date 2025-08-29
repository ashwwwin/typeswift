#!/bin/bash

echo "Building Parakeet Transcription..."
swift build -c release

if [ $? -eq 0 ]; then
    echo -e "\nBuild successful! Running transcription...\n"
    ./.build/release/ParakeetTranscription
else
    echo "Build failed"
    exit 1
fi