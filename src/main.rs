mod audio_recorder;

use audio_recorder::AudioRecorder;
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, rgb, size,
};

struct Voicy {
    recorder: AudioRecorder,
    state: RecordingState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RecordingState {
    Idle,
    Recording,
    Processing,
    Error,
}

impl Voicy {
    fn new() -> Self {
        Self {
            recorder: AudioRecorder::new(),
            state: RecordingState::Idle,
        }
    }

    fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        match self.state {
            RecordingState::Idle => {
                if let Err(e) = self.recorder.start_recording() {
                    eprintln!("Failed to start recording: {}", e);
                    self.state = RecordingState::Error;
                } else {
                    self.state = RecordingState::Recording;
                }
            }
            RecordingState::Recording => {
                let audio_data = self.recorder.stop_recording();
                println!("Recorded {} samples", audio_data.len());
                self.state = RecordingState::Idle;
            }
            _ => {}
        }
        cx.notify();
    }
}

impl Render for Voicy {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let status_text = match self.state {
            RecordingState::Idle => "Click to start",
            RecordingState::Recording => "Recording... (click to stop)",
            RecordingState::Processing => "Processing...",
            RecordingState::Error => "Error occurred",
        };

        let bg_color = match self.state {
            RecordingState::Recording => rgb(0xff0000),
            RecordingState::Error => rgb(0x800000),
            _ => rgb(0x000000),
        };

        div()
            .id("voicy-main")
            .flex()
            .bg(bg_color)
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .border_color(rgb(0x0000ff))
            .text_sm()
            .text_color(rgb(0xffffff))
            .child(status_text)
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _event, _window, cx| {
                this.toggle_recording(cx);
            }))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let window_size = size(px(90.), px(39.0));
        let gap_from_bottom = px(70.);

        // Get the primary display
        let displays = cx.displays();
        let screen = displays.first().expect("No displays found");

        // Calculate position for bottom center with gap
        let bounds = Bounds {
            origin: point(
                screen.bounds().center().x - window_size.width / 2.,
                screen.bounds().size.height - window_size.height - gap_from_bottom,
            ),
            size: window_size,
        };

        cx.open_window(
            WindowOptions {
                is_movable: false,
                titlebar: None,
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(screen.id()),
                ..Default::default()
            },
            |_, cx| cx.new(|_| Voicy::new()),
        )
        .unwrap();
    });
}
