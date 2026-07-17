/// Inspector panel gpui view — matches Swift InspectorView.swift.
///
/// Display modes:
///   • no selection: Project + Format metadata rows, no tab bar
///   • media asset selected: Source metadata (file / generated / prompt)
///   • clip selected: tab bar (Text / Video / Audio / AI Edit) + tab content
///
/// Numeric rows derive their values from the selected clip (keyframe-resolved
/// at the playhead via timeline_core::resolved_*_at) and write back through the
/// shared ToolExecutor (set_clip_properties / update_text), so undo and MCP see
/// every edit. Selection inputs (`selected_clip_ids`, `selected_media_asset_id`,
/// `playhead_frame`) are public fields the app shell wires from the timeline.
use crate::ai_edit_tab_view::AiEditTabView;
use crate::field_components::{
    color_to_hex, ColorField, ColorFieldEvent, FontPickerEvent, FontPickerField,
};
use crate::inspector_model::InspectorState;
use crate::keyframes_view::KeyframesView;
use crate::panel_components::{
    available_tabs, editor_reset_button, editor_value_field, panel_row, resolve_active_tab,
    resolve_preferred_tab, title_tab, title_tab_bar, ClipTab, EditorPanelGroup, GroupStates,
    TabSelection, ADJUST_CHROMA_SUBGROUP, ADJUST_TAB_GROUPS,
};
use crate::text_area::{TextArea, TextAreaEvent};
use crate::theme::{
    Accent, Background, BorderColors, EditorPanel, FontSize, IconSize, Layout, Opacity, Radius,
    Spacing, Text, TrackColor,
};
use core_model::{Clip, ClipType, MediaManifestEntry, TextAlignment, TextFill, TextStyle};
use gpui::{
    div, prelude::*, px, AnyElement, App, Context, DragMoveEvent, Entity, FocusHandle, Focusable,
    Hsla, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    SharedString, Styled, Window,
};
use std::collections::HashMap;
use std::path::PathBuf;

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
    /// A drag actually moved (plain clicks never set this).
    dragged: bool,
}

// ── Pure value derivation (unit-tested) ───────────────────────────────────────

/// Default row values shown when nothing is selected.
fn default_scrub_values() -> HashMap<&'static str, f32> {
    [
        ("volume", 0.0_f32), // 0 dB = unity gain
        ("fade_in", 0.0),
        ("fade_out", 0.0),
        ("position_x", 0.0),
        ("position_y", 0.0),
        ("scale", 100.0),
        ("rotation", 0.0),
        ("opacity", 100.0),
        ("speed", 1.0), // 1.0× = normal speed
        ("text_size", 96.0),
        ("chroma_tolerance", 0.15),
        ("chroma_softness", 0.1),
        ("chroma_spill", 0.5),
    ]
    .into_iter()
    .collect()
}

/// Clip-relative frame for keyframe resolution, clamped to the clip range.
fn clip_local_frame(clip: &Clip, playhead_frame: i64) -> i64 {
    (playhead_frame - clip.start_frame).clamp(0, clip.duration_frames.max(0))
}

/// Derive every scrub-row value from a clip at the playhead.
///
/// Mirrors Swift InspectorView row bindings: volume in dB (keyframe track
/// sampled directly, else dB of the static gain), fades in seconds, position as
/// top-left canvas pixels, scale/opacity in percent, rotation in degrees.
fn derive_scrub_values(
    clip: &Clip,
    playhead_frame: i64,
    fps: i64,
    canvas_w: i64,
    canvas_h: i64,
) -> HashMap<&'static str, f32> {
    let local = clip_local_frame(clip, playhead_frame);
    let fps = fps.max(1) as f32;

    let volume_db = match &clip.volume_track {
        Some(track) if !track.keyframes.is_empty() => {
            timeline_core::sample_keyframe_track(track, local, 0.0)
        }
        _ => timeline_core::db_from_linear(clip.volume),
    };
    let t = timeline_core::resolved_transform_at(clip, local);
    let top_left_x = (t.center_x - t.width / 2.0) * canvas_w as f64;
    let top_left_y = (t.center_y - t.height / 2.0) * canvas_h as f64;
    let opacity = timeline_core::resolved_opacity_at(clip, local);
    let text_size = clip
        .text_style
        .as_ref()
        .map(|s| s.font_size)
        .unwrap_or_else(|| TextStyle::default().font_size);
    let chroma = crate::chroma_controls::ChromaControls::from_chroma_key(clip.chroma_key.as_ref());

    [
        ("volume", volume_db as f32),
        ("fade_in", clip.fade_in_frames as f32 / fps),
        ("fade_out", clip.fade_out_frames as f32 / fps),
        ("position_x", top_left_x as f32),
        ("position_y", top_left_y as f32),
        ("scale", (t.width * 100.0) as f32),
        ("rotation", t.rotation as f32),
        ("opacity", (opacity * 100.0) as f32),
        ("speed", clip.speed as f32),
        ("text_size", text_size as f32),
        ("chroma_tolerance", chroma.tolerance as f32),
        ("chroma_softness", chroma.softness as f32),
        ("chroma_spill", chroma.spill as f32),
    ]
    .into_iter()
    .collect()
}

/// Tool + args to commit one scrub value for one clip. `None` for fields with
/// no write path yet (fades — no agent tool covers fade frames).
fn scrub_commit_args(
    field: &str,
    value: f64,
    clip: &Clip,
    canvas_w: i64,
    canvas_h: i64,
) -> Option<(&'static str, serde_json::Value)> {
    let props = |properties: serde_json::Value| serde_json::json!({ "clipIds": [clip.id], "properties": properties });
    match field {
        "volume" => {
            let db = value.clamp(
                timeline_core::VOLUME_FLOOR_DB,
                timeline_core::VOLUME_CEILING_DB,
            );
            Some((
                "set_clip_properties",
                props(serde_json::json!({ "volume": timeline_core::linear_from_db(db) })),
            ))
        }
        "opacity" => Some((
            "set_clip_properties",
            props(serde_json::json!({ "opacity": (value / 100.0).clamp(0.0, 1.0) })),
        )),
        "speed" => Some((
            "set_clip_properties",
            props(serde_json::json!({ "speed": value.clamp(0.25, 4.0) })),
        )),
        "rotation" => Some((
            "set_clip_properties",
            props(serde_json::json!({ "transform": { "rotation": value } })),
        )),
        "scale" => {
            let w = (value / 100.0).max(0.01);
            let aspect = if clip.transform.width.abs() > 1e-9 {
                clip.transform.height / clip.transform.width
            } else {
                1.0
            };
            Some((
                "set_clip_properties",
                props(serde_json::json!({ "transform": { "width": w, "height": w * aspect } })),
            ))
        }
        "position_x" => {
            let center_x = value / canvas_w.max(1) as f64 + clip.transform.width / 2.0;
            Some((
                "set_clip_properties",
                props(serde_json::json!({ "transform": { "centerX": center_x } })),
            ))
        }
        "position_y" => {
            let center_y = value / canvas_h.max(1) as f64 + clip.transform.height / 2.0;
            Some((
                "set_clip_properties",
                props(serde_json::json!({ "transform": { "centerY": center_y } })),
            ))
        }
        "text_size" => {
            if clip.media_type != ClipType::Text {
                return None;
            }
            Some((
                "update_text",
                update_text_style_args(
                    &clip.id,
                    serde_json::json!({ "fontSize": value.clamp(12.0, 300.0) }),
                ),
            ))
        }
        "chroma_tolerance" | "chroma_softness" | "chroma_spill" => {
            // Chroma params write the whole key.chroma effect (apply_effect), so
            // carry the other params + colour + enabled from the clip's key.
            let mut c =
                crate::chroma_controls::ChromaControls::from_chroma_key(clip.chroma_key.as_ref());
            let v = value.clamp(0.0, 1.0);
            match field {
                "chroma_tolerance" => c.tolerance = v,
                "chroma_softness" => c.softness = v,
                _ => c.spill = v,
            }
            Some(("apply_effect", c.apply_args(&[clip.id.clone()])))
        }
        _ => None, // fade_in / fade_out: no fade tool in the agent surface yet
    }
}

/// Format a scrub value for display, matching Swift inspector labels.
fn fmt_scrub(field: &'static str, v: f32) -> String {
    match field {
        "fade_in" | "fade_out" => format!("{:.1} s", v),
        "position_x" | "position_y" => format!("{:.0}", v),
        "text_size" => format!("{:.0} pt", v),
        "rotation" => format!("{:.0}°", v),
        // Volume uses dB scale: -60 floor (shown as "–∞ dB"), +15 ceiling
        "volume" => {
            if v <= -60.0 {
                "–∞ dB".to_string()
            } else {
                format!("{:.1} dB", v)
            }
        }
        // Speed uses multiplier notation (0.25×–4.0×)
        "speed" => format!("{:.2}×", v),
        // Chroma params are 0..1 shown as percent.
        "chroma_tolerance" | "chroma_softness" | "chroma_spill" => format!("{:.0}%", v * 100.0),
        _ => format!("{:.0}%", v), // scale, opacity
    }
}

/// update_text payload with the #330 nested `style` patch — the inspector no
/// longer sends the deprecated flat `fontName`/`fontSize`/`color`/`alignment`
/// compat keys.
fn update_text_style_args(clip_id: &str, style: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "clipIds": [clip_id], "style": style })
}

/// Crop aspect presets — mirrors Swift `CropAspectLock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropAspect {
    Free,
    Original,
    R16x9,
    R9x16,
    R1x1,
    R4x3,
    R3x4,
    R21x9,
}

impl CropAspect {
    pub const ALL: [CropAspect; 8] = [
        CropAspect::Free,
        CropAspect::Original,
        CropAspect::R16x9,
        CropAspect::R9x16,
        CropAspect::R1x1,
        CropAspect::R4x3,
        CropAspect::R3x4,
        CropAspect::R21x9,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CropAspect::Free => "Custom",
            CropAspect::Original => "Original",
            CropAspect::R16x9 => "16:9",
            CropAspect::R9x16 => "9:16",
            CropAspect::R1x1 => "1:1",
            CropAspect::R4x3 => "4:3",
            CropAspect::R3x4 => "3:4",
            CropAspect::R21x9 => "21:9",
        }
    }

    pub fn pixel_aspect(self) -> Option<f64> {
        match self {
            CropAspect::Free | CropAspect::Original => None,
            CropAspect::R16x9 => Some(16.0 / 9.0),
            CropAspect::R9x16 => Some(9.0 / 16.0),
            CropAspect::R1x1 => Some(1.0),
            CropAspect::R4x3 => Some(4.0 / 3.0),
            CropAspect::R3x4 => Some(3.0 / 4.0),
            CropAspect::R21x9 => Some(21.0 / 9.0),
        }
    }
}

/// File size like Swift's ByteCountFormatter `.file` style (1000-based).
fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1000.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} bytes")
    } else if b < KB * KB {
        format!("{:.0} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.2} GB", b / (KB * KB * KB))
    }
}

/// Human label for a clip/asset type (Swift `ClipType.trackLabel`).
fn clip_type_label(t: ClipType) -> &'static str {
    match t {
        ClipType::Video => "Video",
        ClipType::Audio => "Audio",
        ClipType::Image => "Image",
        ClipType::Text => "Text",
        ClipType::Lottie => "Lottie",
        ClipType::Shape => "Shape",
        ClipType::Sequence => "Sequence",
    }
}

/// Generation model display name from the catalog, falling back to the raw id.
fn model_display_name(model_id: &str) -> String {
    generation_core::model_catalog::model_by_id(model_id)
        .map(|m| m.display_name.to_string())
        .unwrap_or_else(|| model_id.to_string())
}

// ── Shared-state snapshots ────────────────────────────────────────────────────

/// Selected clips + project format cloned out of the shared executor.
struct SelectionSnapshot {
    clips: Vec<Clip>,
    fps: i64,
    canvas_w: i64,
    canvas_h: i64,
    /// Group of the first stamped clip in the selection, if it still resolves
    /// (Swift `selectedMulticamGroupId`).
    multicam_group: Option<core_model::MulticamSource>,
}

impl SelectionSnapshot {
    /// Selection shape for the tab-availability logic (Swift availableTabs).
    fn tab_selection(&self) -> TabSelection {
        let text = self
            .clips
            .iter()
            .filter(|c| c.media_type == ClipType::Text)
            .count();
        let audio = self
            .clips
            .iter()
            .filter(|c| c.media_type == ClipType::Audio)
            .count();
        TabSelection {
            text_clips: text,
            non_text_visual_clips: self.clips.len() - text - audio,
            audio_clips: audio,
            has_multicam_group: self.multicam_group.is_some(),
            // Swift gates on a resolvable asset + account state; the Rust AI
            // Edit tab binds per-selection itself, so any selection is eligible
            // (preserves the pre-#327 always-visible behavior).
            ai_eligible: !self.clips.is_empty(),
        }
    }

}

fn snapshot_selected_clips(selected_ids: &[String]) -> SelectionSnapshot {
    let hub = crate::editor_state_hub::EditorStateHub::global();
    let exec = hub.executor();
    let guard = exec.lock().unwrap();
    let t = guard.timeline();
    // Timeline order (Swift iterates tracks), not click order.
    let clips: Vec<Clip> = t
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .filter(|c| selected_ids.iter().any(|id| id == &c.id))
        .cloned()
        .collect();
    let multicam_group = clips
        .iter()
        .filter_map(|c| c.multicam_group_id.as_deref())
        .find_map(|gid| guard.multicam_groups().iter().find(|g| g.id == gid))
        .cloned();
    SelectionSnapshot {
        clips,
        fps: t.fps,
        canvas_w: t.width,
        canvas_h: t.height,
        multicam_group,
    }
}

fn snapshot_selected_asset(asset_id: &str) -> Option<(MediaManifestEntry, Option<PathBuf>)> {
    let hub = crate::editor_state_hub::EditorStateHub::global();
    let root = hub.project_root();
    let exec = hub.executor();
    let guard = exec.lock().unwrap();
    let entry = guard
        .media_manifest()
        .entries
        .iter()
        .find(|e| e.id == asset_id)
        .cloned()?;
    Some((entry, root))
}

// ── View ─────────────────────────────────────────────────────────────────────

pub struct InspectorView {
    pub state: InspectorState,
    /// Preferred clip tab (Swift `preferredTab`); the rendered tab falls back
    /// to the first available one.
    preferred_tab: ClipTab,
    /// Selection the preferred tab was last resolved for (Swift onChange).
    tab_resolved_for: Option<Vec<String>>,
    /// Session-scoped collapse state for panel groups (#327).
    groups: GroupStates,
    pub has_clip_selected: bool,
    /// True when a media asset in the library panel is selected (Swift: Source mode).
    pub has_media_asset_selected: bool,
    /// Selected timeline clip ids (wired by the app shell from the timeline view).
    pub selected_clip_ids: Vec<String>,
    /// Selected media-library asset id (wired by the app shell).
    pub selected_media_asset_id: Option<String>,
    /// Timeline playhead, for keyframe-resolved row values.
    pub playhead_frame: i64,
    /// Crop-on-canvas editing toggle (Swift: editor.cropEditingActive). The
    /// preview's crop overlay reads this via the app shell.
    pub crop_editing_active: bool,
    crop_aspect: CropAspect,
    crop_menu_open: bool,
    ai_edit_view: Entity<AiEditTabView>,
    keyframes_view: Entity<KeyframesView>,
    focus_handle: FocusHandle,
    /// Current numeric values for all scrub fields (derived from the selected
    /// clip each render unless a drag is live).
    pub scrub_values: HashMap<&'static str, f32>,
    /// Drag session in progress — set on mouse-down, read during on_drag_move.
    active_scrub: Option<ScrubSession>,
    // Text tab entities
    content_area: Entity<TextArea>,
    font_picker: Entity<FontPickerField>,
    text_color_field: Entity<ColorField>,
    bg_color_field: Entity<ColorField>,
    border_color_field: Entity<ColorField>,
    shadow_color_field: Entity<ColorField>,
    /// Clip id the text-tab entities are currently synced to.
    text_synced_clip: Option<String>,
    /// Prompt copy feedback flag (Swift PromptCopyButton).
    prompt_copied: bool,
}

impl InspectorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let content_area = cx.new(|cx| {
            TextArea::new(cx, "Text")
                .with_min_lines(4)
                .with_max_lines(8)
        });
        cx.subscribe(&content_area, |this: &mut Self, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                if let Some(id) = this.text_synced_clip.clone() {
                    let text = area.read(cx).text().to_string();
                    Self::run_tool(
                        "update_text",
                        serde_json::json!({ "clipIds": [id], "content": text }),
                    );
                    cx.notify();
                }
            }
        })
        .detach();

        let font_picker = cx.new(|cx| FontPickerField::new(cx, "Poppins"));
        cx.subscribe(&font_picker, |this: &mut Self, _, event, cx| {
            let FontPickerEvent::Picked(name) = event;
            if let Some(id) = this.text_synced_clip.clone() {
                Self::run_tool(
                    "update_text",
                    update_text_style_args(&id, serde_json::json!({ "fontName": name })),
                );
                cx.notify();
            }
        })
        .detach();

        let text_color_field = cx.new(|cx| ColorField::new(cx, core_model::TextRgba::default()));
        cx.subscribe(&text_color_field, |this: &mut Self, _, event, cx| {
            let ColorFieldEvent::Changed(rgba) = event;
            if let Some(id) = this.text_synced_clip.clone() {
                Self::run_tool(
                    "update_text",
                    update_text_style_args(&id, serde_json::json!({ "color": color_to_hex(rgba) })),
                );
                cx.notify();
            }
        })
        .detach();

        let bg_color_field = cx.new(|cx| ColorField::new(cx, core_model::TextRgba::default()));
        cx.subscribe(&bg_color_field, |this: &mut Self, _, event, cx| {
            let ColorFieldEvent::Changed(rgba) = event;
            this.commit_text_fill("background", Some(*rgba), None, cx);
        })
        .detach();

        let border_color_field = cx.new(|cx| ColorField::new(cx, core_model::TextRgba::default()));
        cx.subscribe(&border_color_field, |this: &mut Self, _, event, cx| {
            let ColorFieldEvent::Changed(rgba) = event;
            this.commit_text_fill("border", Some(*rgba), None, cx);
        })
        .detach();

        // Shadow has no write path in the tool surface yet — display-only.
        let shadow_color_field = cx.new(|cx| ColorField::new(cx, core_model::TextRgba::default()));
        shadow_color_field.update(cx, |f, cx| f.set_enabled(false, cx));

        Self {
            state: InspectorState::new(),
            preferred_tab: ClipTab::Video,
            tab_resolved_for: None,
            groups: GroupStates::default(),
            has_clip_selected: false,
            has_media_asset_selected: false,
            selected_clip_ids: Vec::new(),
            selected_media_asset_id: None,
            playhead_frame: 0,
            crop_editing_active: false,
            crop_aspect: CropAspect::Free,
            crop_menu_open: false,
            ai_edit_view: cx.new(|cx| AiEditTabView::new(cx)),
            keyframes_view: cx.new(|cx| KeyframesView::new(cx)),
            focus_handle: cx.focus_handle(),
            scrub_values: default_scrub_values(),
            active_scrub: None,
            content_area,
            font_picker,
            text_color_field,
            bg_color_field,
            border_color_field,
            shadow_color_field,
            text_synced_clip: None,
            prompt_copied: false,
        }
    }

    /// Run a tool on the shared executor; tool errors leave the UI unchanged.
    fn run_tool(tool: &str, args: serde_json::Value) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            if let Err(reason) = exec.execute(tool, &args) {
                eprintln!("{tool} failed: {reason}");
            }
        }
    }

    fn select_clip_tab(&mut self, tab: ClipTab, cx: &mut Context<Self>) {
        self.preferred_tab = tab;
        cx.notify();
    }

    fn toggle_group(&mut self, key: &'static str, default_expanded: bool, cx: &mut Context<Self>) {
        self.groups.toggle(key, default_expanded);
        cx.notify();
    }

    /// The first selected clip that can carry a chroma key (visual), with its
    /// current chroma controls (defaults when the clip has no key yet).
    fn first_chroma_clip(&self) -> Option<(String, crate::chroma_controls::ChromaControls)> {
        let snap = self.selection();
        snap.clips
            .iter()
            .find(|c| matches!(c.media_type, ClipType::Video | ClipType::Image))
            .map(|c| {
                (
                    c.id.clone(),
                    crate::chroma_controls::ChromaControls::from_chroma_key(c.chroma_key.as_ref()),
                )
            })
    }

    /// Update the selected visual clip's chroma key via `apply_effect`.
    fn apply_chroma(&self, update: impl FnOnce(&mut crate::chroma_controls::ChromaControls)) {
        if let Some((id, mut c)) = self.first_chroma_clip() {
            update(&mut c);
            Self::run_tool("apply_effect", c.apply_args(&[id]));
        }
    }

    fn toggle_chroma_enabled(&mut self, cx: &mut Context<Self>) {
        self.apply_chroma(|c| c.enabled = !c.enabled);
        cx.notify();
    }

    /// Set the key colour from a hue preset and enable the key.
    fn set_chroma_hue(&mut self, hue: f64, cx: &mut Context<Self>) {
        self.apply_chroma(|c| {
            *c = c.with_hue(hue);
            c.enabled = true;
        });
        cx.notify();
    }

    /// Arm the preview eyedropper for the selected visual clip.
    fn start_chroma_sampling(&mut self, cx: &mut Context<Self>) {
        if let Some((id, _)) = self.first_chroma_clip() {
            crate::chroma_sampling::set_sampling(Some(id));
        }
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

    fn selection(&self) -> SelectionSnapshot {
        snapshot_selected_clips(&self.selected_clip_ids)
    }

    /// Commit the live scrub value through the standard clip-property tools.
    fn commit_scrub(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.active_scrub.take() else {
            return;
        };
        if !session.dragged {
            cx.notify();
            return;
        }
        let value = self.scrub_value(session.field) as f64;
        let snap = self.selection();
        for clip in &snap.clips {
            if let Some((tool, args)) =
                scrub_commit_args(session.field, value, clip, snap.canvas_w, snap.canvas_h)
            {
                Self::run_tool(tool, args);
            }
        }
        cx.notify();
    }

    /// Write a full background/border fill (replace semantics — the tool
    /// overwrites the whole TextFill, so current values are carried through).
    fn commit_text_fill(
        &mut self,
        which: &str,
        new_color: Option<core_model::TextRgba>,
        new_enabled: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.text_synced_clip.clone() else {
            return;
        };
        let snap = self.selection();
        let Some(clip) = snap.clips.iter().find(|c| c.id == id) else {
            return;
        };
        let style = clip.text_style.clone().unwrap_or_default();
        let current: &TextFill = if which == "border" {
            &style.border
        } else {
            &style.background
        };
        // #330 nested style: "border" maps to style.outline; the partial
        // patch leaves every untouched field (padding, radius, …) as-is.
        let key = if which == "border" {
            "outline"
        } else {
            "background"
        };
        let fill = serde_json::json!({
            "enabled": new_enabled.unwrap_or(current.enabled),
            "color": color_to_hex(&new_color.unwrap_or(current.color)),
        });
        Self::run_tool(
            "update_text",
            serde_json::json!({ "clipIds": [id], "style": { key: fill } }),
        );
        cx.notify();
    }

    fn reset_levels(&mut self, cx: &mut Context<Self>) {
        for id in self.selected_clip_ids.clone() {
            Self::run_tool(
                "set_clip_properties",
                serde_json::json!({ "clipIds": [id], "properties": { "volume": 1.0 } }),
            );
        }
        cx.notify();
    }

    fn reset_transform(&mut self, cx: &mut Context<Self>) {
        for id in self.selected_clip_ids.clone() {
            Self::run_tool(
                "set_clip_properties",
                serde_json::json!({ "clipIds": [id], "properties": {
                    "opacity": 1.0,
                    "transform": {
                        "centerX": 0.5, "centerY": 0.5,
                        "width": 1.0, "height": 1.0,
                        "rotation": 0.0,
                        "flipHorizontal": false, "flipVertical": false,
                    }
                }}),
            );
            for property in ["position", "scale", "rotation"] {
                Self::run_tool(
                    "set_keyframes",
                    serde_json::json!({ "clipId": id, "property": property, "keyframes": [] }),
                );
            }
        }
        cx.notify();
    }

    fn reset_playback(&mut self, cx: &mut Context<Self>) {
        for id in self.selected_clip_ids.clone() {
            Self::run_tool(
                "set_clip_properties",
                serde_json::json!({ "clipIds": [id], "properties": { "speed": 1.0 } }),
            );
        }
        cx.notify();
    }

    /// Toggle a flip flag on every selected clip.
    fn toggle_flip(&mut self, horizontal: bool, current: bool, cx: &mut Context<Self>) {
        let key = if horizontal {
            "flipHorizontal"
        } else {
            "flipVertical"
        };
        let ids = self.selected_clip_ids.clone();
        if ids.is_empty() {
            return;
        }
        Self::run_tool(
            "set_clip_properties",
            serde_json::json!({ "clipIds": ids, "properties": { "transform": { key: !current } } }),
        );
        cx.notify();
    }

    /// Push the selected text clip's state into the text-tab entities once per
    /// clip switch (so typing isn't clobbered by render-time syncs).
    fn sync_text_entities(&mut self, clip: &Clip, cx: &mut Context<Self>) {
        if self.text_synced_clip.as_deref() == Some(clip.id.as_str()) {
            return;
        }
        self.text_synced_clip = Some(clip.id.clone());
        let style = clip.text_style.clone().unwrap_or_default();
        let content = clip.text_content.clone().unwrap_or_default();
        self.content_area
            .update(cx, |a, cx| a.set_text(content, cx));
        self.font_picker
            .update(cx, |f, cx| f.set_current(style.font_name.clone(), cx));
        self.text_color_field
            .update(cx, |f, cx| f.set_color(style.color, cx));
        self.bg_color_field.update(cx, |f, cx| {
            f.set_color(style.background.color, cx);
            f.set_enabled(style.background.enabled, cx);
        });
        self.border_color_field.update(cx, |f, cx| {
            f.set_color(style.border.color, cx);
            f.set_enabled(style.border.enabled, cx);
        });
        self.shadow_color_field.update(cx, |f, cx| {
            f.set_color(style.shadow.color, cx);
            f.set_enabled(false, cx);
        });
    }

    fn copy_prompt(&mut self, prompt: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(prompt));
        self.prompt_copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1400))
                .await;
            let _ = this.update(cx, |view, cx| {
                view.prompt_copied = false;
                cx.notify();
            });
        })
        .detach();
    }

    /// Creates a scrubable numeric property row — matches Swift ScrubbableNumberField.
    ///
    /// `keyframeable`: when true, appends ◆ ‹ › keyframe buttons (Swift: keyframe control strip).
    /// `enabled`: false renders the value muted and inert (no selection).
    #[allow(clippy::too_many_arguments)]
    fn scrub_row(
        &self,
        field: &'static str,
        label: &str,
        min: f32,
        max: f32,
        sensitivity: f32,
        keyframeable: bool,
        enabled: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let value = self.scrub_value(field);
        let display = fmt_scrub(field, value);
        // #327 InspectorRow layout: right-aligned label in the fixed column,
        // value right-aligned in the trailing area.
        div()
            .id(SharedString::from(format!("scrub-{field}")))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(Spacing::SM))
            .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
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
            .child(div().flex_1())
            .child(
                div()
                    .text_color(if enabled {
                        Accent::PRIMARY
                    } else {
                        Text::MUTED
                    })
                    .text_size(px(FontSize::XS))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .when(enabled, |el| el.cursor_pointer())
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
                        .ml(px(Spacing::XS))
                        .child(
                            div()
                                .id(SharedString::from(format!("kf-prev-{field}")))
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
                                .id(SharedString::from(format!("kf-add-{field}")))
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
                                .id(SharedString::from(format!("kf-next-{field}")))
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
            .when(enabled, |el| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(
                        move |this: &mut InspectorView, event: &MouseDownEvent, _window, _cx| {
                            this.active_scrub = Some(ScrubSession {
                                field,
                                start_x: event.position.x.as_f32(),
                                start_value: this.scrub_value(field),
                                sensitivity,
                                min,
                                max,
                                dragged: false,
                            });
                        },
                    ),
                )
                // Initiate gpui drag — required to activate on_drag_move globally
                .on_drag(ScrubData, move |_, _offset, _window, cx: &mut App| {
                    cx.new(|_| ScrubPreview)
                })
            })
    }
}

impl Focusable for InspectorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── Static row helpers ────────────────────────────────────────────────────────

/// Plain metadata row (Swift plainMetadataRow): label left, value right.
fn prop_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(EditorPanel::ROW_MIN_HEIGHT))
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

fn keyframes_btn(id: &str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .text_color(if active {
            Text::PRIMARY
        } else {
            Text::TERTIARY
        })
        .text_size(px(FontSize::XS))
        .cursor_pointer()
        .child("Keyframes")
}

/// Icon toggle button (Swift iconToggleButton) — accent color when on.
fn icon_toggle(id: String, glyph: &'static str, is_on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id))
        .w(px(22.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::XS))
        .bg(if is_on {
            BorderColors::SUBTLE
        } else {
            Background::SURFACE
        })
        .cursor_pointer()
        .text_size(px(FontSize::SM))
        .text_color(if is_on {
            Accent::PRIMARY
        } else {
            Text::SECONDARY
        })
        .child(glyph)
}

// ── Source mode (media asset metadata) ────────────────────────────────────────

impl InspectorView {
    /// No-selection mode (Swift projectMetadataContent, #327): Project and
    /// Settings panel groups. Settings rows stay read-only — the Swift preset
    /// menus write timeline settings, a binding the Rust inspector doesn't
    /// have yet.
    fn project_metadata_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        use timeline_core::TimelineMathExt;

        let hub = crate::editor_state_hub::EditorStateHub::global();
        let (width, height, fps, total_frames) = {
            let exec = hub.executor();
            let guard = exec.lock().unwrap();
            let t = guard.timeline();
            (t.width, t.height, t.fps, t.total_frames())
        };
        let (name, path) = match hub.project_root() {
            Some(p) => (
                p.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Untitled".into()),
                p.display().to_string(),
            ),
            None => ("Untitled".into(), "~/Movies/Untitled.palmier".into()),
        };
        let resolution = format!("{width} × {height}");
        let frame_rate = format!("{fps} fps");
        let aspect = timeline_core::format_aspect_ratio(width, height);
        let duration = timeline_core::format_duration(total_frames as f64 / fps.max(1) as f64);

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(
                EditorPanelGroup::new("group-meta-project", "Project")
                    .expanded(self.groups.expanded("meta-project", true))
                    .content_spacing(Spacing::SM)
                    .on_toggle(cx.listener(|this, _, _, cx| {
                        this.toggle_group("meta-project", true, cx)
                    }))
                    .child(prop_row("Name", &name))
                    .child(prop_row("Path", &path))
                    .child(prop_row("Duration", &duration)),
            )
            .child(
                EditorPanelGroup::new("group-meta-settings", "Settings")
                    .expanded(self.groups.expanded("meta-settings", true))
                    .content_spacing(Spacing::SM)
                    .on_toggle(cx.listener(|this, _, _, cx| {
                        this.toggle_group("meta-settings", true, cx)
                    }))
                    .child(prop_row("Resolution", &resolution))
                    .child(prop_row("Frame Rate", &frame_rate))
                    .child(prop_row("Aspect Ratio", &aspect)),
            )
            .into_any_element()
    }

    /// Source mode — displayed when a media asset (not a timeline clip) is
    /// selected. Matches Swift InspectorView.assetDetailsContent.
    fn source_asset_content(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some((entry, root)) = self
            .selected_media_asset_id
            .as_deref()
            .and_then(snapshot_selected_asset)
        else {
            return div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::MD))
                .px(px(Spacing::LG))
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::SM))
                        .child("Select an asset"),
                )
                .into_any_element();
        };

        let is_generated = entry.generation_input.is_some();
        let path = crate::video_export::source_path(
            &entry,
            root.as_deref().unwrap_or_else(|| std::path::Path::new("")),
        );
        let path_str = path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".to_string());
        let file_size = path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| format_file_size(m.len()));

        // Identity header: name + AI badge
        let mut header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::SM))
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::LG))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(entry.name.clone()),
            );
        if is_generated {
            header = header.child(
                div()
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XXS))
                    .rounded(px(Radius::SM))
                    .border_1()
                    .border_color(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::MUTED,
                    })
                    .text_color(Accent::PRIMARY)
                    .text_size(px(FontSize::XXS))
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("AI"),
            );
        }

        // File group (Swift fileSection → metadataSection "File").
        let mut file_section = EditorPanelGroup::new("group-asset-file", "File")
            .expanded(self.groups.expanded("asset-file", true))
            .content_spacing(Spacing::SM)
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("asset-file", true, cx)))
            .child(prop_row("Type", clip_type_label(entry.r#type)));
        if entry.r#type != ClipType::Audio {
            if let (Some(w), Some(h)) = (entry.source_width, entry.source_height) {
                file_section = file_section.child(prop_row("Dimensions", &format!("{w} × {h}")));
            }
        }
        if entry.duration > 0.0 && entry.r#type != ClipType::Image {
            file_section = file_section.child(prop_row(
                "Duration",
                &timeline_core::format_duration(entry.duration),
            ));
        }
        if let Some(size) = file_size {
            file_section = file_section.child(prop_row("Size", &size));
        }
        file_section = file_section.child(prop_row("Path", &path_str));

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .pt(px(Spacing::MD))
            .gap(px(Spacing::SM))
            .child(header.px(px(Spacing::SM_MD)))
            .child(file_section);

        // Generated + prompt sections (Swift metadataSection "Generated").
        if let Some(gen) = &entry.generation_input {
            let mut gen_section = EditorPanelGroup::new("group-asset-generated", "Generated")
                .expanded(self.groups.expanded("asset-generated", true))
                .content_spacing(Spacing::SM)
                .on_toggle(cx.listener(|this, _, _, cx| {
                    this.toggle_group("asset-generated", true, cx)
                }))
                .child(prop_row("Model", &model_display_name(&gen.model)));
            if !gen.aspect_ratio.is_empty() {
                gen_section = gen_section.child(prop_row("Aspect Ratio", &gen.aspect_ratio));
            }
            if let Some(res) = &gen.resolution {
                gen_section = gen_section.child(prop_row("Resolution", res));
            }
            if gen.duration > 0 {
                gen_section =
                    gen_section.child(prop_row("Duration", &format!("{}s", gen.duration)));
            }
            body = body.child(gen_section);

            if !gen.prompt.is_empty() {
                let prompt = gen.prompt.clone();
                let copied = self.prompt_copied;
                // Swift promptSection: title + copy button, then the text.
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::SM_MD))
                        .px(px(Spacing::SM_MD))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::SM_MD))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child("Prompt"),
                                )
                                .child(
                                    div()
                                        .id("prompt-copy-btn")
                                        .cursor_pointer()
                                        .text_color(if copied {
                                            Text::PRIMARY
                                        } else {
                                            Text::MUTED
                                        })
                                        .text_size(px(FontSize::XS))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.copy_prompt(prompt.clone(), cx);
                                        }))
                                        .child(if copied { "✓" } else { "⧉" }),
                                ),
                        )
                        .child(
                            div()
                                .text_color(Text::SECONDARY)
                                .text_size(px(FontSize::SM))
                                .child(gen.prompt.clone()),
                        ),
                );
            }
        }

        body.into_any_element()
    }

    // ── Crop / Flip rows (Video tab) ─────────────────────────────────────────

    fn crop_row(&self, first: Option<&Clip>, cx: &mut Context<Self>) -> AnyElement {
        let enabled = first.is_some();
        let editing = self.crop_editing_active && enabled;
        let active_aspect = self.crop_aspect;
        let menu_open = self.crop_menu_open;

        let mut aspect_menu = div()
            .id("crop-aspect-dropdown")
            .absolute()
            .top(px(22.0))
            .right_0()
            .w(px(110.0))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .rounded(px(Radius::SM))
            .flex()
            .flex_col()
            .py(px(Spacing::XS));
        for (i, preset) in CropAspect::ALL.iter().enumerate() {
            let preset = *preset;
            let is_active = preset == active_aspect;
            aspect_menu = aspect_menu.child(
                div()
                    .id(SharedString::from(format!("crop-aspect-{i}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::XS))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.crop_aspect = preset;
                        this.crop_menu_open = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(if is_active {
                                Text::PRIMARY
                            } else {
                                Hsla {
                                    h: 0.0,
                                    s: 0.0,
                                    l: 1.0,
                                    a: 0.0,
                                }
                            })
                            .child("✓"),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(Text::SECONDARY)
                            .child(preset.label()),
                    ),
            );
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(Spacing::SM))
            .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
            .opacity(if enabled {
                Opacity::OPAQUE
            } else {
                Opacity::MEDIUM
            })
            .child(
                div()
                    .w(px(EditorPanel::LABEL_COLUMN_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child("Crop"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        icon_toggle("crop-toggle".into(), "▣", editing).when(enabled, |el| {
                            el.on_click(cx.listener(|this, _, _, cx| {
                                this.crop_editing_active = !this.crop_editing_active;
                                cx.notify();
                            }))
                        }),
                    )
                    .child(
                        div()
                            .relative()
                            .child(
                                div()
                                    .id("crop-aspect-btn")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .px(px(Spacing::SM))
                                    .py(px(Spacing::XXS))
                                    .cursor_pointer()
                                    .when(enabled, |el| {
                                        el.on_click(cx.listener(|this, _, _, cx| {
                                            this.crop_menu_open = !this.crop_menu_open;
                                            cx.notify();
                                        }))
                                    })
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::XS))
                                            .child(active_aspect.label()),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::TERTIARY)
                                            .text_size(px(FontSize::XXS))
                                            .child("▾"),
                                    ),
                            )
                            .when(menu_open && enabled, |el| el.child(aspect_menu)),
                    ),
            )
            .into_any_element()
    }

    fn flip_row(&self, first: Option<&Clip>, cx: &mut Context<Self>) -> AnyElement {
        let enabled = first.is_some();
        let flip_h = first.map(|c| c.transform.flip_horizontal).unwrap_or(false);
        let flip_v = first.map(|c| c.transform.flip_vertical).unwrap_or(false);
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(Spacing::SM))
            .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
            .opacity(if enabled {
                Opacity::OPAQUE
            } else {
                Opacity::MEDIUM
            })
            .child(
                div()
                    .w(px(EditorPanel::LABEL_COLUMN_WIDTH))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child("Flip"),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        icon_toggle("flip-h".into(), "↔", flip_h).when(enabled, |el| {
                            el.on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_flip(true, flip_h, cx);
                            }))
                        }),
                    )
                    .child(
                        icon_toggle("flip-v".into(), "↕", flip_v).when(enabled, |el| {
                            el.on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_flip(false, flip_v, cx);
                            }))
                        }),
                    ),
            )
            .into_any_element()
    }

    // ── Text tab ─────────────────────────────────────────────────────────────

    fn text_tab_content(&mut self, first: Option<&Clip>, cx: &mut Context<Self>) -> AnyElement {
        let text_clip = first.filter(|c| c.media_type == ClipType::Text);
        let Some(clip) = text_clip else {
            self.text_synced_clip = None;
            return div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::MD))
                .px(px(Spacing::LG))
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::SM))
                        .child("Select a text clip"),
                )
                .into_any_element();
        };
        let clip = clip.clone();
        self.sync_text_entities(&clip, cx);
        let style = clip.text_style.clone().unwrap_or_default();

        // Keep style-driven bits live across edits on the same clip (toggles and
        // external tool edits change them without a clip switch). set_color /
        // set_current no-op when unchanged, so typing is never disturbed.
        self.font_picker
            .update(cx, |f, cx| f.set_current(style.font_name.clone(), cx));
        self.text_color_field
            .update(cx, |f, cx| f.set_color(style.color, cx));
        self.bg_color_field.update(cx, |f, cx| {
            f.set_color(style.background.color, cx);
            f.set_enabled(style.background.enabled, cx);
        });
        self.border_color_field.update(cx, |f, cx| {
            f.set_color(style.border.color, cx);
            f.set_enabled(style.border.enabled, cx);
        });
        self.shadow_color_field
            .update(cx, |f, cx| f.set_color(style.shadow.color, cx));

        // Alignment segmented control (Swift: 3-way L/C/R picker)
        let alignments = [
            (TextAlignment::Left, "◀▌"),
            (TextAlignment::Center, "▌◀▶▌"),
            (TextAlignment::Right, "▌▶"),
        ];
        let mut align_buttons = div().flex().flex_row().gap(px(Spacing::XXS));
        for (i, (alignment, glyph)) in alignments.into_iter().enumerate() {
            let active = style.alignment == alignment;
            let name = match alignment {
                TextAlignment::Left => "left",
                TextAlignment::Center => "center",
                TextAlignment::Right => "right",
            };
            let clip_id = clip.id.clone();
            align_buttons = align_buttons.child(
                div()
                    .id(SharedString::from(format!("align-btn-{i}")))
                    .px(px(Spacing::XS))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Radius::XS))
                    .bg(if active {
                        BorderColors::SUBTLE
                    } else {
                        Background::SURFACE
                    })
                    .cursor_pointer()
                    .text_size(px(FontSize::XXS))
                    .text_color(if active {
                        Text::PRIMARY
                    } else {
                        Text::TERTIARY
                    })
                    .on_click(cx.listener(move |_, _, _, cx| {
                        Self::run_tool(
                            "update_text",
                            update_text_style_args(&clip_id, serde_json::json!({ "alignment": name })),
                        );
                        cx.notify();
                    }))
                    .child(glyph),
            );
        }

        // Decoration-group enable toggle (Swift decorationGroup headerAccessory).
        // `which == None` renders a display-only toggle (no write path yet).
        let fill_toggle = |label: &'static str,
                           which: Option<&'static str>,
                           enabled: bool,
                           cx: &mut Context<Self>| {
            let toggle = icon_toggle(
                format!("fill-toggle-{}", label.to_lowercase()),
                if enabled { "◉" } else { "○" },
                enabled,
            );
            if let Some(which) = which {
                toggle
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.commit_text_fill(which, None, Some(!enabled), cx);
                    }))
                    .into_any_element()
            } else {
                toggle.opacity(Opacity::MEDIUM).into_any_element()
            }
        };

        let size_row = self.scrub_row("text_size", "Size", 12.0, 300.0, 0.5, false, true, cx);
        let opacity_row = self.scrub_row("opacity", "Opacity", 0.0, 100.0, 0.5, false, true, cx);
        let pos_x_row = self.scrub_row(
            "position_x",
            "Position X",
            -9999.0,
            9999.0,
            2.0,
            false,
            true,
            cx,
        );
        let pos_y_row = self.scrub_row(
            "position_y",
            "Position Y",
            -9999.0,
            9999.0,
            2.0,
            false,
            true,
            cx,
        );

        // "Text" group: content editor in value-field chrome.
        let text_group = EditorPanelGroup::new("group-text-content", "Text")
            .expanded(self.groups.expanded("text-text", true))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("text-text", true, cx)))
            .child(
                editor_value_field()
                    .w_full()
                    .min_h(px(EditorPanel::TEXT_EDITOR_MIN_HEIGHT))
                    .p(px(Spacing::SM_MD))
                    .child(self.content_area.clone()),
            );

        // "Style" group: Font / Size / Alignment / Position / Color / Opacity.
        let style_group = EditorPanelGroup::new("group-text-style", "Style")
            .expanded(self.groups.expanded("text-style", true))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("text-style", true, cx)))
            .child(panel_row("Font", self.font_picker.clone().into_any_element()))
            .child(size_row)
            .child(panel_row("Alignment", align_buttons.into_any_element()))
            .child(pos_x_row)
            .child(pos_y_row)
            .child(panel_row(
                "Color",
                self.text_color_field.clone().into_any_element(),
            ))
            .child(opacity_row);

        // Decoration groups (Swift Outline / Shadow / Background).
        let outline_group = EditorPanelGroup::new("group-text-outline", "Outline")
            .expanded(self.groups.expanded("text-outline", true))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("text-outline", true, cx)))
            .header_accessory(fill_toggle("Outline", Some("border"), style.border.enabled, cx))
            .child(
                div()
                    .w_full()
                    .opacity(if style.border.enabled {
                        Opacity::OPAQUE
                    } else {
                        Opacity::MEDIUM
                    })
                    .child(panel_row(
                        "Color",
                        self.border_color_field.clone().into_any_element(),
                    )),
            );
        // Shadow: bound display; editing needs a tool-surface extension.
        let shadow_group = EditorPanelGroup::new("group-text-shadow", "Shadow")
            .expanded(self.groups.expanded("text-shadow", true))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("text-shadow", true, cx)))
            .header_accessory(fill_toggle("Shadow", None, style.shadow.enabled, cx))
            .child(div().w_full().opacity(Opacity::MEDIUM).child(panel_row(
                "Color",
                self.shadow_color_field.clone().into_any_element(),
            )));
        let background_group = EditorPanelGroup::new("group-text-background", "Background")
            .expanded(self.groups.expanded("text-background", true))
            .on_toggle(cx.listener(|this, _, _, cx| {
                this.toggle_group("text-background", true, cx)
            }))
            .header_accessory(fill_toggle(
                "Background",
                Some("background"),
                style.background.enabled,
                cx,
            ))
            .child(
                div()
                    .w_full()
                    .opacity(if style.background.enabled {
                        Opacity::OPAQUE
                    } else {
                        Opacity::MEDIUM
                    })
                    .child(panel_row(
                        "Color",
                        self.bg_color_field.clone().into_any_element(),
                    )),
            );

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(text_group)
            .child(style_group)
            .child(outline_group)
            .child(shadow_group)
            .child(background_group)
            .into_any_element()
    }
}

// ── Clip tab content (#327 panel groups) ─────────────────────────────────────

impl InspectorView {
    /// Keyframes toggle header accessory (Swift keyframesToggleButton).
    fn keyframes_accessory(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        keyframes_btn(id, self.state.keyframes_visible)
            .on_click(cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_keyframes(cx);
            }))
            .into_any_element()
    }

    /// Keyframe panel strip below the groups (Rust layout; Swift splits the
    /// group content instead — a follow-up).
    fn keyframes_strip(&self, kf_entity: &Entity<KeyframesView>) -> Option<AnyElement> {
        if !self.state.keyframes_visible {
            return None;
        }
        Some(
            div()
                .w_full()
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .child(kf_entity.clone())
                .into_any_element(),
        )
    }

    /// Video tab (Swift videoTabContent): Transform + Playback groups.
    fn video_tab(
        &mut self,
        snap: &SelectionSnapshot,
        rows_enabled: bool,
        kf_entity: &Entity<KeyframesView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let transform_expanded = self.state.transform_expanded;
        let pos_x_row = self.scrub_row(
            "position_x",
            "Position X",
            -9999.0,
            9999.0,
            2.0,
            true,
            rows_enabled,
            cx,
        );
        let pos_y_row = self.scrub_row(
            "position_y",
            "Position Y",
            -9999.0,
            9999.0,
            2.0,
            true,
            rows_enabled,
            cx,
        );
        let scale_row = self.scrub_row("scale", "Scale", 1.0, 1000.0, 1.0, true, rows_enabled, cx);
        let rotation_row = self.scrub_row(
            "rotation",
            "Rotation",
            -360.0,
            360.0,
            1.0,
            true,
            rows_enabled,
            cx,
        );
        let opacity_row = self.scrub_row(
            "opacity",
            "Opacity",
            0.0,
            100.0,
            0.5,
            true,
            rows_enabled,
            cx,
        );
        let first_clip = snap.clips.first();
        let crop_row = self.crop_row(first_clip, cx);
        let flip_row = self.flip_row(first_clip, cx);

        let mut transform_group = EditorPanelGroup::new("group-transform", "Transform")
            .expanded(transform_expanded)
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_transform(cx)));
        if transform_expanded {
            transform_group =
                transform_group.header_accessory(self.keyframes_accessory("kf-toggle-video", cx));
        }
        if rows_enabled {
            transform_group = transform_group.reset(
                editor_reset_button(
                    "reset-transform",
                    cx.listener(|this, _, _, cx| this.reset_transform(cx)),
                )
                .into_any_element(),
            );
        }
        transform_group = transform_group
            .child(pos_x_row)
            .child(pos_y_row)
            .child(scale_row)
            .child(rotation_row)
            .child(opacity_row)
            .child(crop_row)
            .child(flip_row);

        let playback_group = self.playback_group("video-playback", rows_enabled, cx);

        let mut body = div()
            .flex()
            .flex_col()
            .w_full()
            .child(transform_group)
            .child(playback_group);
        if let Some(strip) = self.keyframes_strip(kf_entity) {
            body = body.child(strip);
        }
        body.into_any_element()
    }

    /// Playback group (Swift speedSection) shared by Video and Audio tabs.
    fn playback_group(
        &mut self,
        key: &'static str,
        rows_enabled: bool,
        cx: &mut Context<Self>,
    ) -> EditorPanelGroup {
        let speed_row = self.scrub_row("speed", "Speed", 0.25, 4.0, 0.01, false, rows_enabled, cx);
        let mut group = EditorPanelGroup::new(format!("group-{key}"), "Playback")
            .expanded(self.groups.expanded(key, true))
            .on_toggle(cx.listener(move |this, _, _, cx| this.toggle_group(key, true, cx)));
        if rows_enabled {
            group = group.reset(
                editor_reset_button(
                    format!("reset-{key}"),
                    cx.listener(|this, _, _, cx| this.reset_playback(cx)),
                )
                .into_any_element(),
            );
        }
        group.child(speed_row)
    }

    /// Audio tab (Swift audioTabContent): Levels group + Playback when the
    /// selection has no visual clips. (Swift also has an Enhance/denoise
    /// group — no Rust denoise UI yet.)
    fn audio_tab(
        &mut self,
        snap: &SelectionSnapshot,
        rows_enabled: bool,
        kf_entity: &Entity<KeyframesView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let levels_expanded = self.state.volume_expanded;
        let vol_row = self.scrub_row("volume", "Volume", -60.0, 15.0, 0.5, false, rows_enabled, cx);
        // Fade rows are display-bound only: the tool surface has no fade write yet.
        let fade_in_row = self.scrub_row("fade_in", "Fade In", 0.0, 10.0, 0.05, false, false, cx);
        let fade_out_row =
            self.scrub_row("fade_out", "Fade Out", 0.0, 10.0, 0.05, false, false, cx);

        let mut levels_group = EditorPanelGroup::new("group-levels", "Levels")
            .expanded(levels_expanded)
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_volume(cx)));
        if levels_expanded {
            levels_group =
                levels_group.header_accessory(self.keyframes_accessory("kf-toggle-audio", cx));
        }
        if rows_enabled {
            levels_group = levels_group.reset(
                editor_reset_button(
                    "reset-levels",
                    cx.listener(|this, _, _, cx| this.reset_levels(cx)),
                )
                .into_any_element(),
            );
        }
        levels_group = levels_group
            .child(vol_row)
            .child(fade_in_row)
            .child(fade_out_row);

        let mut body = div().flex().flex_col().w_full().child(levels_group);
        if snap.tab_selection().non_text_visual_clips == 0 {
            body = body.child(self.playback_group("audio-playback", rows_enabled, cx));
        }
        if let Some(strip) = self.keyframes_strip(kf_entity) {
            body = body.child(strip);
        }
        body.into_any_element()
    }

    /// Adjust tab (Swift effectsTabContent). Only the Effects › Chroma Key
    /// subgroup has Rust controls; the other Swift sections (Basic Correction,
    /// Curves, Color Wheels, Hue Curves, LUTs) have no Rust effect UI yet, so
    /// no empty shells are rendered.
    fn adjust_tab(&mut self, snap: &SelectionSnapshot, cx: &mut Context<Self>) -> AnyElement {
        let effects_default = ADJUST_TAB_GROUPS[0].1;
        let chroma_default = ADJUST_CHROMA_SUBGROUP.1;
        let effects_expanded = self.groups.expanded("adjust-effects", effects_default);
        let chroma_expanded = self.groups.expanded("adjust-chroma", chroma_default);

        // Same clip the apply path targets (first_chroma_clip: Video | Image).
        let chroma_clip = snap
            .clips
            .iter()
            .find(|c| matches!(c.media_type, ClipType::Video | ClipType::Image));
        let is_visual = chroma_clip.is_some();
        let chroma_ctrls = crate::chroma_controls::ChromaControls::from_chroma_key(
            chroma_clip.and_then(|c| c.chroma_key.as_ref()),
        );
        let chroma_enabled = chroma_ctrls.enabled;
        let chroma_hue = chroma_ctrls.key_hue() as f32;
        let sampling_active = crate::chroma_sampling::sampling_clip().is_some();
        let chroma_rows_on = is_visual && chroma_enabled;

        let mut effects_group = EditorPanelGroup::new("group-adjust-effects", "Effects")
            .expanded(effects_expanded)
            .content_spacing(Spacing::XS)
            .on_toggle(cx.listener(move |this, _, _, cx| {
                this.toggle_group("adjust-effects", effects_default, cx)
            }));

        // Chroma Key subgroup header (Swift adjustSubgroup): chevron + title,
        // eyedropper accessory, plus the Rust enable chip.
        let subgroup_header = div()
            .id("chroma-subgroup-header")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(Spacing::XS))
            .cursor_pointer()
            .child(
                div()
                    .w(px(IconSize::XXS))
                    .flex()
                    .justify_center()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XXS))
                    .child(if chroma_expanded { "▾" } else { "▸" }),
            )
            .child(
                div()
                    .flex_1()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Chroma Key"),
            )
            .child(
                div()
                    .id("chroma-eyedropper")
                    .px(px(Spacing::SM))
                    .h(px(IconSize::SM))
                    .flex()
                    .items_center()
                    .rounded(px(Radius::XS))
                    .cursor_pointer()
                    .text_size(px(FontSize::XS))
                    .bg(if sampling_active {
                        Accent::PRIMARY
                    } else {
                        Background::RAISED
                    })
                    .text_color(if sampling_active {
                        Background::BASE
                    } else {
                        Text::SECONDARY
                    })
                    .child(if sampling_active {
                        "Click preview…"
                    } else {
                        "⦿ Pick"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.start_chroma_sampling(cx);
                    })),
            )
            .child(
                div()
                    .id("chroma-enable")
                    .cursor_pointer()
                    .text_size(px(FontSize::XS))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(if chroma_enabled {
                        Accent::PRIMARY
                    } else {
                        Text::MUTED
                    })
                    .child(if chroma_enabled { "On" } else { "Off" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_chroma_enabled(cx);
                    })),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_group("adjust-chroma", chroma_default, cx)
            }));
        effects_group = effects_group.child(subgroup_header);

        if chroma_expanded {
            let tol_row = self.scrub_row(
                "chroma_tolerance",
                "Tolerance",
                0.0,
                1.0,
                0.004,
                false,
                chroma_rows_on,
                cx,
            );
            let soft_row = self.scrub_row(
                "chroma_softness",
                "Softness",
                0.0,
                1.0,
                0.004,
                false,
                chroma_rows_on,
                cx,
            );
            let spill_row = self.scrub_row(
                "chroma_spill",
                "Spill",
                0.0,
                1.0,
                0.004,
                false,
                chroma_rows_on,
                cx,
            );
            let key_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::SM))
                .w_full()
                .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
                .child(
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded(px(Radius::XS))
                        .border_1()
                        .border_color(BorderColors::PRIMARY)
                        .bg(gpui::hsla(chroma_hue, 0.9, 0.5, 1.0)),
                )
                .child(
                    div()
                        .id("chroma-green")
                        .px(px(Spacing::SM))
                        .h(px(IconSize::SM))
                        .flex()
                        .items_center()
                        .rounded(px(Radius::XS))
                        .cursor_pointer()
                        .bg(Background::RAISED)
                        .text_size(px(FontSize::XS))
                        .text_color(Text::SECONDARY)
                        .child("Green")
                        .on_click(cx.listener(|this, _, _, cx| this.set_chroma_hue(1.0 / 3.0, cx))),
                )
                .child(
                    div()
                        .id("chroma-blue")
                        .px(px(Spacing::SM))
                        .h(px(IconSize::SM))
                        .flex()
                        .items_center()
                        .rounded(px(Radius::XS))
                        .cursor_pointer()
                        .bg(Background::RAISED)
                        .text_size(px(FontSize::XS))
                        .text_color(Text::SECONDARY)
                        .child("Blue")
                        .on_click(cx.listener(|this, _, _, cx| this.set_chroma_hue(2.0 / 3.0, cx))),
                );
            effects_group = effects_group
                .child(key_row)
                .child(tol_row)
                .child(soft_row)
                .child(spill_row);
        }

        div()
            .flex()
            .flex_col()
            .w_full()
            .child(effects_group)
            .into_any_element()
    }

    /// Multicam tab (Swift MulticamTab): read-only member roster for the
    /// selected clip's group. Functional multicam UI stays deferred.
    fn multicam_tab(&mut self, snap: &SelectionSnapshot, cx: &mut Context<Self>) -> AnyElement {
        let Some(group) = snap.multicam_group.clone() else {
            return div().into_any_element();
        };
        let title = if group.name.is_empty() {
            "Multicam".to_string()
        } else {
            group.name.clone()
        };
        let mut panel = EditorPanelGroup::new("group-multicam", title)
            .expanded(self.groups.expanded("multicam", true))
            .on_toggle(cx.listener(|this, _, _, cx| this.toggle_group("multicam", true, cx)));
        for member in &group.members {
            panel = panel.child(multicam_member_row(member, &group));
        }
        div()
            .flex()
            .flex_col()
            .w_full()
            .child(panel)
            .into_any_element()
    }
}

/// One multicam member row (Swift MulticamTab.memberRow).
fn multicam_member_row(
    member: &core_model::MulticamMember,
    group: &core_model::MulticamSource,
) -> impl IntoElement {
    let (kind_label, kind_color) = match member.kind {
        core_model::MulticamMemberKind::Angle => ("Angle", TrackColor::VIDEO),
        core_model::MulticamMemberKind::Mic => ("Mic", TrackColor::AUDIO),
        core_model::MulticamMemberKind::Both => ("Both", TrackColor::MULTICAM),
    };
    let is_master = member.id == group.master_member_id;
    let usable = member.usable();
    let sync_text = format!(
        "{:+.2}s · {:.0}%",
        member.sync.offset_seconds,
        member.sync.confidence * 100.0
    );
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .gap(px(Spacing::SM))
        .min_h(px(EditorPanel::ROW_MIN_HEIGHT))
        .child(
            div()
                .px(px(Spacing::XS))
                .py(px(Spacing::XXS))
                .rounded(px(Radius::XS))
                .bg(kind_color)
                .text_size(px(FontSize::XXS))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: Opacity::PROMINENT,
                })
                .child(kind_label),
        )
        .child(
            div()
                .text_size(px(FontSize::SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(Text::PRIMARY)
                .child(member.angle_label.clone()),
        )
        .when(is_master, |el| {
            el.child(
                div()
                    .text_size(px(FontSize::XXS))
                    .text_color(Accent::TIMECODE)
                    .child("★"),
            )
        })
        .child(div().flex_1())
        .child(if usable {
            div()
                .text_size(px(FontSize::XXS))
                .text_color(Text::TERTIARY)
                .child(sync_text)
                .into_any_element()
        } else {
            div()
                .text_size(px(FontSize::XXS))
                .text_color(crate::theme::Status::ERROR)
                .child("⚠ Not synced")
                .into_any_element()
        })
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_clip = self.has_clip_selected || !self.selected_clip_ids.is_empty();
        let has_asset =
            !has_clip && (self.has_media_asset_selected || self.selected_media_asset_id.is_some());
        let title = if has_asset {
            "Source"
        } else if has_clip {
            "Inspector"
        } else {
            "Timeline"
        };
        let icon = if has_asset {
            "◈"
        } else if has_clip {
            "⊙"
        } else {
            "i"
        };

        // Selection snapshot: derive row values from the selected clip unless a
        // scrub drag is live (the drag owns the value until commit).
        let snap = if has_clip {
            self.selection()
        } else {
            SelectionSnapshot {
                clips: Vec::new(),
                fps: 30,
                canvas_w: 1920,
                canvas_h: 1080,
                multicam_group: None,
            }
        };
        let first_clip = snap.clips.first().cloned();
        let scrub_live = self
            .active_scrub
            .as_ref()
            .map(|s| s.dragged)
            .unwrap_or(false);
        if !scrub_live {
            self.scrub_values = match &first_clip {
                Some(clip) => derive_scrub_values(
                    clip,
                    self.playhead_frame,
                    snap.fps,
                    snap.canvas_w,
                    snap.canvas_h,
                ),
                None => default_scrub_values(),
            };
        }
        if first_clip.is_none() {
            self.text_synced_clip = None;
        }
        let rows_enabled = first_clip.is_some();

        // Tab availability + preferred-tab resolution (Swift availableTabs +
        // resolvePreferredTab, the latter on selection change only).
        let tab_sel = snap.tab_selection();
        if self.tab_resolved_for.as_deref() != Some(self.selected_clip_ids.as_slice()) {
            self.preferred_tab = resolve_preferred_tab(self.preferred_tab, tab_sel);
            self.tab_resolved_for = Some(self.selected_clip_ids.clone());
            // Swift drops crop editing when the selection changes.
            self.crop_editing_active = false;
        }
        let tabs = available_tabs(tab_sel);
        let active_tab = resolve_active_tab(self.preferred_tab, &tabs);

        // Forward the media-library selection so AI Edit actions bind to it.
        // Guarded like the text-tab entity syncs — notify only on change.
        let asset_sel = self.selected_media_asset_id.clone();
        self.ai_edit_view.update(cx, |ai, cx| {
            if ai.selected_media_asset_id != asset_sel {
                ai.selected_media_asset_id = asset_sel;
                ai.state.status = None;
                ai.state.show_upscale_picker = false;
                cx.notify();
            }
        });
        let ai_edit_entity = self.ai_edit_view.clone();
        let kf_entity = self.keyframes_view.clone();

        // WeakEntity captured for on_drag_move (global while a drag is active).
        let weak_drag = cx.entity().downgrade();

        // Per-tab content (only the active tab is built).
        let tab_content: Option<AnyElement> = if has_clip {
            match active_tab {
                Some(ClipTab::Video) => Some(self.video_tab(&snap, rows_enabled, &kf_entity, cx)),
                Some(ClipTab::Adjust) => Some(self.adjust_tab(&snap, cx)),
                Some(ClipTab::Audio) => Some(self.audio_tab(&snap, rows_enabled, &kf_entity, cx)),
                Some(ClipTab::Text) => Some(self.text_tab_content(first_clip.as_ref(), cx)),
                Some(ClipTab::Multicam) => Some(self.multicam_tab(&snap, cx)),
                Some(ClipTab::AiEdit) => Some(ai_edit_entity.clone().into_any_element()),
                None => None,
            }
        } else {
            None
        };

        // Tab bar (Swift TitleTabBar; hidden when only one tab is available).
        let tab_bar: Option<AnyElement> = if has_clip && tabs.len() > 1 {
            let mut bar = title_tab_bar().id("inspector-tabs");
            for tab in &tabs {
                let tab = *tab;
                bar = bar.child(
                    title_tab(
                        SharedString::from(format!("clip-tab-{}", tab.label())),
                        tab.label(),
                        Some(tab) == active_tab,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.select_clip_tab(tab, cx))),
                );
            }
            Some(bar.into_any_element())
        } else {
            None
        };

        let metadata = if !has_clip && !has_asset {
            Some(self.project_metadata_content(cx))
        } else {
            None
        };
        let source_content = if has_asset {
            Some(self.source_asset_content(cx))
        } else {
            None
        };

        div()
            .id("inspector-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // on_drag_move fires globally while a ScrubData drag is active.
            // Computes delta from active_scrub.start_x and updates the field value.
            .on_drag_move::<ScrubData>(
                move |event: &DragMoveEvent<ScrubData>, _window, cx: &mut App| {
                    let _ = weak_drag.update(cx, |this: &mut InspectorView, inner_cx| {
                        if let Some(ref mut session) = this.active_scrub {
                            let delta = event.event.position.x.as_f32() - session.start_x;
                            let new_val = (session.start_value + delta * session.sensitivity)
                                .clamp(session.min, session.max);
                            session.dragged = true;
                            let field = session.field;
                            this.scrub_values.insert(field, new_val);
                            inner_cx.notify();
                        }
                    });
                },
            )
            // Releasing the drag over the panel commits through the shared tools.
            .on_drop::<ScrubData>(cx.listener(|this, _, _, cx| this.commit_scrub(cx)))
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
                    .when_some(metadata, |el, content| el.child(content))
                    .when_some(source_content, |el, content| el.child(content))
                    .when_some(tab_bar, |el, bar| el.child(bar))
                    .when_some(tab_content, |el, content| el.child(content)),
            )
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_clip(extra: serde_json::Value) -> Clip {
        let mut base = serde_json::json!({
            "id": "c1",
            "mediaRef": "m1",
            "mediaType": "video",
            "sourceClipType": "video",
            "startFrame": 0,
            "durationFrames": 100
        });
        if let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_map {
                base_map.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn derive_defaults_from_plain_clip() {
        let clip = test_clip(serde_json::json!({}));
        let v = derive_scrub_values(&clip, 0, 30, 1920, 1080);
        assert!((v["volume"] - 0.0).abs() < 1e-4, "1.0 linear = 0 dB");
        assert!((v["opacity"] - 100.0).abs() < 1e-4);
        assert!((v["scale"] - 100.0).abs() < 1e-4);
        assert!((v["rotation"]).abs() < 1e-4);
        assert!((v["speed"] - 1.0).abs() < 1e-4);
        assert!((v["position_x"]).abs() < 1e-3, "default top-left x = 0 px");
        assert!((v["position_y"]).abs() < 1e-3);
        assert!((v["fade_in"]).abs() < 1e-6);
        assert!(
            (v["text_size"] - 96.0).abs() < 1e-4,
            "TextStyle default 96pt"
        );
    }

    #[test]
    fn derive_reads_static_clip_values() {
        let clip = test_clip(serde_json::json!({
            "volume": 0.5,
            "opacity": 0.7,
            "speed": 2.0,
            "fadeInFrames": 30,
            "fadeOutFrames": 15,
            "transform": {"centerX": 0.5, "centerY": 0.5, "width": 0.5, "height": 0.5, "rotation": 45.0}
        }));
        let v = derive_scrub_values(&clip, 0, 30, 1920, 1080);
        assert!(
            (v["volume"] - timeline_core::db_from_linear(0.5) as f32).abs() < 1e-4,
            "0.5 linear ≈ -6.02 dB, got {}",
            v["volume"]
        );
        assert!((v["opacity"] - 70.0).abs() < 1e-3);
        assert!((v["speed"] - 2.0).abs() < 1e-4);
        assert!((v["fade_in"] - 1.0).abs() < 1e-4, "30 frames @30fps = 1s");
        assert!((v["fade_out"] - 0.5).abs() < 1e-4);
        assert!((v["scale"] - 50.0).abs() < 1e-3);
        assert!((v["rotation"] - 45.0).abs() < 1e-3);
        // top-left = (0.5 - 0.25) * 1920 = 480
        assert!((v["position_x"] - 480.0).abs() < 1e-2);
        assert!((v["position_y"] - 270.0).abs() < 1e-2);
    }

    #[test]
    fn derive_resolves_keyframes_at_playhead() {
        let clip = test_clip(serde_json::json!({
            "startFrame": 50,
            "opacity": 1.0,
            "opacityTrack": {"keyframes": [
                {"frame": 0, "value": 0.0, "interpolationOut": "linear"},
                {"frame": 100, "value": 1.0, "interpolationOut": "linear"}
            ]},
            "volumeTrack": {"keyframes": [
                {"frame": 0, "value": 0.0, "interpolationOut": "linear"},
                {"frame": 100, "value": -12.0, "interpolationOut": "linear"}
            ]}
        }));
        // playhead 100 → local frame 50 → opacity 0.5, volume -6 dB
        let v = derive_scrub_values(&clip, 100, 30, 1920, 1080);
        assert!((v["opacity"] - 50.0).abs() < 1e-3, "kf-resolved opacity");
        assert!((v["volume"] + 6.0).abs() < 1e-3, "kf dB sampled directly");
        // playhead before the clip clamps to local 0
        let v0 = derive_scrub_values(&clip, 0, 30, 1920, 1080);
        assert!((v0["opacity"]).abs() < 1e-3);
    }

    #[test]
    fn commit_args_volume_converts_db_to_linear() {
        let clip = test_clip(serde_json::json!({}));
        let (tool, args) = scrub_commit_args("volume", -6.0, &clip, 1920, 1080).unwrap();
        assert_eq!(tool, "set_clip_properties");
        let vol = args["properties"]["volume"].as_f64().unwrap();
        assert!((vol - timeline_core::linear_from_db(-6.0)).abs() < 1e-9);
        assert_eq!(args["clipIds"][0], "c1");
        // Floor hard-mutes.
        let (_, args) = scrub_commit_args("volume", -60.0, &clip, 1920, 1080).unwrap();
        assert_eq!(args["properties"]["volume"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn commit_args_opacity_speed_rotation() {
        let clip = test_clip(serde_json::json!({}));
        let (_, args) = scrub_commit_args("opacity", 70.0, &clip, 1920, 1080).unwrap();
        assert!((args["properties"]["opacity"].as_f64().unwrap() - 0.7).abs() < 1e-9);
        let (_, args) = scrub_commit_args("speed", 2.0, &clip, 1920, 1080).unwrap();
        assert!((args["properties"]["speed"].as_f64().unwrap() - 2.0).abs() < 1e-9);
        let (_, args) = scrub_commit_args("speed", 100.0, &clip, 1920, 1080).unwrap();
        assert!(
            (args["properties"]["speed"].as_f64().unwrap() - 4.0).abs() < 1e-9,
            "speed clamps to the Swift 0.25–4.0 range"
        );
        let (_, args) = scrub_commit_args("rotation", 90.0, &clip, 1920, 1080).unwrap();
        assert!(
            (args["properties"]["transform"]["rotation"]
                .as_f64()
                .unwrap()
                - 90.0)
                .abs()
                < 1e-9
        );
    }

    #[test]
    fn commit_args_scale_preserves_aspect() {
        let clip = test_clip(serde_json::json!({
            "transform": {"centerX": 0.5, "centerY": 0.5, "width": 0.8, "height": 0.4}
        }));
        let (_, args) = scrub_commit_args("scale", 40.0, &clip, 1920, 1080).unwrap();
        let t = &args["properties"]["transform"];
        assert!((t["width"].as_f64().unwrap() - 0.4).abs() < 1e-9);
        assert!(
            (t["height"].as_f64().unwrap() - 0.2).abs() < 1e-9,
            "height keeps the 2:1 shape"
        );
    }

    #[test]
    fn commit_args_position_converts_top_left_to_center() {
        let clip = test_clip(serde_json::json!({
            "transform": {"centerX": 0.5, "centerY": 0.5, "width": 0.5, "height": 0.5}
        }));
        let (_, args) = scrub_commit_args("position_x", 480.0, &clip, 1920, 1080).unwrap();
        let cx = args["properties"]["transform"]["centerX"].as_f64().unwrap();
        assert!((cx - 0.5).abs() < 1e-9, "480px top-left + w/2 = 0.5 centre");
        let (_, args) = scrub_commit_args("position_y", 0.0, &clip, 1920, 1080).unwrap();
        let cy = args["properties"]["transform"]["centerY"].as_f64().unwrap();
        assert!((cy - 0.25).abs() < 1e-9);
    }

    #[test]
    fn commit_args_text_size_only_for_text_clips() {
        let video = test_clip(serde_json::json!({}));
        assert!(scrub_commit_args("text_size", 48.0, &video, 1920, 1080).is_none());
        let text = test_clip(serde_json::json!({"mediaType": "text"}));
        let (tool, args) = scrub_commit_args("text_size", 48.0, &text, 1920, 1080).unwrap();
        assert_eq!(tool, "update_text");
        // #330 nested style patch — never the deprecated flat compat key.
        assert!((args["style"]["fontSize"].as_f64().unwrap() - 48.0).abs() < 1e-9);
        assert!(args.get("fontSize").is_none(), "flat fontSize key retired");
        // Clamps to the Swift 12–300 range.
        let (_, args) = scrub_commit_args("text_size", 1.0, &text, 1920, 1080).unwrap();
        assert!((args["style"]["fontSize"].as_f64().unwrap() - 12.0).abs() < 1e-9);
    }

    /// Every inspector update_text payload sends the nested `style` object and
    /// none of the deprecated flat keys (fontName/fontSize/color/alignment).
    #[test]
    fn update_text_payloads_use_nested_style_without_flat_keys() {
        for (style, flat_key) in [
            (serde_json::json!({"fontName": "Anton"}), "fontName"),
            (serde_json::json!({"fontSize": 42.0}), "fontSize"),
            (serde_json::json!({"color": "#FF0000"}), "color"),
            (serde_json::json!({"alignment": "center"}), "alignment"),
        ] {
            let args = update_text_style_args("c1", style.clone());
            assert_eq!(args["clipIds"][0], "c1");
            assert_eq!(args["style"], style);
            assert!(
                args.get(flat_key).is_none(),
                "flat '{flat_key}' must not appear at the top level"
            );
            // Top level carries exactly clipIds + style.
            assert_eq!(args.as_object().unwrap().len(), 2);
        }
    }

    #[test]
    fn commit_args_fades_have_no_tool_path_yet() {
        let clip = test_clip(serde_json::json!({}));
        assert!(scrub_commit_args("fade_in", 1.0, &clip, 1920, 1080).is_none());
        assert!(scrub_commit_args("fade_out", 1.0, &clip, 1920, 1080).is_none());
    }

    #[test]
    fn crop_aspect_mirrors_swift_presets() {
        assert_eq!(CropAspect::ALL.len(), 8);
        assert_eq!(CropAspect::Free.label(), "Custom");
        assert_eq!(CropAspect::Original.label(), "Original");
        assert_eq!(CropAspect::R16x9.pixel_aspect(), Some(16.0 / 9.0));
        assert_eq!(CropAspect::R9x16.pixel_aspect(), Some(9.0 / 16.0));
        assert_eq!(CropAspect::Free.pixel_aspect(), None);
        assert_eq!(CropAspect::Original.pixel_aspect(), None);
    }

    #[test]
    fn format_file_size_matches_byte_count_formatter_style() {
        assert_eq!(format_file_size(0), "0 bytes");
        assert_eq!(format_file_size(999), "999 bytes");
        assert_eq!(format_file_size(12_000), "12 KB");
        assert_eq!(format_file_size(42_300_000), "42.3 MB");
        assert_eq!(format_file_size(1_020_000_000), "1.02 GB");
    }

    #[test]
    fn clip_local_frame_clamps_to_clip_range() {
        let clip = test_clip(serde_json::json!({"startFrame": 50, "durationFrames": 100}));
        assert_eq!(clip_local_frame(&clip, 0), 0);
        assert_eq!(clip_local_frame(&clip, 50), 0);
        assert_eq!(clip_local_frame(&clip, 100), 50);
        assert_eq!(clip_local_frame(&clip, 500), 100);
    }

    #[test]
    fn clip_type_labels_cover_all_variants() {
        assert_eq!(clip_type_label(ClipType::Video), "Video");
        assert_eq!(clip_type_label(ClipType::Audio), "Audio");
        assert_eq!(clip_type_label(ClipType::Image), "Image");
        assert_eq!(clip_type_label(ClipType::Text), "Text");
        assert_eq!(clip_type_label(ClipType::Sequence), "Sequence");
    }

    #[test]
    fn model_display_name_falls_back_to_raw_id() {
        assert_eq!(model_display_name("not-a-real-model"), "not-a-real-model");
    }
}
