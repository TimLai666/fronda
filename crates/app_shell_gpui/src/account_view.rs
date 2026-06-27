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
    /// Whether the account is a paid subscriber (controls upgradeBlock visibility).
    pub is_paid: bool,
    /// Whether the subscription will cancel at period end (shows orange "Cancels" banner).
    pub cancel_at_period_end: bool,
    /// Credit fraction 0.0–1.0
    pub credit_fraction: f32,
    pub credits_left: u32,
    pub credits_total: u32,
    /// Optional reset date label (e.g. "Jul 1") — shown as "Resets Jul 1" next to the credit count.
    /// Matches Swift creditsBlock's "Resets \(date)" label.
    pub credits_reset_date: Option<String>,
}

impl AccountState {
    pub fn signed_out() -> Self {
        Self {
            is_signed_in: false,
            display_name: "Not signed in".to_string(),
            email: String::new(),
            plan_label: "Free".to_string(),
            is_paid: false,
            cancel_at_period_end: false,
            credit_fraction: 0.0,
            credits_left: 0,
            credits_total: 0,
            credits_reset_date: None,
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
fn footer_btn(id: &str, icon: &str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                let plan_label = state.plan_label.clone();
                let cancel_at = state.cancel_at_period_end;
                let credits_left = state.credits_left;
                let credits_total = state.credits_total;
                let credit_frac = state.credit_fraction;
                let is_paid = state.is_paid;
                let reset_date = state.credits_reset_date.clone();

                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::SM))
                        // Plan label row + optional "Cancels" banner
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::MD))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(plan_label),
                                )
                                .when(cancel_at, |el| {
                                    el.child(
                                        div()
                                            .text_color(gpui::Hsla { h: 30.0 / 360.0, s: 0.95, l: 0.60, a: 1.0 })
                                            .text_size(px(FontSize::XXS))
                                            .child("Cancels soon"),
                                    )
                                }),
                        )
                        // Credit progress bar
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(Spacing::XS))
                                .child(
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
                                                .w(px(credit_frac * CARD_WIDTH * 0.85))
                                                .rounded_full()
                                                .bg(bar_color),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .child(
                                            div()
                                                .flex_1()
                                                .text_color(Text::SECONDARY)
                                                .text_size(px(FontSize::SM))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(format!("{} / {} credits", credits_left, credits_total)),
                                        )
                                        .when_some(reset_date, |el, date| {
                                            el.child(
                                                div()
                                                    .text_color(Text::TERTIARY)
                                                    .text_size(px(FontSize::XS))
                                                    .child(format!("Resets {date}")),
                                            )
                                        }),
                                ),
                        )
                        // upgradeBlock — shown when not paid (Swift: if !account.isPaid)
                        .when(!is_paid, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::XS))
                                    // Pro plan row
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .child(
                                                div()
                                                    .text_color(Text::PRIMARY)
                                                    .text_size(px(FontSize::SM))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child("Pro"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::SECONDARY)
                                                    .text_size(px(FontSize::SM))
                                                    .child("$29/mo"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::TERTIARY)
                                                    .text_size(px(FontSize::XS))
                                                    .child("1.5k credits"),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                div()
                                                    .id("btn-upgrade-pro-card")
                                                    .px(px(Spacing::SM_MD))
                                                    .py(px(2.0))
                                                    .rounded_full()
                                                    .bg(Accent::PRIMARY)
                                                    .cursor_pointer()
                                                    .text_color(Background::BASE)
                                                    .text_size(px(FontSize::XS))
                                                    .child("Upgrade"),
                                            ),
                                    )
                                    // Max plan row
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .child(
                                                div()
                                                    .text_color(Text::PRIMARY)
                                                    .text_size(px(FontSize::SM))
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .child("Max"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::SECONDARY)
                                                    .text_size(px(FontSize::SM))
                                                    .child("$99/mo"),
                                            )
                                            .child(
                                                div()
                                                    .text_color(Text::TERTIARY)
                                                    .text_size(px(FontSize::XS))
                                                    .child("6k credits"),
                                            )
                                            .child(div().flex_1())
                                            .child(
                                                div()
                                                    .id("btn-upgrade-max-card")
                                                    .px(px(Spacing::SM_MD))
                                                    .py(px(2.0))
                                                    .rounded_full()
                                                    .border_1()
                                                    .border_color(BorderColors::PRIMARY)
                                                    .cursor_pointer()
                                                    .text_color(Text::SECONDARY)
                                                    .text_size(px(FontSize::XS))
                                                    .child("Upgrade"),
                                            ),
                                    ),
                            )
                        }),
                )
                .child(div().w_full().h(px(1.0)).bg(BorderColors::SUBTLE))
            })
            // ── Footer: Settings, Feedback, Sign in/out ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XXS))
                    .child(footer_btn("footer-settings", "⚙", "Settings")
                        .on_click(cx.listener(|_, _, _, _| {})))
                    .child(footer_btn("footer-feedback", "✉", "Feedback")
                        .on_click(cx.listener(|_, _, _, _| {})))
                    .child(if state.is_signed_in {
                        footer_btn("footer-signout", "→", "Sign out")
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .into_any_element()
                    } else {
                        footer_btn("footer-signin", "↩", "Sign in")
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .into_any_element()
                    }),
            )
    }
}
