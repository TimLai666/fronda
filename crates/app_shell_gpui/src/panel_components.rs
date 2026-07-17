//! Shared editor-panel chrome (upstream #327).
//!
//! gpui counterparts of Swift `EditorPanelGroup` / `EditorActionFooter` /
//! `EditorPanelControls` (EditorMenuValue, editorValueField, editorPrimary
//! button style) plus the pure tab/group structure the inspector and media
//! panel assemble. Collapse state lives in the owning view for the session
//! (Swift `@State` equivalent) via [`GroupStates`].

use crate::theme::{
    Accent, Background, BorderColors, EditorPanel, FontSize, IconSize, Opacity, Radius, Spacing,
    Status, Text,
};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Div, IntoElement, ParentElement, RenderOnce,
    SharedString, Stateful, Styled, Window,
};
use std::collections::HashMap;

// ── Pure tab structure ────────────────────────────────────────────────────────

/// Inspector clip tabs — mirrors Swift `InspectorView.ClipTab` (#327).
///
/// `Animate` (text-animation presets) is omitted: the caption preset gallery
/// has no Rust UI yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipTab {
    Text,
    Video,
    Adjust,
    Audio,
    Multicam,
    AiEdit,
}

impl ClipTab {
    /// Swift rawValue shown in the tab bar.
    pub fn label(self) -> &'static str {
        match self {
            ClipTab::Text => "Content",
            ClipTab::Video => "Video",
            ClipTab::Adjust => "Adjust",
            ClipTab::Audio => "Audio",
            ClipTab::Multicam => "Multicam",
            ClipTab::AiEdit => "AI Edit",
        }
    }
}

/// Selection shape driving tab availability (Swift `availableTabs` inputs).
#[derive(Debug, Clone, Copy, Default)]
pub struct TabSelection {
    pub text_clips: usize,
    pub non_text_visual_clips: usize,
    pub audio_clips: usize,
    pub has_multicam_group: bool,
    pub ai_eligible: bool,
}

/// Swift `InspectorView.availableTabs`, minus the Animate tab.
pub fn available_tabs(sel: TabSelection) -> Vec<ClipTab> {
    let is_text_only =
        sel.text_clips > 0 && sel.non_text_visual_clips == 0 && sel.audio_clips == 0;
    let mut tabs = Vec::new();
    if is_text_only {
        tabs.push(ClipTab::Text);
    }
    if sel.non_text_visual_clips > 0 {
        tabs.push(ClipTab::Video);
        tabs.push(ClipTab::Adjust);
    }
    if sel.audio_clips > 0 {
        tabs.push(ClipTab::Audio);
    }
    if sel.has_multicam_group {
        tabs.push(ClipTab::Multicam);
    }
    if sel.ai_eligible {
        tabs.push(ClipTab::AiEdit);
    }
    tabs
}

/// Swift `activeTab`: the preferred tab when available, else the first.
pub fn resolve_active_tab(preferred: ClipTab, tabs: &[ClipTab]) -> Option<ClipTab> {
    if tabs.contains(&preferred) {
        Some(preferred)
    } else {
        tabs.first().copied()
    }
}

/// Swift `resolvePreferredTab` (runs on selection change only): a single text
/// clip prefers Content; a stale Content preference falls back to Video.
pub fn resolve_preferred_tab(preferred: ClipTab, sel: TabSelection) -> ClipTab {
    let single_text =
        sel.text_clips == 1 && sel.non_text_visual_clips == 0 && sel.audio_clips == 0;
    if single_text {
        ClipTab::Text
    } else if preferred == ClipTab::Text {
        ClipTab::Video
    } else {
        preferred
    }
}

// ── Group structure specs (report/test pinning) ───────────────────────────────

/// `(title, default_expanded)` per tab, mirroring the Swift #327 layout for
/// the controls that exist on the Rust side. Gaps vs Swift are intentional
/// and documented in the change report (no empty shells).
pub const VIDEO_TAB_GROUPS: &[(&str, bool)] = &[("Transform", true), ("Playback", true)];
/// Swift AudioTab also has an "Enhance" (denoise) group — no Rust denoise UI yet.
pub const AUDIO_TAB_GROUPS: &[(&str, bool)] = &[("Levels", true), ("Playback", true)];
/// Swift TextTab: "Text" + TextStyleControls ("Style"/"Outline"/"Shadow"/"Background").
pub const TEXT_TAB_GROUPS: &[(&str, bool)] = &[
    ("Text", true),
    ("Style", true),
    ("Outline", true),
    ("Shadow", true),
    ("Background", true),
];
/// Swift AdjustTab sections are Basic Correction / Curves / Color Wheels /
/// Hue Curves / LUTs / Effects; only Effects (Chroma Key) has Rust controls.
/// Swift defaults "Effects" collapsed, and every subgroup collapsed.
pub const ADJUST_TAB_GROUPS: &[(&str, bool)] = &[("Effects", false)];
pub const ADJUST_CHROMA_SUBGROUP: (&str, bool) = ("Chroma Key", false);
/// Swift CaptionTab also has an "Animation" group (preset gallery) — no Rust UI yet.
pub const CAPTIONS_TAB_GROUPS: &[(&str, bool)] = &[
    ("Source", true),
    ("Settings", true),
    ("Style", false),
    ("Placement", true),
];
pub const MUSIC_TAB_GROUPS: &[(&str, bool)] = &[("Music", true)];
/// Inspector with no selection (Swift projectMetadataContent).
pub const PROJECT_METADATA_GROUPS: &[(&str, bool)] = &[("Project", true), ("Settings", true)];

/// Session-scoped collapse state for panel groups (Swift `@State` equivalent).
#[derive(Debug, Default)]
pub struct GroupStates {
    overrides: HashMap<&'static str, bool>,
}

impl GroupStates {
    pub fn expanded(&self, key: &'static str, default_expanded: bool) -> bool {
        self.overrides.get(key).copied().unwrap_or(default_expanded)
    }

    pub fn toggle(&mut self, key: &'static str, default_expanded: bool) {
        let cur = self.expanded(key, default_expanded);
        self.overrides.insert(key, !cur);
    }
}

// ── EditorPanelGroup ──────────────────────────────────────────────────────────

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Collapsible panel group — Swift `EditorPanelGroup`.
///
/// Header: chevron + title, optional trailing accessory, optional reset
/// button. The whole header toggles expansion; accessory/reset handlers must
/// stop propagation (the helpers here do).
#[derive(IntoElement)]
pub struct EditorPanelGroup {
    id: SharedString,
    title: SharedString,
    expanded: bool,
    content_spacing: f32,
    header_accessory: Option<AnyElement>,
    reset: Option<AnyElement>,
    on_toggle: Option<ClickHandler>,
    children: Vec<AnyElement>,
}

impl EditorPanelGroup {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            expanded: true,
            content_spacing: Spacing::SM_MD,
            header_accessory: None,
            reset: None,
            on_toggle: None,
            children: Vec::new(),
        }
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn content_spacing(mut self, spacing: f32) -> Self {
        self.content_spacing = spacing;
        self
    }

    pub fn header_accessory(mut self, accessory: AnyElement) -> Self {
        self.header_accessory = Some(accessory);
        self
    }

    /// Pre-wired reset button (see [`editor_reset_button`]).
    pub fn reset(mut self, reset: AnyElement) -> Self {
        self.reset = Some(reset);
        self
    }

    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Box::new(handler));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }
}

impl RenderOnce for EditorPanelGroup {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let chevron = if self.expanded { "▾" } else { "▸" };
        let mut header = div()
            .id(SharedString::from(format!("{}-header", self.id)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .min_h(px(EditorPanel::GROUP_HEADER_HEIGHT))
            .px(px(Spacing::SM_MD))
            .gap(px(Spacing::SM))
            .bg(Background::SURFACE)
            .cursor_pointer()
            .child(
                div()
                    .w(px(IconSize::XS))
                    .flex()
                    .justify_center()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::XXS))
                    .child(chevron),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(self.title),
            );
        if let Some(accessory) = self.header_accessory {
            header = header.child(accessory);
        }
        if let Some(reset) = self.reset {
            header = header.child(reset);
        }
        if let Some(on_toggle) = self.on_toggle {
            header = header.on_click(move |e, w, cx| on_toggle(e, w, cx));
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::PRIMARY)
            .child(header)
            .when(self.expanded, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .gap(px(self.content_spacing))
                        .p(px(Spacing::SM_MD))
                        .children(self.children),
                )
            })
    }
}

// ── EditorActionFooter ────────────────────────────────────────────────────────

/// Footer chrome (Swift `EditorActionFooter`): raised background, top border,
/// optional error note above the action row. Callers add the actions row.
pub fn editor_action_footer() -> Div {
    div()
        .flex()
        .flex_col()
        .w_full()
        .gap(px(Spacing::SM))
        .px(px(Spacing::LG_XL))
        .py(px(Spacing::MD))
        .bg(Background::RAISED)
        .border_t_1()
        .border_color(BorderColors::PRIMARY)
}

/// Error note line inside the footer (Swift: FontSize.xs medium, Status.error).
pub fn footer_note(message: &str) -> Div {
    div()
        .text_color(Status::ERROR)
        .text_size(px(FontSize::XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(message.to_string())
}

// ── EditorPanelControls ───────────────────────────────────────────────────────

/// Value-field chrome (Swift `editorValueField`): base fill, subtle border,
/// XS_SM radius, FIELD_MIN_HEIGHT.
pub fn editor_value_field() -> Div {
    div()
        .min_h(px(EditorPanel::FIELD_MIN_HEIGHT))
        .rounded(px(Radius::XS_SM))
        .bg(Background::BASE)
        .border_1()
        .border_color(BorderColors::SUBTLE)
}

/// Menu value chip (Swift `EditorMenuValue`): current value + ▾ chevron in
/// value-field chrome. Caller attaches `on_click`.
pub fn editor_menu_value(id: impl Into<SharedString>, text: String, expand: bool) -> Stateful<Div> {
    editor_value_field()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .px(px(Spacing::SM_MD))
        .cursor_pointer()
        .when(expand, |el| el.w_full().justify_between())
        .child(
            div()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(text),
        )
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XXS))
                .child("▾"),
        )
}

/// Primary action button (Swift `EditorPrimaryButtonStyle`). Caller attaches
/// `on_click` when enabled and any sizing.
pub fn editor_primary_button(
    id: impl Into<SharedString>,
    label: String,
    enabled: bool,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .px(px(Spacing::MD_LG))
        .py(px(Spacing::SM_MD))
        .rounded(px(Radius::SM))
        .bg(Accent::PRIMARY)
        .text_color(Background::BASE)
        .text_size(px(FontSize::SM))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .opacity(if enabled {
            Opacity::OPAQUE
        } else {
            Opacity::MEDIUM
        })
        .when(enabled, |el| el.cursor_pointer())
        .child(label)
}

/// Reset button (Swift `EditorResetButton`): ↺, stops propagation so a reset
/// inside a group header doesn't also collapse it.
pub fn editor_reset_button(
    id: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .w(px(IconSize::MD))
        .h(px(IconSize::MD))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::XS))
        .cursor_pointer()
        .text_color(Text::TERTIARY)
        .text_size(px(FontSize::SM))
        .child("↺")
        .on_click(move |e, w, cx| {
            cx.stop_propagation();
            on_click(e, w, cx);
        })
}

/// Aligned label/value row (Swift `InspectorRow`): right-aligned label in the
/// fixed label column, trailing content right-aligned, optional reset.
pub fn panel_row(label: &str, trailing: AnyElement) -> Div {
    panel_row_with_reset(label, trailing, None)
}

pub fn panel_row_with_reset(label: &str, trailing: AnyElement, reset: Option<AnyElement>) -> Div {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
        .gap(px(Spacing::SM))
        .child(
            div()
                .w(px(EditorPanel::LABEL_COLUMN_WIDTH))
                .flex_none()
                .flex()
                .justify_end()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_row()
                .items_center()
                .justify_end()
                .child(trailing),
        );
    if let Some(reset) = reset {
        row = row.child(reset);
    }
    row
}

// ── TitleTabBar ───────────────────────────────────────────────────────────────

/// Tab bar container (Swift `TitleTabBar`): full width, TAB_BAR_HEIGHT,
/// raised background, bottom border.
pub fn title_tab_bar() -> Div {
    div()
        .flex()
        .flex_row()
        .w_full()
        .h(px(EditorPanel::TAB_BAR_HEIGHT))
        .bg(Background::RAISED)
        .border_b_1()
        .border_color(BorderColors::PRIMARY)
}

/// One equal-width tab. Active: surface fill, primary text, thick accent
/// underline. Caller attaches `on_click`.
pub fn title_tab(id: impl Into<SharedString>, title: &str, active: bool) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(if active {
            Background::SURFACE
        } else {
            gpui::transparent_black()
        })
        .border_b(px(crate::theme::BorderWidth::THICK))
        .border_color(if active {
            Accent::PRIMARY
        } else {
            gpui::transparent_black()
        })
        .text_size(px(FontSize::SM))
        .font_weight(if active {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .text_color(if active { Text::PRIMARY } else { Text::TERTIARY })
        .child(title.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn titles<'a>(spec: &'a [(&'a str, bool)]) -> Vec<&'a str> {
        spec.iter().map(|(t, _)| *t).collect()
    }

    #[test]
    fn clip_tab_labels_mirror_swift_raw_values() {
        assert_eq!(ClipTab::Text.label(), "Content");
        assert_eq!(ClipTab::Video.label(), "Video");
        assert_eq!(ClipTab::Adjust.label(), "Adjust");
        assert_eq!(ClipTab::Audio.label(), "Audio");
        assert_eq!(ClipTab::Multicam.label(), "Multicam");
        assert_eq!(ClipTab::AiEdit.label(), "AI Edit");
    }

    #[test]
    fn available_tabs_text_only_selection() {
        let tabs = available_tabs(TabSelection {
            text_clips: 1,
            ai_eligible: true,
            ..Default::default()
        });
        assert_eq!(tabs, vec![ClipTab::Text, ClipTab::AiEdit]);
    }

    #[test]
    fn available_tabs_visual_selection_gets_video_and_adjust() {
        let tabs = available_tabs(TabSelection {
            non_text_visual_clips: 1,
            ..Default::default()
        });
        assert_eq!(tabs, vec![ClipTab::Video, ClipTab::Adjust]);
    }

    #[test]
    fn available_tabs_av_pair_orders_video_adjust_audio() {
        let tabs = available_tabs(TabSelection {
            non_text_visual_clips: 1,
            audio_clips: 1,
            has_multicam_group: true,
            ai_eligible: true,
            ..Default::default()
        });
        assert_eq!(
            tabs,
            vec![
                ClipTab::Video,
                ClipTab::Adjust,
                ClipTab::Audio,
                ClipTab::Multicam,
                ClipTab::AiEdit
            ]
        );
    }

    #[test]
    fn available_tabs_text_mixed_with_visual_is_not_text_only() {
        let tabs = available_tabs(TabSelection {
            text_clips: 1,
            non_text_visual_clips: 1,
            ..Default::default()
        });
        assert!(!tabs.contains(&ClipTab::Text));
        assert_eq!(tabs[0], ClipTab::Video);
    }

    #[test]
    fn resolve_active_tab_prefers_then_falls_back_to_first() {
        let tabs = vec![ClipTab::Video, ClipTab::Adjust, ClipTab::Audio];
        assert_eq!(resolve_active_tab(ClipTab::Audio, &tabs), Some(ClipTab::Audio));
        assert_eq!(resolve_active_tab(ClipTab::Text, &tabs), Some(ClipTab::Video));
        assert_eq!(resolve_active_tab(ClipTab::Text, &[]), None);
    }

    #[test]
    fn resolve_preferred_tab_mirrors_swift() {
        // Single text clip → Content.
        let single_text = TabSelection {
            text_clips: 1,
            ..Default::default()
        };
        assert_eq!(resolve_preferred_tab(ClipTab::Video, single_text), ClipTab::Text);
        // Stale Content preference without a text-only selection → Video.
        let visual = TabSelection {
            non_text_visual_clips: 1,
            ..Default::default()
        };
        assert_eq!(resolve_preferred_tab(ClipTab::Text, visual), ClipTab::Video);
        // Anything else is preserved.
        assert_eq!(resolve_preferred_tab(ClipTab::Audio, visual), ClipTab::Audio);
    }

    #[test]
    fn group_states_default_and_toggle() {
        let mut g = GroupStates::default();
        assert!(g.expanded("Transform", true));
        assert!(!g.expanded("Style", false));
        g.toggle("Transform", true);
        assert!(!g.expanded("Transform", true));
        g.toggle("Style", false);
        assert!(g.expanded("Style", false));
        g.toggle("Style", false);
        assert!(!g.expanded("Style", false));
    }

    // Group titles/order/defaults mirror the Swift #327 files (see the change
    // report for the per-file mapping and the documented gaps).
    #[test]
    fn group_specs_mirror_swift_327() {
        assert_eq!(titles(VIDEO_TAB_GROUPS), ["Transform", "Playback"]);
        assert_eq!(titles(AUDIO_TAB_GROUPS), ["Levels", "Playback"]);
        assert_eq!(
            titles(TEXT_TAB_GROUPS),
            ["Text", "Style", "Outline", "Shadow", "Background"]
        );
        assert_eq!(titles(ADJUST_TAB_GROUPS), ["Effects"]);
        assert_eq!(ADJUST_CHROMA_SUBGROUP.0, "Chroma Key");
        assert_eq!(
            titles(CAPTIONS_TAB_GROUPS),
            ["Source", "Settings", "Style", "Placement"]
        );
        assert_eq!(titles(MUSIC_TAB_GROUPS), ["Music"]);
        assert_eq!(titles(PROJECT_METADATA_GROUPS), ["Project", "Settings"]);

        // Swift default collapse states: CaptionTab styleExpanded=false,
        // AdjustTab "Effects" + "Chroma Key" start collapsed, rest expanded.
        assert!(VIDEO_TAB_GROUPS.iter().all(|(_, e)| *e));
        assert!(AUDIO_TAB_GROUPS.iter().all(|(_, e)| *e));
        assert!(TEXT_TAB_GROUPS.iter().all(|(_, e)| *e));
        assert!(!ADJUST_TAB_GROUPS[0].1);
        assert!(!ADJUST_CHROMA_SUBGROUP.1);
        let caption_defaults: Vec<bool> = CAPTIONS_TAB_GROUPS.iter().map(|(_, e)| *e).collect();
        assert_eq!(caption_defaults, [true, true, false, true]);
    }
}
