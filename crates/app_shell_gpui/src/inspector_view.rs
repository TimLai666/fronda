/// Inspector panel gpui view — matches Swift InspectorView.swift exactly.
///
/// Two display modes:
///   • no_clip: Project + Format metadata rows, no tab bar
///   • clip_selected: tab bar (Text / Video / Audio / AI Edit) + tab content
///
/// Numeric fields (Volume, Fade In/Out, Position X/Y, Scale, Rotation, Opacity, Speed, Size)
/// are ScrubbableNumberField equivalents: accent-colored values, horizontal drag to scrub,
/// drag threshold handled by gpui on_drag + on_drag_move.

use crate::ai_edit_tab_view::AiEditTabView;
use crate::inspector_model::{InspectorState, InspectorTab};
use crate::keyframes_view::KeyframesView;
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Radius, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, App, Context, DragMoveEvent, Entity, FocusHandle, Focusable,
    IntoElement, InteractiveElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled,
    WeakEntity, Window,
};
use std::collections::HashMap;

// ── Scrub drag infrastructure ─────────────────────────────────────────────────

/// Marker type for inspector scrub drags — matches Swift ScrubbableNumberField gesture.
#[derive(Clone)]
struct ScrubData;

/// Minimal transparent drag-preview view required by gpui's on_drag API.
struct ScrubPreview;
impl Render for ScrubPreview {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

/// State captured at drag-start for delta computation.
#[derive(Clone)]
struct ScrubSession {
    field: &'static str,
    start_x: f32,
    start_value: f32,
    sensitivity: f32,
    min: f32,
    max: f32,
}

// ── Default numeric values ────────────────────────────────────────────────────

fn default_scrub_values() -> HashMap<&'static str, f32> {
    [
        ("volume", 0.0_f32),   // 0 dB = unity gain
        ("fade_in", 0.0),
        ("fade_out", 0.0),
        ("position_x", 0.0),
        ("position_y", 0.0),
        ("scale", 100.0),
        ("rotation", 0.0),
        ("opacity", 100.0),
        ("speed", 1.0),        // 1.0× = normal speed
        ("text_size", 48.0),
    ]
    .into_iter()
    .collect()
}

/// Format a scrub value for display, matching Swift inspector labels.
fn fmt_scrub(field: &'static str, v: f32) -> String {
    match field {
        "fade_in" | "fade_out" => format!("{:.1} s", v),
        "position_x" | "position_y" | "text_size" => format!("{:.0}", v),
        "rotation" => format!("{:.0}°", v),
        // Volume uses dB scale: -60 floor (shown as "–∞ dB"), +15 ceiling
        "volume" => {
            if v <= -60.0 { "–∞ dB".to_string() } else { format!("{:.1} dB", v) }
        }
        // Speed uses multiplier notation (0.25×–4.0×)
        "speed" => format!("{:.2}×", v),
        _ => format!("{:.0}%", v), // scale, opacity
    }
}

// ── View ─────────────────────────────────────────────────────────────────────

pub struct InspectorView {
    pub state: InspectorState,
    pub has_clip_selected: bool,
    /// True when a media asset in the library panel is selected (Swift: Source mode).
    pub has_media_asset_selected: bool,
    ai_edit_view: Entity<AiEditTabView>,
    keyframes_view: Entity<KeyframesView>,
    focus_handle: FocusHandle,
    /// Current numeric values for all scrub fields.
    pub scrub_values: HashMap<&'static str, f32>,
    /// Drag session in progress — set on mouse-down, read during on_drag_move.
    active_scrub: Option<ScrubSession>,
}

impl InspectorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: InspectorState::new(),
            has_clip_selected: false,
            has_media_asset_selected: false,
            ai_edit_view: cx.new(|cx| AiEditTabView::new(cx)),
            keyframes_view: cx.new(|cx| KeyframesView::new(cx)),
            focus_handle: cx.focus_handle(),
            scrub_values: default_scrub_values(),
            active_scrub: None,
        }
    }

    pub fn select_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.state.select_tab(tab);
        cx.notify();
    }

    pub fn toggle_transform(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_transform();
        cx.notify();
    }

    pub fn toggle_volume(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_volume();
        cx.notify();
    }

    pub fn toggle_keyframes(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_keyframes();
        cx.notify();
    }

    fn scrub_value(&self, field: &'static str) -> f32 {
        self.scrub_values.get(field).copied().unwrap_or(0.0)
    }

    /// Creates a scrubable numeric property row — matches Swift ScrubbableNumberField.
    ///
    /// `keyframeable`: when true, appends ◆ ‹ › keyframe buttons (Swift: keyframe control strip).
    fn scrub_row(
        &self,
        field: &'static str,
        label: &str,
        min: f32,
        max: f32,
        sensitivity: f32,
        keyframeable: bool,
        weak: WeakEntity<Self>,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let value = self.scrub_value(field);
        let display = fmt_scrub(field, value);
        let weak_down = weak.clone();
        let weak_drag = weak;

        div()
            .id(format!("scrub-{field}"))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px(px(Spacing::LG))
            .h(px(22.0))
            .child(
                div()
                    .flex_1()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::XS))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_color(Accent::PRIMARY)
                    .text_size(px(FontSize::XS))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .child(display),
            )
            // Keyframe controls: ‹ ◆ › (add keyframe, prev, next)
            .when(keyframeable, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(1.0))
                        .ml(px(4.0))
                        .child(
                            div()
                                .id(format!("kf-prev-{field}"))
                                .w(px(14.0))
                                .h(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XS))
                                .child("‹"),
                        )
                        .child(
                            div()
                                .id(format!("kf-add-{field}"))
                                .w(px(12.0))
                                .h(px(12.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::SM))
                                .child("◆"),
                        )
                        .child(
                            div()
                                .id(format!("kf-next-{field}"))
                                .w(px(14.0))
                                .h(px(14.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XS))
                                .child("›"),
                        ),
                )
            })
            // Record drag start: global mouse position + current value
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this: &mut InspectorView, event: &MouseDownEvent, _window, _cx| {
                    this.active_scrub = Some(ScrubSession {
                        field,
                        start_x: event.position.x.as_f32(),
                        start_value: this.scrub_value(field),
                        sensitivity,
                        min,
                        max,
                    });
                }),
            )
            // Initiate gpui drag — required to activate on_drag_move globally
            .on_drag(ScrubData, move |_, _offset, _window, cx: &mut App| {
                cx.new(|_| ScrubPreview)
            })
    }
}

impl Focusable for InspectorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── Static row helpers ────────────────────────────────────────────────────────

fn prop_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(Spacing::LG))
        .h(px(22.0))
        .child(
            div()
                .flex_1()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(value.to_string()),
        )
}

fn section_header(id: &str, label: &str, expanded: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(Spacing::LG))
        .h(px(28.0))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child(if expanded { "v" } else { ">" }),
        )
}

fn project_metadata_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::MD))
                .gap(px(Spacing::XXS))
                .child(section_header("section-project", "Project", true))
                .child(prop_row("Name", "Untitled"))
                .child(prop_row("Path", "~/Movies/Untitled.palmier")),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::SM))
                .gap(px(Spacing::XXS))
                .child(section_header("section-format", "Format", true))
                .child(prop_row("Resolution", "1920 x 1080"))
                .child(prop_row("Frame Rate", "30 fps"))
                .child(prop_row("Aspect Ratio", "16:9"))
                .child(prop_row("Duration", "0:20")),
        )
}

/// Source mode — displayed when a media asset (not timeline clip) is selected.
/// Matches Swift InspectorView.assetDetailsContent.
fn source_media_content(name: &str, media_type: &str) -> impl IntoElement {
    use crate::theme::FontSize as FS;
    div()
        .flex()
        .flex_col()
        .w_full()
        .pt(px(Spacing::MD))
        .px(px(Spacing::LG))
        .gap(px(Spacing::XL))
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FS::MD_LG))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(name.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::XXS))
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FS::XXS))
                        .child("FILE"),
                )
                .child(prop_row("Type", media_type))
                .child(prop_row("Dimensions", "1920 × 1080"))
                .child(prop_row("Duration", "0:10"))
                .child(prop_row("Size", "42.3 MB"))
                .child(prop_row("Path", "~/Movies/clip.mp4")),
        )
}

fn keyframes_btn(id: &str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .text_color(if active { Text::PRIMARY } else { Text::TERTIARY })
        .text_size(px(FontSize::XS))
        .cursor_pointer()
        .child("Keyframes")
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab.clone();
        let transform_expanded = self.state.transform_expanded;
        let volume_expanded = self.state.volume_expanded;
        let kf_visible = self.state.keyframes_visible;
        let has_clip = self.has_clip_selected;
        let has_asset = self.has_media_asset_selected;
        let title = if has_asset { "Source" } else if has_clip { "Inspector" } else { "Timeline" };
        let icon = if has_asset { "◈" } else if has_clip { "⊙" } else { "i" };

        let ai_edit_entity = self.ai_edit_view.clone();
        let kf_entity = self.keyframes_view.clone();

        // WeakEntity captured for scrub row creation and for on_drag_move
        let weak = cx.entity().downgrade();
        let weak_drag = weak.clone();

        // Build interactive scrub rows for all numeric fields
        // Volume: -60 dB (floor) to +15 dB, 0.5 dB/px sensitivity (matches Swift VolumeScale)
        let vol_row = self.scrub_row("volume", "Volume", -60.0, 15.0, 0.5, false, weak.clone(), cx);
        let fade_in_row = self.scrub_row("fade_in", "Fade In", 0.0, 10.0, 0.05, false, weak.clone(), cx);
        let fade_out_row = self.scrub_row("fade_out", "Fade Out", 0.0, 10.0, 0.05, false, weak.clone(), cx);
        let pos_x_row = self.scrub_row("position_x", "Position X", -9999.0, 9999.0, 2.0, true, weak.clone(), cx);
        let pos_y_row = self.scrub_row("position_y", "Position Y", -9999.0, 9999.0, 2.0, true, weak.clone(), cx);
        let scale_row = self.scrub_row("scale", "Scale", 1.0, 1000.0, 1.0, true, weak.clone(), cx);
        let rotation_row = self.scrub_row("rotation", "Rotation", -360.0, 360.0, 1.0, true, weak.clone(), cx);
        let opacity_row = self.scrub_row("opacity", "Opacity", 0.0, 100.0, 0.5, true, weak.clone(), cx);
        // Speed: 0.25× to 4.0×, 0.01/px sensitivity (matches Swift speedRange 0.25...4.0, suffix "x")
        let speed_row = self.scrub_row("speed", "Speed", 0.25, 4.0, 0.01, false, weak.clone(), cx);
        let text_size_row = self.scrub_row("text_size", "Size", 1.0, 1000.0, 0.5, false, weak.clone(), cx);

        // Levels section (clickable header — toggles volume_expanded)
        let levels_section = div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                section_header("section-levels", "Levels", volume_expanded)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_volume(cx)))
            )
            .when(volume_expanded, |el| {
                el.child(vol_row)
                    .child(fade_in_row)
                    .child(fade_out_row)
            });

        // Transform section (clickable header — toggles transform_expanded)
        let transform_section = div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                section_header("section-transform", "Transform", transform_expanded)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_transform(cx)))
            )
            .when(transform_expanded, |el| {
                el.child(pos_x_row)
                    .child(pos_y_row)
                    .child(scale_row)
                    .child(rotation_row)
                    .child(opacity_row)
                    .child(prop_row("Crop", "None"))
                    .child(prop_row("Flip", "None"))
            });

        // Speed section
        let speed_section = div()
            .flex()
            .flex_col()
            .w_full()
            .child(section_header("section-playback", "Playback", true))
            .child(speed_row);

        // Text tab size row
        // Alignment toggle row: 4 icon buttons (≡ left, center, right, justify)
        let align_row = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(28.0))
            .px(px(Spacing::SM_MD))
            .gap(px(Spacing::XS))
            .child(
                div()
                    .flex_1()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::SM))
                    .child("Alignment"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::XXS))
                    .children(["⬛", "⬜", "⬛", "⬛"].iter().enumerate().map(|(i, glyph)| {
                        let icons = ["text.alignleft", "text.aligncenter", "text.alignright", "text.justify"];
                        let labels = ["◀▌", "▌◀▶▌", "▌▶", "≡"];
                        let active = i == 1; // center is default active
                        let _ = glyph; let _ = icons;
                        div()
                            .id(format!("align-btn-{i}"))
                            .w(px(22.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS))
                            .bg(if active { BorderColors::SUBTLE } else { Background::SURFACE })
                            .cursor_pointer()
                            .text_size(px(FontSize::XXS))
                            .text_color(if active { Text::PRIMARY } else { Text::TERTIARY })
                            .child(labels[i])
                    })),
            );

        let text_size_section = div()
            .flex()
            .flex_col()
            .w_full()
            .pt(px(Spacing::MD))
            .child(section_header("section-text-content", "Content", true))
            .child(prop_row("Text", "Title"))
            .child(prop_row("Font", "System"))
            .child(text_size_row)
            .child(prop_row("Color", "White"))
            .child(align_row);

        div()
            .id("inspector-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // on_drag_move fires globally while a ScrubData drag is active.
            // Computes delta from active_scrub.start_x and updates the field value.
            .on_drag_move::<ScrubData>(move |event: &DragMoveEvent<ScrubData>, _window, cx: &mut App| {
                let _ = weak_drag.update(cx, |this: &mut InspectorView, inner_cx| {
                    if let Some(ref session) = this.active_scrub {
                        let delta = event.event.position.x.as_f32() - session.start_x;
                        let new_val = (session.start_value + delta * session.sensitivity)
                            .clamp(session.min, session.max);
                        this.scrub_values.insert(session.field, new_val);
                        inner_cx.notify();
                    }
                });
            })
            // Header
            .child(
                div()
                    .id("inspector-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::LG))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(title),
                    ),
            )
            // Body
            .child(
                div()
                    .id("inspector-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .when(!has_clip && !has_asset, |el| el.child(project_metadata_content()))
                    .when(has_asset, |el| {
                        el.child(source_media_content("Interview A-roll", "Video"))
                    })
                    .when(has_clip, |el| {
                        el
                            // Tab bar
                            .child(
                                div()
                                    .id("inspector-tabs")
                                    .flex()
                                    .flex_row()
                                    .items_end()
                                    .w_full()
                                    .px(px(Spacing::LG))
                                    .pt(px(Spacing::XS))
                                    .gap(px(Spacing::MD_LG))
                                    .bg(Background::SURFACE)
                                    .border_b_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .children(InspectorTab::all_tabs().iter().map(|tab| {
                                        let is_active = *tab == active_tab;
                                        let is_ai = *tab == InspectorTab::AiEdit;
                                        let tab_clone = tab.clone();
                                        div()
                                            .id(tab.label())
                                            .pb(px(Spacing::XS))
                                            .cursor_pointer()
                                            .text_color(if is_ai {
                                                Accent::PRIMARY
                                            } else if is_active {
                                                Text::PRIMARY
                                            } else {
                                                Text::TERTIARY
                                            })
                                            .text_size(px(FontSize::SM))
                                            .font_weight(if is_active { gpui::FontWeight::MEDIUM } else { gpui::FontWeight::NORMAL })
                                            .border_b(px(if is_active { 1.5 } else { 0.0 }))
                                            .border_color(if is_ai { Accent::PRIMARY } else { Text::PRIMARY })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_tab(tab_clone.clone(), cx);
                                            }))
                                            .child(tab.label())
                                    })),
                            )
                            // Tab content
                            .child(match active_tab {
                                InspectorTab::Video => {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .child(levels_section)
                                        .child(transform_section)
                                        .child(speed_section)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .justify_end()
                                                .w_full()
                                                .px(px(Spacing::LG))
                                                .py(px(Spacing::XS))
                                                .child(
                                                    keyframes_btn("kf-toggle-video", kf_visible)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_keyframes(cx);
                                                        })),
                                                ),
                                        )
                                        .when(kf_visible, |el| {
                                            el.child(
                                                div()
                                                    .w_full()
                                                    .border_t_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .child(kf_entity.clone()),
                                            )
                                        })
                                        .into_any_element()
                                }
                                InspectorTab::Audio => {
                                    // Reuse levels + speed for audio tab
                                    let vol_row2 = div()
                                        .flex().flex_col().w_full()
                                        .child(section_header("section-levels-audio", "Levels", volume_expanded)
                                            .on_click(cx.listener(|this, _, _, cx| this.toggle_volume(cx))))
                                        .when(volume_expanded, |el| {
                                            let v_row = div()
                                                .id("scrub-volume-audio")
                                                .flex().flex_row().items_center().w_full()
                                                .px(px(Spacing::LG)).h(px(22.0))
                                                .child(div().flex_1().text_color(Text::TERTIARY).text_size(px(FontSize::XS)).child("Volume"))
                                                .child(div().text_color(Accent::PRIMARY).text_size(px(FontSize::XS)).font_weight(gpui::FontWeight::MEDIUM).cursor_pointer().child(fmt_scrub("volume", self.scrub_value("volume"))));
                                            let fi_row = div()
                                                .id("scrub-fade_in-audio")
                                                .flex().flex_row().items_center().w_full()
                                                .px(px(Spacing::LG)).h(px(22.0))
                                                .child(div().flex_1().text_color(Text::TERTIARY).text_size(px(FontSize::XS)).child("Fade In"))
                                                .child(div().text_color(Accent::PRIMARY).text_size(px(FontSize::XS)).font_weight(gpui::FontWeight::MEDIUM).cursor_pointer().child(fmt_scrub("fade_in", self.scrub_value("fade_in"))));
                                            let fo_row = div()
                                                .id("scrub-fade_out-audio")
                                                .flex().flex_row().items_center().w_full()
                                                .px(px(Spacing::LG)).h(px(22.0))
                                                .child(div().flex_1().text_color(Text::TERTIARY).text_size(px(FontSize::XS)).child("Fade Out"))
                                                .child(div().text_color(Accent::PRIMARY).text_size(px(FontSize::XS)).font_weight(gpui::FontWeight::MEDIUM).cursor_pointer().child(fmt_scrub("fade_out", self.scrub_value("fade_out"))));
                                            el.child(v_row).child(fi_row).child(fo_row)
                                        });
                                    let spd_row2 = div()
                                        .id("scrub-speed-audio")
                                        .flex().flex_row().items_center().w_full()
                                        .px(px(Spacing::LG)).h(px(22.0))
                                        .child(div().flex_1().text_color(Text::TERTIARY).text_size(px(FontSize::XS)).child("Speed"))
                                        .child(div().text_color(Accent::PRIMARY).text_size(px(FontSize::XS)).font_weight(gpui::FontWeight::MEDIUM).cursor_pointer().child(fmt_scrub("speed", self.scrub_value("speed"))));
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .child(vol_row2)
                                        .child(div().flex().flex_col().w_full().child(section_header("section-audio-playback", "Playback", true)).child(spd_row2))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .justify_end()
                                                .w_full()
                                                .px(px(Spacing::LG))
                                                .py(px(Spacing::XS))
                                                .child(
                                                    keyframes_btn("kf-toggle-audio", kf_visible)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_keyframes(cx);
                                                        })),
                                                ),
                                        )
                                        .when(kf_visible, |el| {
                                            el.child(
                                                div()
                                                    .w_full()
                                                    .border_t_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .child(kf_entity.clone()),
                                            )
                                        })
                                        .into_any_element()
                                }
                                InspectorTab::Text => text_size_section.into_any_element(),
                                InspectorTab::AiEdit => ai_edit_entity.clone().into_any_element(),
                            })
                    }),
            )
    }
}
