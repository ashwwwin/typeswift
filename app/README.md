<p align="center">
  <img src="logo.png" alt="Typeswift" width="120" />
</p>

<h1 align="center">Typeswift</h1>

<p align="center">
  High‑performance, privacy‑first push‑to‑talk speech‑to‑text for macOS.<br/>
  Hold a hotkey, speak, and Typeswift types locally using Core ML.
  <br/>
  <br/>
  under ~100mb memory at all times. Built with Rust and Swift.
</p>

## Why

- Fast local transcription: no servers; audio never leaves your Mac.
- Works everywhere: types into any focused app via virtual keystrokes.
- Minimal friction: menu bar app that never steals focus.

## Features

- Push‑to‑talk: Fn/Globe by default; configurable chords (e.g., `cmd+space`).
- On‑device ASR: Core ML via a Swift bridge; auto‑downloads model if missing.
- Menu bar UI: recording indicator, Preferences, About, Quit.
- Smart typing: optional leading space; waits for modifier keys to release before typing.
- Launch at login: toggle in Preferences (ServiceManagement with LaunchAgents fallback).

## Requirements

- macOS 14+ to build the Swift package (bundled app runs on 13+).
- Xcode 15+ (Swift 5.9 toolchain) and Command Line Tools.
- Rust 1.89+
- Microphone and Accessibility permissions (see below).

## Quick Start (dev)

```bash
# From repo root:
cargo run --release
```

Then grant prompts for Microphone and Accessibility. The menu bar icon appears—hold Fn (or your chosen hotkey), speak, release to type.

## Packaging (.app)

```bash
# Full bundle (build Swift + Rust and package)
./tools/rebuild.sh

# Fast re‑bundle (reuse existing Swift dylib)
./tools/rebuild.sh fast

# Regenerate .icns from logo.png
./tools/rebuild.sh icons
```

Output: `dist/Typeswift.app`

## Permissions

- Microphone: System Settings → Privacy & Security → Microphone → enable for Typeswift.
- Accessibility (typing + Fn monitor): Privacy & Security → Accessibility → enable for Typeswift.
- If using Fn and events don’t trigger, also check Privacy & Security → Input Monitoring.

## Usage

- Hold the push‑to‑talk key, speak, release to type into the focused app.
- Preferences (menu bar → Preferences):
  - Enable typing: master toggle for simulated typing.
  - Add space between utterances: prepends a single space before each result.
  - Push‑to‑talk shortcut: click and press your keys (Esc to cancel). “Use Fn key” sets Fn/Globe.
  - Launch at startup: toggle login item.

## Configuration (optional)

- Config file: `~/.typeswift/config.toml`. Missing file uses sane defaults.

```toml
[audio]
target_sample_rate = 16000

[model]
# Leave default to auto‑manage Core ML model; set an absolute path to override
model_name = "mlx-community/parakeet-tdt-0.6b-v3"
left_context_seconds = 5
right_context_seconds = 3

[ui]
window_width = 90.0
window_height = 39.0
gap_from_bottom = 70.0

[output]
enable_typing = true
add_space_between_utterances = true

[hotkeys]
# `fn` for Fn/Globe, or chords like "cmd+space", "ctrl+shift+y"
push_to_talk = "fn"
# Optional: show/hide the small status window
toggle_window = "cmd+shift+y"
```

## Models

- The Swift bridge (FluidAudio) looks for a Core ML model locally and downloads it on first run if not present.
- To provide your own, set an absolute path in `model_name` or set `TYPESWIFT_MODELS=/path/to/model_dir`.

## Supported Languages

Parakeet TDT v3 (0.6B) supports 25 European languages with automatic detection:

- English, Spanish, French, German, Bulgarian, Croatian, Czech, Danish, Dutch,
  Estonian, Finnish, Greek, Hungarian, Italian, Latvian, Lithuanian, Maltese,
  Polish, Portuguese, Romanian, Slovak, Slovenian, Swedish, Russian, Ukrainian.

## Logging & Troubleshooting

- Verbose logs: `RUST_LOG=info cargo run --release`
- Nothing types: ensure Accessibility permission is granted and “Enable typing” is on.
- Fn key not detected: grant Accessibility (and Input Monitoring if prompted), or switch PTT to a chord (e.g., `cmd+space`).
- No audio: select a working input device in macOS and confirm Microphone permission.

## Architecture (at a glance)

- Rust (gpui, cpal, enigo): UI/status, hotkeys, audio capture/resample (16 kHz), simulated typing.
- Swift (Core ML via FluidAudio): model management and transcription; menu bar + system integration.
- FFI: Rust links the Swift dynamic library built from `VoicySwift`.

## Development tips

- For dev runs, assets can be loaded from the working directory; override with `TYPESWIFT_ASSETS=/path/to/assets`.
- When packaging, the Swift dylib is staged under `Contents/Frameworks` and the app is ad‑hoc signed.

## Limitations

- macOS only. Transcription is batch in the current release (no partial streaming yet).

## Credits

- Built on: gpui (Zed), cpal, rubato, enigo, global‑hotkey, FluidAudio.
