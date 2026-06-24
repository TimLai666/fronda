use app_shell_gpui::{
    editor_view,
    menu::{route_shortcut, MenuAction, Modifiers},
    pane::{LayoutPreset, PaneId, PaneLayout},
    window::WindowConfig,
};
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, FocusHandle, Focusable, InteractiveElement,
    KeyDownEvent, Window, WindowBounds, WindowOptions,
};

struct FrondaRoot {
    focus_handle: FocusHandle,
    pane_layout: PaneLayout,
}

impl FrondaRoot {
    fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        if let Some(window) = cx.window_mut() {
            window.focus(&handle, cx);
        }
        Self {
            focus_handle: handle,
            pane_layout: PaneLayout::new(),
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = Modifiers {
            command: event.keystroke.modifiers.platform,
            shift: event.keystroke.modifiers.shift,
            option: event.keystroke.modifiers.alt,
            control: event.keystroke.modifiers.control,
        };

        let Some(action) = route_shortcut(&event.keystroke.key, &modifiers) else {
            return;
        };

        cx.stop_propagation();

        match action {
            MenuAction::ToggleMediaPanel => {
                self.pane_layout.toggle_pane(PaneId::Media);
            }
            MenuAction::ToggleInspector => {
                self.pane_layout.toggle_pane(PaneId::Inspector);
            }
            MenuAction::ToggleAgentPanel => {
                self.pane_layout.toggle_pane(PaneId::Agent);
            }
            MenuAction::MaximizeFocusedPane => {
                if self.pane_layout.is_maximized() {
                    self.pane_layout.unmaximize();
                } else {
                    // Default to preview as the focused pane
                    self.pane_layout.maximize(PaneId::Preview);
                }
            }
            MenuAction::LayoutDefault => self.pane_layout.apply_preset(LayoutPreset::Default),
            MenuAction::LayoutMedia => self.pane_layout.apply_preset(LayoutPreset::Media),
            MenuAction::LayoutVertical => self.pane_layout.apply_preset(LayoutPreset::Vertical),
            MenuAction::EnterFullScreen => {}
            MenuAction::Quit => {}
            // File actions — stubs until project_io integration
            MenuAction::NewProject
            | MenuAction::OpenProject
            | MenuAction::SaveProject
            | MenuAction::SaveProjectAs
            | MenuAction::ImportMedia
            | MenuAction::Export => {}
            // Edit actions — stubs until timeline integration
            MenuAction::Undo
            | MenuAction::Redo
            | MenuAction::Cut
            | MenuAction::Copy
            | MenuAction::Paste
            | MenuAction::SelectAll
            | MenuAction::SplitAtPlayhead
            | MenuAction::TrimStartToPlayhead
            | MenuAction::TrimEndToPlayhead
            | MenuAction::Delete => {}
            // App menu — stubs
            MenuAction::About | MenuAction::CheckForUpdates | MenuAction::Settings => {}
            // Help — stubs
            MenuAction::Tutorial
            | MenuAction::KeyboardShortcuts
            | MenuAction::McpInstructions
            | MenuAction::SendFeedback => {}
        }

        cx.notify();
    }
}

impl Focusable for FrondaRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FrondaRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("fronda-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
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
            |_, cx| cx.new(|cx| FrondaRoot::new(cx)),
        )
        .unwrap();

        cx.activate(true);
    });
}
