use app_shell_gpui::launch_status_lines;
use gpui::{div, prelude::*, px, size, App, Bounds, Context, Window, WindowBounds, WindowOptions};

struct FrondaRoot;

impl FrondaRoot {
    fn new() -> Self {
        Self
    }
}

impl Render for FrondaRoot {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let [title, headline, status] = launch_status_lines();

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .justify_center()
            .items_center()
            .gap_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(title),
            )
            .child(div().child(headline))
            .child(div().child(status))
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| FrondaRoot::new()),
        )
        .unwrap();

        cx.activate(true);
    });
}
