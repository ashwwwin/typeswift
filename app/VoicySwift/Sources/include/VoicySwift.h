#ifndef TYPESWIFT_SWIFT_H
#define TYPESWIFT_SWIFT_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Initialize the transcriber with optional model path
/// @param model_path Optional path to CoreML models (can be NULL for default)
/// @return 0 on success, -1 on failure
int32_t typeswift_init(const char* model_path);

/// Transcribe audio samples
/// @param samples Pointer to float32 audio samples (16kHz mono)
/// @param sample_count Number of samples
/// @return Transcribed text as C string (caller must free with voicy_free_string)
char* typeswift_transcribe(const float* samples, int32_t sample_count);

/// Free a string returned by voicy_transcribe
/// @param str String to free
void typeswift_free_string(char* str);

/// Cleanup all resources
void typeswift_cleanup(void);

/// Check if transcriber is ready
/// @return true if initialized and ready
bool typeswift_is_ready(void);

#ifdef __cplusplus
}
#endif

#endif // TYPESWIFT_SWIFT_H
