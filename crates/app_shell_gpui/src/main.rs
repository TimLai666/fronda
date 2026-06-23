use app_shell_gpui::{launch_status_lines, pane::PaneLayout, window::WindowConfig};
use gpui::{div, prelude::*, px, size, App, Bounds, Context, Window, WindowBounds, WindowOptions};

struct FrondaRoot {
    pane_layout: PaneLayout,
}

impl FrondaRoot {
    fn new() -> Self {
        Self {
            pane_layout: PaneLayout::new(),
        }
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
            .child(div().child(format!(
                "Panels: {} visible",
                self.pane_layout.visible_count()
            )))
            .child(div().child(format!("Layout: {:?}", self.pane_layout.preset)))
    }
}

fn main() {
    let cfg = WindowConfig::for_project();
    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(
            None,
            size(px(cfg.default_width as f32), px(cfg.default_height as f32)),
            cx,
        );

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
