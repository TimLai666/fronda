//! Reusable right-click context menu: pure open/close/activate state plus a
//! gpui renderer (deferred + anchored popover, outside-click dismiss).
//!
//! Hosts own a [`ContextMenuState`], open it from an
//! `on_mouse_down(MouseButton::Right, ..)` listener with the event's window
//! position, render via [`render_context_menu`], and close on Escape in their
//! key handler. Destructive items can require an in-menu confirm step: the
//! first activation arms the item (label swaps to `confirm_label`), the second
//! performs it.

use gpui::{
    anchored, deferred, div, prelude::*, px, Context, IntoElement, MouseDownEvent, Pixels, Point,
    SharedString, Window,
};

use crate::theme::{
    Background, BorderColors, BorderWidth, FontSize, Radius, Spacing, Status, Text,
};

/// Draw order among deferred elements; menus sit above other overlays.
const MENU_LAYER_PRIORITY: usize = 1;
/// Component-local sizing (no AppTheme equivalent; Swift uses native NSMenu).
const MENU_MIN_WIDTH: f32 = 160.0;

/// One selectable row in a context menu.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub id: &'static str,
    pub label: SharedString,
    pub destructive: bool,
    /// Present ⇒ activation arms a confirm step first; shown while armed.
    pub confirm_label: Option<SharedString>,
    /// Right-aligned shortcut hint (e.g. "Ctrl+N").
    pub hint: Option<SharedString>,
}

impl MenuItem {
    /// Label to render given whether the item is armed for confirmation.
    pub fn display_label(&self, armed: bool) -> SharedString {
        if armed {
            self.confirm_label
                .clone()
                .unwrap_or_else(|| self.label.clone())
        } else {
            self.label.clone()
        }
    }
}

/// A menu row: an activatable item or a visual separator.
#[derive(Debug, Clone)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
}

impl MenuEntry {
    pub fn item(id: &'static str, label: impl Into<SharedString>) -> Self {
        Self::Item(MenuItem {
            id,
            label: label.into(),
            destructive: false,
            confirm_label: None,
            hint: None,
        })
    }

    pub fn item_with_hint(
        id: &'static str,
        label: impl Into<SharedString>,
        hint: Option<SharedString>,
    ) -> Self {
        Self::Item(MenuItem {
            id,
            label: label.into(),
            destructive: false,
            confirm_label: None,
            hint,
        })
    }

    pub fn destructive(id: &'static str, label: impl Into<SharedString>) -> Self {
        Self::Item(MenuItem {
            id,
            label: label.into(),
            destructive: true,
            confirm_label: None,
            hint: None,
        })
    }

    pub fn destructive_confirm(
        id: &'static str,
        label: impl Into<SharedString>,
        confirm_label: impl Into<SharedString>,
    ) -> Self {
        Self::Item(MenuItem {
            id,
            label: label.into(),
            destructive: true,
            confirm_label: Some(confirm_label.into()),
            hint: None,
        })
    }

    pub fn separator() -> Self {
        Self::Separator
    }
}

/// An open menu: pointer position (window coords), the host-defined target the
/// menu acts on, and which entry (if any) is armed for confirmation.
#[derive(Debug, Clone)]
pub struct OpenMenu<T> {
    pub x: f32,
    pub y: f32,
    pub target: T,
    pub confirming: Option<usize>,
}

/// Outcome of activating an entry index against an entry list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    /// Entered the confirm step; the menu stays open.
    Armed,
    /// Perform the action with this item id; the menu has closed.
    Perform(&'static str),
    /// Separator, out-of-bounds index, or menu not open.
    Ignored,
}

/// Pure open/close/activate state for one context menu surface.
#[derive(Debug, Clone)]
pub struct ContextMenuState<T> {
    open: Option<OpenMenu<T>>,
}

impl<T> Default for ContextMenuState<T> {
    fn default() -> Self {
        Self { open: None }
    }
}

impl<T> ContextMenuState<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open (or move) the menu at a window position for a target. Resets any
    /// pending confirm step.
    pub fn open_at(&mut self, x: f32, y: f32, target: T) {
        self.open = Some(OpenMenu {
            x,
            y,
            target,
            confirming: None,
        });
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn open_menu(&self) -> Option<&OpenMenu<T>> {
        self.open.as_ref()
    }

    pub fn target(&self) -> Option<&T> {
        self.open.as_ref().map(|m| &m.target)
    }

    /// Activate the entry at `index`. Items with a `confirm_label` arm on the
    /// first activation and perform on the second; plain items perform
    /// immediately. Performing closes the menu.
    pub fn activate(&mut self, index: usize, entries: &[MenuEntry]) -> Activation {
        let Some(open) = self.open.as_mut() else {
            return Activation::Ignored;
        };
        match entries.get(index) {
            Some(MenuEntry::Item(item)) => {
                if item.confirm_label.is_some() && open.confirming != Some(index) {
                    open.confirming = Some(index);
                    Activation::Armed
                } else {
                    let id = item.id;
                    self.open = None;
                    Activation::Perform(id)
                }
            }
            _ => Activation::Ignored,
        }
    }
}

/// Render an open context menu as a deferred, window-anchored popover.
///
/// `on_activate` receives the activated entry index; the host routes it
/// through [`ContextMenuState::activate`]. `on_dismiss` fires on any mouse
/// down outside the menu (the host also closes on Escape in its key handler).
pub fn render_context_menu<V: 'static>(
    position: Point<Pixels>,
    entries: Vec<MenuEntry>,
    confirming: Option<usize>,
    cx: &mut Context<V>,
    on_activate: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + Clone + 'static,
    on_dismiss: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let mut menu = div()
        .id("context-menu")
        .occlude()
        .flex()
        .flex_col()
        .min_w(px(MENU_MIN_WIDTH))
        .py(px(Spacing::XS))
        .bg(Background::RAISED)
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .rounded(px(Radius::SM))
        .shadow_lg()
        .on_mouse_down_out(cx.listener(
            move |this, _: &MouseDownEvent, window: &mut Window, cx| {
                on_dismiss(this, window, cx);
            },
        ));

    for (i, entry) in entries.iter().enumerate() {
        match entry {
            MenuEntry::Separator => {
                menu = menu.child(
                    div()
                        .my(px(Spacing::XXS))
                        .h(px(BorderWidth::HAIRLINE))
                        .w_full()
                        .bg(BorderColors::SUBTLE),
                );
            }
            MenuEntry::Item(item) => {
                let armed = confirming == Some(i);
                let color = if item.destructive {
                    Status::ERROR
                } else {
                    Text::PRIMARY
                };
                let on_activate = on_activate.clone();
                menu = menu.child(
                    div()
                        .id(("context-menu-item", i))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::LG))
                        .px(px(Spacing::MD))
                        .py(px(Spacing::XS))
                        .cursor_pointer()
                        .text_size(px(FontSize::SM))
                        .text_color(color)
                        .hover(|s| s.bg(Background::PROMINENT))
                        .when(armed, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .on_click(cx.listener(move |this, _, window: &mut Window, cx| {
                            on_activate(this, i, window, cx);
                        }))
                        .child(div().flex_1().child(item.display_label(armed)))
                        .when_some(item.hint.clone(), |el, hint| {
                            el.child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child(hint),
                            )
                        }),
                );
            }
        }
    }

    deferred(
        anchored()
            .position(position)
            .snap_to_window_with_margin(px(Spacing::SM_MD))
            .child(menu),
    )
    .with_priority(MENU_LAYER_PRIORITY)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<MenuEntry> {
        vec![
            MenuEntry::item("open", "Open"),
            MenuEntry::separator(),
            MenuEntry::destructive("remove", "Remove"),
            MenuEntry::destructive_confirm("delete", "Delete Project", "Confirm Delete"),
        ]
    }

    #[test]
    fn open_at_sets_position_and_target() {
        let mut state = ContextMenuState::new();
        assert!(!state.is_open());
        state.open_at(12.0, 34.0, "proj-1");
        let open = state.open_menu().expect("open");
        assert_eq!((open.x, open.y), (12.0, 34.0));
        assert_eq!(open.target, "proj-1");
        assert_eq!(open.confirming, None);
        assert_eq!(state.target(), Some(&"proj-1"));
    }

    #[test]
    fn close_clears_state() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        state.close();
        assert!(!state.is_open());
        assert!(state.open_menu().is_none());
        assert!(state.target().is_none());
    }

    #[test]
    fn activate_when_closed_is_ignored() {
        let mut state: ContextMenuState<()> = ContextMenuState::new();
        assert_eq!(state.activate(0, &entries()), Activation::Ignored);
    }

    #[test]
    fn activate_separator_is_ignored_and_menu_stays_open() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(1, &entries()), Activation::Ignored);
        assert!(state.is_open());
    }

    #[test]
    fn activate_out_of_bounds_is_ignored() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(99, &entries()), Activation::Ignored);
        assert!(state.is_open());
    }

    #[test]
    fn plain_item_performs_and_closes() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(0, &entries()), Activation::Perform("open"));
        assert!(!state.is_open());
    }

    #[test]
    fn destructive_without_confirm_performs_immediately() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(2, &entries()), Activation::Perform("remove"));
        assert!(!state.is_open());
    }

    #[test]
    fn confirm_item_arms_then_performs() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(3, &entries()), Activation::Armed);
        assert!(state.is_open());
        assert_eq!(state.open_menu().unwrap().confirming, Some(3));
        assert_eq!(state.activate(3, &entries()), Activation::Perform("delete"));
        assert!(!state.is_open());
    }

    #[test]
    fn armed_state_does_not_leak_to_other_items() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, ());
        assert_eq!(state.activate(3, &entries()), Activation::Armed);
        // A plain item still performs immediately while another is armed.
        assert_eq!(state.activate(0, &entries()), Activation::Perform("open"));
        assert!(!state.is_open());
    }

    #[test]
    fn reopen_resets_armed_state() {
        let mut state = ContextMenuState::new();
        state.open_at(0.0, 0.0, "a");
        assert_eq!(state.activate(3, &entries()), Activation::Armed);
        state.open_at(5.0, 5.0, "b");
        assert_eq!(state.open_menu().unwrap().confirming, None);
        // First activation after reopen arms again rather than performing.
        assert_eq!(state.activate(3, &entries()), Activation::Armed);
    }

    #[test]
    fn display_label_swaps_while_armed() {
        let MenuEntry::Item(item) = MenuEntry::destructive_confirm("delete", "Delete", "Confirm")
        else {
            panic!("expected item");
        };
        assert_eq!(item.display_label(false).as_ref(), "Delete");
        assert_eq!(item.display_label(true).as_ref(), "Confirm");

        let MenuEntry::Item(plain) = MenuEntry::item("open", "Open") else {
            panic!("expected item");
        };
        assert_eq!(plain.display_label(true).as_ref(), "Open");
    }
}
