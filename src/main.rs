use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, point, prelude::*,
    px, rgb, size,
};

struct Voicy;

impl Render for Voicy {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .bg(gpui::black())
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .border_color(rgb(0x0000ff))
            .text_sm()
            .text_color(rgb(0xffffff))
            .child("Listening")
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
            |_, cx| cx.new(|_| Voicy),
        )
        .unwrap();
    });
}
