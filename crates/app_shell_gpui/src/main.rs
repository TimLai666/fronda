use app_shell_gpui::{editor_view, pane::PaneLayout, window::WindowConfig};
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
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(editor_view::render_pane_layout(&self.pane_layout))
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
