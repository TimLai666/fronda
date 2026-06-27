//! Account popover view — matches Swift AccountPopoverCard.
//!
//! Shows when user clicks the avatar button in the title bar.
//! Dimensions: 280px wide (Swift cardWidth = 280).

use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Account card width (Swift: cardWidth = 280).
const CARD_WIDTH: f32 = 280.0;

/// Signed-in state for the account card.
#[derive(Debug, Clone, Default)]
pub struct AccountState {
    pub is_signed_in: bool,
    pub display_name: String,
    pub email: String,
    pub plan_label: String,
    /// Credit fraction 0.0–1.0
    pub credit_fraction: f32,
    pub credits_left: u32,
    pub credits_total: u32,
}

impl AccountState {
    pub fn signed_out() -> Self {
        Self {
            is_signed_in: false,
            display_name: "Not signed in".to_string(),
            email: String::new(),
            plan_label: "Free".to_string(),
            credit_fraction: 0.0,
            credits_left: 0,
            credits_total: 0,
        }
    }
}

/// Account popover card view entity.
pub struct AccountView {
    pub state: AccountState,
    focus_handle: FocusHandle,
}

impl AccountView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: AccountState::signed_out(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for AccountView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A footer action button row (Settings / Feedback / Sign in).
fn footer_btn(icon: &str, label: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .w_full()
        .px(px(Spacing::SM))
        .py(px(Spacing::XS))
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM_MD))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
}

impl Render for AccountView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let state = &self.state;
        let initials = state
            .display_name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>();
        let initials = if initials.is_empty() { "?".to_string() } else { initials };

        // Credit bar color: red < 5%, orange < 25%, else accent
        let bar_color: Hsla = if state.credit_fraction < 0.05 {
            Hsla { h: 0.0, s: 0.75, l: 0.55, a: 1.0 }
        } else if state.credit_fraction < 0.25 {
            Hsla { h: 35.0 / 360.0, s: 0.90, l: 0.55, a: 1.0 }
        } else {
            Accent::PRIMARY
        };

        div()
            .id("account-card")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w(px(CARD_WIDTH))
            .bg(Background::RAISED)
            .rounded(px(Radius::MD_LG))
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .p(px(Spacing::MD))
            .gap(px(Spacing::SM))
            // ── Identity block ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::MD))
                    // Avatar circle (Swift: UserAvatar diameter=xl = 32px)
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded_full()
                            .bg(Accent::PRIMARY)
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::MD_LG))
                            .child(initials),
                    )
                    // Name + email
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(Spacing::XXS))
                            .child(
                                div()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::MD))
                                    .child(state.display_name.clone()),
                            )
                            .when(!state.email.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(FontSize::XS))
                                        .child(state.email.clone()),
                                )
                            }),
                    ),
            )
            // ── Divider ──
            .child(div().w_full().h(px(1.0)).bg(BorderColors::SUBTLE))
            // ── Plan + credits (when signed in) ──
            .when(state.is_signed_in, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::SM))
                        // Plan label
                        .child(
                            div()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .child(state.plan_label.clone()),
                        )
                        // Credit progress bar
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(Spacing::XS))
                                .child(
                                    // Bar track
                                    div()
                                        .relative()
                                        .w_full()
                                        .h(px(4.0))
                                        .rounded_full()
                                        .bg(BorderColors::SUBTLE)
                                        .child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .h_full()
                                                .w(px(state.credit_fraction * CARD_WIDTH * 0.85))
                                                .rounded_full()
                                                .bg(bar_color),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_color(Text::SECONDARY)
                                                .text_size(px(FontSize::SM))
                                                .child(format!(
                                                    "{} / {} credits",
                                                    state.credits_left, state.credits_total
                                                )),
                                        ),
                                ),
                        ),
                )
                .child(div().w_full().h(px(1.0)).bg(BorderColors::SUBTLE))
            })
            // ── Footer: Settings, Feedback, Sign in/out ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XXS))
                    .child(footer_btn("⚙", "Settings"))
                    .child(footer_btn("✉", "Feedback"))
                    .child(if state.is_signed_in {
                        footer_btn("→", "Sign out").into_any_element()
                    } else {
                        footer_btn("↩", "Sign in").into_any_element()
                    }),
            )
    }
}
