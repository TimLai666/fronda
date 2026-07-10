//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::generation_view::{
    credit_shortfall_message, has_inflight_generation, has_insufficient_credits,
    interpret_submission, GenerationView, SubmitOutcome,
};
use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, ComponentSize, DropZone, FontSize, IconSize,
    Layout, MediaPanel, Opacity, Radius, Spacing, Status, Text,
};
use core_model::{ClipType, MediaFolder, MediaManifest, MediaManifestEntry, Timeline};
use generation_core::model_catalog::{self, AudioCategory, ModelCaps, ModelConfig};
use gpui::{
    deferred, div, prelude::*, px, AnyElement, App, ClickEvent, Context, DragMoveEvent, Entity,
    ExternalPaths, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Render, SharedString, Styled, Window,
};
use std::sync::OnceLock;
use timeline_core::{AssetDrag, TimelineMathExt};

/// Floating name chip shown while dragging a media asset between panels.
struct AssetDragPreview {
    name: String,
}

impl Render for AssetDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(Spacing::SM))
            .py(px(Spacing::XXS))
            .rounded(px(Radius::SM))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .text_size(px(FontSize::XS))
            .text_color(Text::PRIMARY)
            .child(self.name.clone())
    }
}

// ── Library view state (pure logic; media-library-ui spec) ──────────────────

/// Grid organization (Swift MediaTab.ViewMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryViewMode {
    #[default]
    Folders,
    Flat,
    Grouped,
}

impl LibraryViewMode {
    pub fn all() -> [LibraryViewMode; 3] {
        [Self::Folders, Self::Flat, Self::Grouped]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Folders => "Folders",
            Self::Flat => "Flat",
            Self::Grouped => "Grouped",
        }
    }
}

/// Grid ordering (Swift MediaTab.SortMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySortKey {
    Name,
    #[default]
    DateAdded,
    Duration,
    Type,
}

impl LibrarySortKey {
    pub fn all() -> [LibrarySortKey; 4] {
        [Self::Name, Self::DateAdded, Self::Duration, Self::Type]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::DateAdded => "Date Added",
            Self::Duration => "Duration",
            Self::Type => "Type",
        }
    }
}

/// View-only media-library state (search, navigation, selection). Pure and
/// unit-testable; the manifest itself stays in the shared executor.
#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    pub search_query: String,
    pub view_mode: LibraryViewMode,
    pub sort_key: LibrarySortKey,
    /// Empty = all types pass.
    pub type_filter: Vec<ClipType>,
    pub filter_ai: bool,
    pub current_folder: Option<String>,
    pub selection: Vec<String>,
    /// Last plainly-clicked or toggled id; shift-click extends from here.
    pub selection_anchor: Option<String>,
}

impl LibraryState {
    pub fn trimmed_query(&self) -> &str {
        self.search_query.trim()
    }

    pub fn search_active(&self) -> bool {
        !self.trimmed_query().is_empty()
    }

    pub fn has_active_filters(&self) -> bool {
        !self.type_filter.is_empty() || self.filter_ai
    }

    pub fn toggle_type_filter(&mut self, t: ClipType) {
        if let Some(pos) = self.type_filter.iter().position(|x| *x == t) {
            self.type_filter.remove(pos);
        } else {
            self.type_filter.push(t);
        }
    }

    pub fn clear_filters(&mut self) {
        self.type_filter.clear();
        self.filter_ai = false;
    }

    /// Plain click: the id becomes the whole selection and the anchor.
    pub fn select_click(&mut self, id: &str) {
        self.selection = vec![id.to_string()];
        self.selection_anchor = Some(id.to_string());
    }

    /// Ctrl/cmd-click: toggle membership; the id becomes the anchor.
    pub fn select_toggle(&mut self, id: &str) {
        if let Some(pos) = self.selection.iter().position(|x| x == id) {
            self.selection.remove(pos);
        } else {
            self.selection.push(id.to_string());
        }
        self.selection_anchor = Some(id.to_string());
    }

    /// Shift-click: select the contiguous span between the anchor and the id
    /// in `ordered` (the current display order). Falls back to a plain click
    /// when there is no usable anchor. The anchor is kept so a further
    /// shift-click re-extends from the same origin.
    pub fn select_range(&mut self, ordered: &[String], id: &str) {
        let to = ordered.iter().position(|x| x == id);
        let from = self
            .selection_anchor
            .as_ref()
            .and_then(|a| ordered.iter().position(|x| x == a));
        let (Some(from), Some(to)) = (from, to) else {
            self.select_click(id);
            return;
        };
        let (lo, hi) = (from.min(to), from.max(to));
        self.selection = ordered[lo..=hi].to_vec();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_anchor = None;
    }
}

/// Stable sort key for ClipType (Swift sorts by rawValue).
fn clip_type_key(t: &ClipType) -> &'static str {
    match t {
        ClipType::Audio => "audio",
        ClipType::Image => "image",
        ClipType::Lottie => "lottie",
        ClipType::Sequence => "sequence",
        ClipType::Shape => "shape",
        ClipType::Text => "text",
        ClipType::Video => "video",
    }
}

/// Type filter + AI filter + name-substring search (Swift passesFilters).
fn entry_passes(entry: &MediaManifestEntry, state: &LibraryState) -> bool {
    let type_ok = state.type_filter.is_empty() || state.type_filter.contains(&entry.r#type);
    let ai_ok = !state.filter_ai || entry.generation_input.is_some();
    let q = state.trimmed_query().to_lowercase();
    let name_ok = q.is_empty() || entry.name.to_lowercase().contains(&q);
    type_ok && ai_ok && name_ok
}

fn sort_entries<'a>(
    mut entries: Vec<&'a MediaManifestEntry>,
    key: LibrarySortKey,
) -> Vec<&'a MediaManifestEntry> {
    match key {
        // Manifest order is insertion order (Swift .dateAdded keeps it).
        LibrarySortKey::DateAdded => {}
        LibrarySortKey::Name => {
            entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        LibrarySortKey::Duration => {
            entries.sort_by(|a, b| b.duration.total_cmp(&a.duration));
        }
        LibrarySortKey::Type => {
            entries.sort_by(|a, b| clip_type_key(&a.r#type).cmp(clip_type_key(&b.r#type)));
        }
    }
    entries
}

/// Filtered + sorted assets of one folder bucket.
pub fn visible_entries_in<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
    folder: Option<&str>,
) -> Vec<&'a MediaManifestEntry> {
    sort_entries(
        manifest
            .entries
            .iter()
            .filter(|e| e.folder_id.as_deref() == folder && entry_passes(e, state))
            .collect(),
        state.sort_key,
    )
}

/// Assets visible in the grid: filter → folder scope → sort. An active search
/// spans the whole library (Swift switches to searchResults); Flat and Grouped
/// span the library; Folders shows the current folder's bucket.
pub fn visible_entries<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<&'a MediaManifestEntry> {
    if state.search_active() || state.view_mode != LibraryViewMode::Folders {
        sort_entries(
            manifest
                .entries
                .iter()
                .filter(|e| entry_passes(e, state))
                .collect(),
            state.sort_key,
        )
    } else {
        visible_entries_in(manifest, state, state.current_folder.as_deref())
    }
}

/// Folder tiles: only in Folders view while not searching — the current
/// folder's subfolders.
pub fn visible_folders<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<&'a MediaFolder> {
    if state.search_active() || state.view_mode != LibraryViewMode::Folders {
        return Vec::new();
    }
    manifest
        .folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == state.current_folder.as_deref())
        .collect()
}

/// Breadcrumb chain root→leaf for a folder. Cycle-safe.
pub fn folder_path<'a>(
    manifest: &'a MediaManifest,
    folder_id: Option<&str>,
) -> Vec<&'a MediaFolder> {
    let mut path: Vec<&MediaFolder> = Vec::new();
    let mut cur = folder_id;
    while let Some(id) = cur {
        let Some(f) = manifest.folders.iter().find(|f| f.id == id) else {
            break;
        };
        if path.iter().any(|p| p.id == f.id) {
            break; // cycle guard
        }
        path.push(f);
        cur = f.parent_folder_id.as_deref();
    }
    path.reverse();
    path
}

/// Subfolder + asset count shown on a folder tile.
pub fn folder_child_count(manifest: &MediaManifest, folder_id: &str) -> usize {
    manifest
        .folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == Some(folder_id))
        .count()
        + manifest
            .entries
            .iter()
            .filter(|e| e.folder_id.as_deref() == Some(folder_id))
            .count()
}

/// Grouped-view sections: root bucket first (skipped when empty), then every
/// folder ordered by its full path, each with its filtered + sorted assets.
pub fn grouped_sections<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<(Option<&'a str>, String, Vec<&'a MediaManifestEntry>)> {
    let mut sections = Vec::new();
    let root = visible_entries_in(manifest, state, None);
    if !root.is_empty() {
        sections.push((None, "Library".to_string(), root));
    }
    let mut folders: Vec<(&MediaFolder, String)> = manifest
        .folders
        .iter()
        .map(|f| {
            let title = folder_path(manifest, Some(&f.id))
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            (f, title)
        })
        .collect();
    folders.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    for (f, title) in folders {
        sections.push((
            Some(f.id.as_str()),
            title,
            visible_entries_in(manifest, state, Some(&f.id)),
        ));
    }
    sections
}

// ── Captions tab logic (pure; captions-tab spec) ────────────────────────────

/// Swift `AppTheme.Caption` values. Local to this file for now — this change
/// is scoped away from theme.rs; move there on the next theme pass.
struct CaptionTheme;
impl CaptionTheme {
    const DEFAULT_FONT_SIZE: f64 = 48.0;
    const MIN_FONT_SIZE: f64 = 12.0;
    const MAX_FONT_SIZE: f64 = 300.0;
    const MIN_POSITION: f64 = 0.0;
    const MAX_POSITION: f64 = 1.0;
    const CENTER_SNAP_VALUE: f64 = 0.5;
    const CENTER_SNAP_THRESHOLD: f64 = 0.02;
    const DEFAULT_CENTER_Y: f64 = 0.9;
    /// Swift TextLayout.referenceCanvasHeight — caption pt sizes are relative
    /// to a 1080-tall canvas (same reference as render_core::text).
    const REFERENCE_CANVAS_HEIGHT: f64 = 1080.0;
}

const GENERATE_BUTTON_HEIGHT: f32 = 32.0;
const CAPTION_PREVIEW_TEXT: &str = "Captions will look like this";

/// Bundled families render_core::text can rasterize — the Rust counterpart of
/// Swift BundledFonts.families (sorted). System fonts are excluded on purpose:
/// the exporter falls back to Poppins for anything unbundled.
pub const BUNDLED_FONT_FAMILIES: [&str; 6] = [
    "Anton",
    "Basement Grotesque",
    "Bebas Neue",
    "Permanent Marker",
    "Poppins",
    "Shrikhand",
];

/// Preset swatches standing in for Swift's ColorField color well (a shared
/// gpui color-picker component is Inspector-Text-tab scope, audit row 5).
pub const CAPTION_SWATCHES: [(&str, u32); 8] = [
    ("White", 0xFFFFFF),
    ("Black", 0x000000),
    ("Yellow", 0xF5C542),
    ("Orange", 0xE8833A),
    ("Red", 0xE54F4F),
    ("Green", 0x58B368),
    ("Blue", 0x4F8FE5),
    ("Pink", 0xE57FB3),
];

/// Stand-in locale list until a TranscriptionProvider host supplies real ones
/// (Swift: Transcription.supportedLocales()).
pub const CAPTION_LANGUAGES: [(&str, &str); 11] = [
    ("en-US", "English"),
    ("es-ES", "Spanish"),
    ("fr-FR", "French"),
    ("de-DE", "German"),
    ("it-IT", "Italian"),
    ("pt-BR", "Portuguese"),
    ("ja-JP", "Japanese"),
    ("ko-KR", "Korean"),
    ("zh-TW", "Chinese"),
    ("hi-IN", "Hindi"),
    ("ar-SA", "Arabic"),
];

/// Swift EditorViewModel.CaptionCase; wire form matches
/// search_core::caption::TextCase's serde values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptionCase {
    #[default]
    Auto,
    Upper,
    Lower,
}

impl CaptionCase {
    pub fn all() -> [CaptionCase; 3] {
        [Self::Auto, Self::Upper, Self::Lower]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Upper => "UPPERCASE",
            Self::Lower => "lowercase",
        }
    }

    pub fn config_value(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Upper => "upper",
            Self::Lower => "lower",
        }
    }
}

/// Captions tab view state (Swift CaptionTab @State). Style fields live at
/// the app layer: search_core::CaptionConfig already carries
/// censor/locale/case, and Swift ships style/center beside — not inside —
/// the config in its CaptionRequest.
#[derive(Debug, Clone)]
pub struct CaptionsState {
    /// core_model::TextStyle default font.
    pub font_name: String,
    pub font_size: f64,
    pub color: u32,
    pub background_color: u32,
    pub background_enabled: bool,
    pub text_case: CaptionCase,
    pub censor_profanity: bool,
    pub center_x: f64,
    pub center_y: f64,
    /// BCP-47 override; None = Auto. Seeded from timeline.transcription_language.
    pub language: Option<String>,
    pub language_touched: bool,
    /// None = Auto source (timeline selection, else all captionable audio).
    pub selected_track_id: Option<String>,
    pub is_generating: bool,
    pub note: Option<String>,
}

impl Default for CaptionsState {
    fn default() -> Self {
        Self {
            font_name: "Helvetica-Bold".to_string(),
            font_size: CaptionTheme::DEFAULT_FONT_SIZE,
            color: 0xFFFFFF,
            background_color: 0x000000,
            background_enabled: false,
            text_case: CaptionCase::Auto,
            censor_profanity: false,
            center_x: CaptionTheme::CENTER_SNAP_VALUE,
            center_y: CaptionTheme::DEFAULT_CENTER_Y,
            language: None,
            language_touched: false,
            selected_track_id: None,
            is_generating: false,
            note: None,
        }
    }
}

fn clip_is_captionable(clip: &core_model::Clip) -> bool {
    matches!(
        clip.source_clip_type,
        ClipType::Audio | ClipType::Video
    )
}

/// Captionable clip ids, optionally limited to one track (Swift captionTargets).
pub fn captionable_clip_ids(timeline: &Timeline, track_id: Option<&str>) -> Vec<String> {
    timeline
        .tracks
        .iter()
        .filter(|t| track_id.map_or(true, |id| t.id == id))
        .flat_map(|t| t.clips.iter())
        .filter(|c| clip_is_captionable(c))
        .map(|c| c.id.clone())
        .collect()
}

/// Timeline-selected captionable clips (Swift liveTargets).
pub fn selected_captionable_ids(timeline: &Timeline) -> Vec<String> {
    timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| clip_is_captionable(c) && timeline.selected_clip_ids.contains(&c.id))
        .map(|c| c.id.clone())
        .collect()
}

/// (track id, "V1"-style label, captionable count) rows for the source menu.
pub fn caption_track_entries(timeline: &Timeline) -> Vec<(String, String, usize)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let count = t.clips.iter().filter(|c| clip_is_captionable(c)).count();
            (count > 0).then(|| {
                (
                    t.id.clone(),
                    timeline_core::display_label_for_track(timeline, i),
                    count,
                )
            })
        })
        .collect()
}

/// Swift automaticSourceSummary / sourceSummary.
pub fn caption_source_summary(timeline: &Timeline, selected_track: Option<&str>) -> String {
    if let Some(track_id) = selected_track {
        let Some(index) = timeline.tracks.iter().position(|t| t.id == track_id) else {
            return "No track".to_string();
        };
        let count = captionable_clip_ids(timeline, Some(track_id)).len();
        return format!(
            "{} · {}",
            timeline_core::display_label_for_track(timeline, index),
            count
        );
    }
    let selected = selected_captionable_ids(timeline);
    if !selected.is_empty() {
        return format!("Selected Clips · {}", selected.len());
    }
    if captionable_clip_ids(timeline, None).is_empty() {
        "No audio".to_string()
    } else {
        "Auto".to_string()
    }
}

/// Swift effectiveCount — what the Generate button gates on.
pub fn caption_effective_count(timeline: &Timeline, selected_track: Option<&str>) -> usize {
    caption_source_ids(timeline, selected_track).len()
}

/// The clip ids a Generate run targets (Swift sourceClipIds; Auto resolves to
/// the selection, else every captionable clip — the add_captions tool needs
/// explicit ids).
pub fn caption_source_ids(timeline: &Timeline, selected_track: Option<&str>) -> Vec<String> {
    match selected_track {
        Some(id) => captionable_clip_ids(timeline, Some(id)),
        None => {
            let selected = selected_captionable_ids(timeline);
            if selected.is_empty() {
                captionable_clip_ids(timeline, None)
            } else {
                selected
            }
        }
    }
}

/// Swift CaptionTab.snapCenter.
pub fn snap_center(v: f64) -> f64 {
    if (v - CaptionTheme::CENTER_SNAP_VALUE).abs() < CaptionTheme::CENTER_SNAP_THRESHOLD {
        CaptionTheme::CENTER_SNAP_VALUE
    } else {
        v
    }
}

/// Note for a caption Generate result; None = a real run was queued.
pub fn caption_generate_note(result: &Result<serde_json::Value, String>) -> Option<String> {
    match interpret_submission(result) {
        SubmitOutcome::Queued(_) => None,
        SubmitOutcome::Unavailable => {
            Some("Transcription unavailable — no speech engine is connected.".to_string())
        }
        SubmitOutcome::Failed(reason) => Some(reason),
    }
}

/// Swift CaptionTab.captionTask prompt prefix.
pub fn caption_agent_prompt(task: &str) -> String {
    format!(
        "If the timeline has no captions yet, transcribe the spoken audio and \
         add captions on word boundaries first. Then {task}"
    )
}

pub const CAPTION_AGENT_TASKS: [(&str, &str); 3] = [
    (
        "Remove filler words",
        "remove filler words (um, uh, er, like, you know) from the captions, keeping each caption's timing unchanged.",
    ),
    (
        "Fix names & jargon",
        "fix any misspelled names, brand names, or technical jargon in the captions using the surrounding context, keeping timing unchanged.",
    ),
    (
        "Add emoji",
        "add relevant emoji to the captions, keeping the text and timing otherwise unchanged.",
    ),
];

pub const CAPTION_TRANSLATE_LANGUAGES: [&str; 10] = [
    "Spanish",
    "French",
    "German",
    "Italian",
    "Portuguese",
    "Japanese",
    "Korean",
    "Chinese",
    "Hindi",
    "Arabic",
];

pub fn caption_translate_task(language: &str) -> String {
    format!("translate the captions to {language}, keeping each caption's timing unchanged.")
}

// ── Music tab logic (pure; music-tab spec) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MusicMode {
    #[default]
    VideoToMusic,
    TextToMusic,
}

impl MusicMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::VideoToMusic => "Video to Music",
            Self::TextToMusic => "Text to Music",
        }
    }
}

/// Music tab view state (Swift MusicTab @State).
#[derive(Debug, Clone)]
pub struct MusicState {
    pub mode: MusicMode,
    pub selected_model_id: Option<String>,
    pub prompt: String,
    pub text_duration: f64,
    pub is_generating: bool,
    pub note: Option<String>,
}

impl Default for MusicState {
    fn default() -> Self {
        Self {
            mode: MusicMode::VideoToMusic,
            selected_model_id: None,
            prompt: String::new(),
            text_duration: 90.0,
            is_generating: false,
            note: None,
        }
    }
}

const MUSIC_DURATION_MIN: f64 = 1.0;
const MUSIC_DURATION_MAX: f64 = 600.0;

/// Music-capable catalog entries (Swift filters on video input + music
/// category; the Rust catalog carries no `inputs` metadata, so category is
/// the whole filter).
pub fn music_models() -> Vec<&'static ModelConfig> {
    model_catalog::catalog()
        .iter()
        .filter(|m| matches!(&m.caps, ModelCaps::Audio(c) if c.category == AudioCategory::Music))
        .collect()
}

/// Selected model falling back to the first music model (Swift `model`).
pub fn music_model_for(id: Option<&str>) -> Option<&'static ModelConfig> {
    let models = music_models();
    id.and_then(|id| models.iter().find(|m| m.id == id).copied())
        .or_else(|| models.first().copied())
}

/// Swift supportsTextMode. The Rust catalog has no per-model `inputs` list;
/// every music model is prompt-driven, so text mode is always offered.
pub fn music_supports_text_mode(_model: &ModelConfig) -> bool {
    true
}

pub fn effective_music_mode(mode: MusicMode, model: Option<&ModelConfig>) -> MusicMode {
    match model {
        Some(m) if music_supports_text_mode(m) => mode,
        _ => MusicMode::VideoToMusic,
    }
}

fn music_caps(model: &ModelConfig) -> Option<&model_catalog::AudioCaps> {
    match &model.caps {
        ModelCaps::Audio(c) => Some(c),
        _ => None,
    }
}

/// USD estimate (Swift estimatedCost; None when the span is empty).
pub fn music_cost(model: &ModelConfig, prompt: &str, duration_seconds: f64) -> Option<f64> {
    let secs = duration_seconds.round() as i64;
    if secs <= 0 {
        return None;
    }
    model_catalog::audio_cost(music_caps(model)?, prompt.trim(), Some(secs))
}

/// Swift AudioModelConfig.validate(spanSeconds:) with the catalog defaults
/// (minSeconds 1, maxSeconds 900 — the Rust catalog carries no overrides).
pub fn music_span_note(model: &ModelConfig, span_seconds: f64) -> Option<String> {
    const MIN_SECONDS: i64 = 1;
    const MAX_SECONDS: i64 = 900;
    let s = span_seconds.round() as i64;
    if s < MIN_SECONDS {
        return Some(format!(
            "{} needs at least {MIN_SECONDS}s of video (selection is {s}s).",
            model.display_name
        ));
    }
    if s > MAX_SECONDS {
        return Some(format!(
            "{} accepts at most {MAX_SECONDS}s of video (selection is {s}s).",
            model.display_name
        ));
    }
    None
}

/// Swift MusicTab.validationNote, in order; None = generation may proceed.
pub fn music_validation_note(
    model: Option<&ModelConfig>,
    text_mode: bool,
    prompt: &str,
    span_seconds: f64,
    cost: Option<f64>,
    credits_remaining: Option<f64>,
) -> Option<String> {
    let Some(model) = model else {
        return Some("No music models available.".to_string());
    };
    if text_mode {
        if prompt.trim().is_empty() {
            return Some("Describe the music to generate.".to_string());
        }
    } else {
        if span_seconds <= 0.0 {
            return Some(
                "Add video to the timeline, then mark a range to score only part of it."
                    .to_string(),
            );
        }
        if let Some(issue) = music_span_note(model, span_seconds) {
            return Some(issue);
        }
    }
    if has_insufficient_credits(cost, credits_remaining) {
        return Some(credit_shortfall_message(
            cost.unwrap_or_default(),
            credits_remaining.unwrap_or_default(),
        ));
    }
    None
}

/// Swift generateLabel: cost-badged Generate.
pub fn music_generate_label(cost: Option<f64>) -> String {
    match cost {
        Some(c) if c > 0.0 => format!("Generate · {}", model_catalog::format_usd(Some(c))),
        _ => "Generate".to_string(),
    }
}

/// m:ss clock (Swift MusicTab.clock).
pub fn music_clock(frame: i64, fps: i64) -> String {
    let total = (frame as f64 / fps.max(1) as f64) as i64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Swift sourceSummary. The Rust app has no marked in/out ranges yet, so the
/// span is always the whole timeline.
pub fn music_source_summary(total_frames: i64, fps: i64) -> String {
    if total_frames <= 0 {
        return "No video".to_string();
    }
    let span = total_frames as f64 / fps.max(1) as f64;
    format!(
        "Whole timeline · {} – {} · {:.1}s",
        music_clock(0, fps),
        music_clock(total_frames, fps),
        span
    )
}

/// Note for a music Generate result; None = a real job was queued.
pub fn music_generate_note(result: &Result<serde_json::Value, String>) -> Option<String> {
    match interpret_submission(result) {
        SubmitOutcome::Queued(_) => None,
        SubmitOutcome::Unavailable => {
            Some("Generation unavailable — no backend is connected.".to_string())
        }
        SubmitOutcome::Failed(reason) => Some(reason),
    }
}

pub const MUSIC_MOODS: [&str; 5] = ["Cinematic", "Upbeat", "Ambient", "Tense", "Lo-fi"];

pub const MUSIC_TIMELINE_PROMPT: &str =
    "Score my timeline with music that matches the visuals. Use a video-to-music model on the \
     full timeline span so the music follows the edit, and place it on an audio track.";

pub fn music_mood_prompt(mood: &str) -> String {
    format!(
        "Generate {} music for my timeline and place it on an audio track aligned to the edit.",
        mood.to_lowercase()
    )
}

// ── Scrub fields (inspector ScrubbableNumberField pattern) ──────────────────

/// Marker type for captions/music scrub drags.
#[derive(Clone)]
struct TabScrub;

/// Transparent drag preview required by gpui's on_drag API.
struct TabScrubPreview;
impl Render for TabScrubPreview {
    fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(0.0)).h(px(0.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabScrubField {
    CaptionSize,
    CaptionX,
    CaptionY,
    MusicDuration,
}

#[derive(Clone)]
struct TabScrubSession {
    field: TabScrubField,
    start_x: f32,
    start_value: f64,
}

/// Per-field sensitivity + clamp (+ centre snap for placement).
pub fn tab_scrub_value(field: TabScrubField, start_value: f64, dx: f64) -> f64 {
    match field {
        TabScrubField::CaptionSize => {
            (start_value + dx).clamp(CaptionTheme::MIN_FONT_SIZE, CaptionTheme::MAX_FONT_SIZE)
        }
        TabScrubField::CaptionX | TabScrubField::CaptionY => snap_center(
            (start_value + dx * 0.005)
                .clamp(CaptionTheme::MIN_POSITION, CaptionTheme::MAX_POSITION),
        ),
        TabScrubField::MusicDuration => {
            (start_value + dx).clamp(MUSIC_DURATION_MIN, MUSIC_DURATION_MAX)
        }
    }
}

// ── Agent chat handoff seam ──────────────────────────────────────────────────

/// Cross-view chat handoff (Swift: agentService.newChat + draft + open panel).
/// The app shell installs one once the chat panel accepts drafts; until then
/// the tabs surface an explicit note instead of pretending.
static AGENT_CHAT_HANDOFF: OnceLock<Box<dyn Fn(&str) + Send + Sync>> = OnceLock::new();

pub fn set_agent_chat_handoff(handoff: Box<dyn Fn(&str) + Send + Sync>) {
    let _ = AGENT_CHAT_HANDOFF.set(handoff);
}

const AGENT_CHAT_UNWIRED_NOTE: &str =
    "Agent chat isn't connected to this tab yet. Ask Agent in the chat panel.";

/// Deliver a prompt to the chat panel; the returned note explains when it
/// can't be.
fn send_agent_handoff(prompt: &str) -> Option<String> {
    match AGENT_CHAT_HANDOFF.get() {
        Some(f) => {
            f(prompt);
            None
        }
        None => Some(AGENT_CHAT_UNWIRED_NOTE.to_string()),
    }
}

/// Which captions-tab dropdown is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptionsMenu {
    Source,
    Language,
    Font,
    Case,
    Agent,
    AgentTranslate,
}

/// Which music-tab dropdown is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MusicMenu {
    Input,
    Model,
    Agent,
    AgentMood,
}

/// Simple tooltip capsule for tab buttons.
struct TabTooltip {
    label: SharedString,
}

impl Render for TabTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(Spacing::SM))
            .py(px(Spacing::XXS))
            .rounded(px(Radius::SM))
            .bg(Background::PROMINENT)
            .text_color(Text::PRIMARY)
            .text_size(px(FontSize::XS))
            .child(self.label.clone())
    }
}

/// Which toolbar dropdown is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarMenu {
    View,
    Sort,
    Filter,
    Overflow,
}

/// Right-click target in the library grid: an asset or a folder tile.
#[derive(Debug, Clone)]
enum LibraryMenuTarget {
    Asset { id: String },
    Folder { id: String, name: String },
}

/// Media panel gpui entity.
pub struct MediaPanelView {
    pub state: MediaPanelState,
    /// Library view state (search / folders / sort / filter / selection).
    pub library: LibraryState,
    /// Snapshot of the shared manifest (rebuilt on revision bumps) so render
    /// and pure helpers work without holding the executor lock.
    manifest: MediaManifest,
    /// Search index status line from the executor ("" = nothing to show).
    search_status: String,
    focus_handle: FocusHandle,
    /// AI generation panel embedded in the media tab (Swift: GenerationView).
    pub generation: Entity<GenerationView>,
    /// Last seen shared-state revision; manifest changes rebuild the grid.
    state_revision: u64,
    /// Live search box (Swift: TextField bound to searchQuery).
    search_field: Entity<crate::text_field::TextField>,
    /// Inline folder rename (same pattern as the timeline tab rename).
    folder_rename_field: Entity<crate::text_field::TextField>,
    /// Folder id being renamed inline, if any.
    folder_editing: Option<String>,
    /// Inline asset rename (same TextField pattern, commits via rename_media).
    asset_rename_field: Entity<crate::text_field::TextField>,
    /// Asset id being renamed inline, if any.
    asset_editing: Option<String>,
    /// Right-click menu on asset/folder tiles (context-menu-system 2.2/2.3).
    library_menu: crate::context_menu::ContextMenuState<LibraryMenuTarget>,
    open_menu: Option<ToolbarMenu>,
    /// Captions tab state (captions-tab spec).
    pub captions: CaptionsState,
    /// Music tab state (music-tab spec).
    pub music: MusicState,
    /// Prompt editor for the music tab; `music.prompt` mirrors it.
    music_prompt_area: Entity<crate::text_area::TextArea>,
    captions_menu: Option<CaptionsMenu>,
    music_menu: Option<MusicMenu>,
    /// Scrub drag in progress on a captions/music numeric field.
    tab_scrub: Option<TabScrubSession>,
    /// Snapshot of the shared timeline (rebuilt on revision bumps).
    timeline: Timeline,
}

impl MediaPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let gen = cx.new(|cx| GenerationView::new(cx));
        let search_field = cx.new(|cx| crate::text_field::TextField::new(cx, "Search"));
        cx.subscribe(&search_field, |this, field, event, cx| {
            if matches!(event, crate::text_field::TextFieldEvent::Edited) {
                this.library.search_query = field.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let folder_rename_field =
            cx.new(|cx| crate::text_field::TextField::new(cx, "Folder name"));
        cx.subscribe(&folder_rename_field, |this, _field, event, cx| {
            if matches!(event, crate::text_field::TextFieldEvent::Submitted) {
                this.commit_folder_rename(cx);
                cx.notify();
            }
        })
        .detach();
        let asset_rename_field = cx.new(|cx| crate::text_field::TextField::new(cx, "Asset name"));
        cx.subscribe(&asset_rename_field, |this, _field, event, cx| {
            if matches!(event, crate::text_field::TextFieldEvent::Submitted) {
                this.commit_asset_rename(cx);
                cx.notify();
            }
        })
        .detach();
        let music_prompt_area = cx.new(|cx| {
            crate::text_area::TextArea::new(cx, "Describe the music…")
                .with_min_lines(2)
                .with_max_lines(5)
        });
        cx.subscribe(&music_prompt_area, |this: &mut Self, area, event, cx| {
            if matches!(event, crate::text_area::TextAreaEvent::Edited) {
                this.music.prompt = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            state: MediaPanelState::new(),
            library: LibraryState::default(),
            manifest: MediaManifest::default(),
            search_status: String::new(),
            focus_handle: cx.focus_handle(),
            generation: gen,
            state_revision: u64::MAX,
            search_field,
            folder_rename_field,
            folder_editing: None,
            asset_rename_field,
            asset_editing: None,
            library_menu: crate::context_menu::ContextMenuState::new(),
            open_menu: None,
            captions: CaptionsState::default(),
            music: MusicState::default(),
            music_prompt_area,
            captions_menu: None,
            music_menu: None,
            tab_scrub: None,
            timeline: Timeline::default(),
        };
        view.sync_from_shared_state();
        view
    }

    /// Rebuild grid data from the shared manifest when the revision moved.
    fn sync_from_shared_state(&mut self) -> bool {
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if revision == self.state_revision {
            return false;
        }
        self.state_revision = revision;
        let executor = hub.executor();
        let Ok(exec) = executor.lock() else {
            return false;
        };
        let root = hub.project_root();
        self.state
            .sync_from_manifest(exec.media_manifest(), root.as_deref());
        self.manifest = exec.media_manifest().clone();
        self.timeline = exec.timeline().clone();
        self.search_status = exec.search_status().to_string();
        drop(exec);
        // Captions/music view state that points at gone things falls back.
        if !self.captions.language_touched && self.captions.language.is_none() {
            self.captions.language = self.timeline.transcription_language.clone();
        }
        if self
            .captions
            .selected_track_id
            .as_ref()
            .is_some_and(|id| !self.timeline.tracks.iter().any(|t| &t.id == id))
        {
            self.captions.selected_track_id = None;
        }
        // The music overlay only reflects a genuinely in-flight generation.
        self.music.is_generating =
            self.music.is_generating && has_inflight_generation(&self.manifest);
        // Prune view state that points at deleted things (Swift
        // pruneStaleFolderState).
        let folder_exists =
            |id: &String| self.manifest.folders.iter().any(|f| &f.id == id);
        if self.library.current_folder.as_ref().is_some_and(|id| !folder_exists(id)) {
            self.library.current_folder = None;
        }
        if self.folder_editing.as_ref().is_some_and(|id| !folder_exists(id)) {
            self.folder_editing = None;
        }
        let entry_exists = |id: &String| self.manifest.entries.iter().any(|e| &e.id == id);
        if self.asset_editing.as_ref().is_some_and(|id| !entry_exists(id)) {
            self.asset_editing = None;
        }
        let menu_target_gone = match self.library_menu.target() {
            Some(LibraryMenuTarget::Asset { id }) => !entry_exists(id),
            Some(LibraryMenuTarget::Folder { id, .. }) => !folder_exists(id),
            None => false,
        };
        if menu_target_gone {
            self.library_menu.close();
        }
        self.library
            .selection
            .retain(|id| self.manifest.entries.iter().any(|e| &e.id == id));
        true
    }

    pub fn select_tab(&mut self, tab: MediaPanelTab, cx: &mut Context<Self>) {
        self.library_menu.close();
        self.state.select_tab(tab);
        cx.notify();
    }

    /// Run a tool on the shared executor; tool errors leave the UI unchanged.
    fn run_shared_tool(tool: &str, args: serde_json::Value) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            if let Err(reason) = exec.execute(tool, &args) {
                eprintln!("{tool} failed: {reason}");
            }
        }
    }

    /// Batch delete: every selected asset through delete_media.
    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        for id in std::mem::take(&mut self.library.selection) {
            Self::run_shared_tool("delete_media", serde_json::json!({ "mediaId": id }));
        }
        self.library.clear_selection();
        cx.notify();
    }

    /// New Folder in the current folder; opens the inline rename on it
    /// (Swift createNewFolderInCurrent).
    fn create_folder_in_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut args = serde_json::json!({ "name": "New Folder" });
        if let Some(parent) = &self.library.current_folder {
            args["parentFolderId"] = serde_json::Value::String(parent.clone());
        }
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let new_id = executor.lock().ok().and_then(|mut exec| {
            exec.execute("create_folder", &args).ok()?;
            exec.media_manifest().folders.last().map(|f| f.id.clone())
        });
        if let Some(id) = new_id {
            // The rename field only mounts on folder tiles — jump to the view
            // where the new folder is visible (review M2).
            self.library.view_mode = LibraryViewMode::Folders;
            self.library.search_query.clear();
            self.search_field.update(cx, |field, cx| field.set_text("", cx));
            self.folder_editing = Some(id);
            self.folder_rename_field.update(cx, |field, cx| {
                field.set_text("New Folder", cx);
            });
            window.focus(&self.folder_rename_field.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Commit an in-progress folder rename (Enter or click-away; Swift
    /// commits on focus loss). An empty name cancels.
    fn commit_folder_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.folder_editing.take() {
            let name = self.folder_rename_field.read(cx).text().trim().to_string();
            if !name.is_empty() {
                Self::run_shared_tool(
                    "rename_folder",
                    serde_json::json!({ "folderId": id, "name": name }),
                );
            }
        }
    }

    /// Begin inline rename of a folder tile. A rename already in progress
    /// commits first (Swift commits on focus loss).
    fn begin_folder_rename(
        &mut self,
        id: &str,
        seed: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_folder_rename(cx);
        self.commit_asset_rename(cx);
        self.folder_editing = Some(id.to_string());
        let seed = seed.to_string();
        self.folder_rename_field.update(cx, |field, cx| {
            field.set_text(seed, cx);
        });
        window.focus(&self.folder_rename_field.focus_handle(cx), cx);
        cx.notify();
    }

    /// Begin inline rename of an asset tile (context menu Rename). A rename
    /// already in progress commits first.
    fn begin_asset_rename(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_folder_rename(cx);
        self.commit_asset_rename(cx);
        let Some(name) = self
            .manifest
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
        else {
            return;
        };
        self.asset_editing = Some(id.to_string());
        self.asset_rename_field.update(cx, |field, cx| {
            field.set_text(name, cx);
        });
        window.focus(&self.asset_rename_field.focus_handle(cx), cx);
        cx.notify();
    }

    /// Commit an in-progress asset rename (Enter or click-away). An empty
    /// name cancels, mirroring the folder rename.
    fn commit_asset_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.asset_editing.take() {
            let name = self.asset_rename_field.read(cx).text().trim().to_string();
            if !name.is_empty() {
                Self::run_shared_tool(
                    "rename_media",
                    serde_json::json!({ "mediaId": id, "name": name }),
                );
            }
        }
    }

    /// Resolved on-disk source of an asset (existing files only).
    fn asset_source_path(&self, id: &str) -> Option<std::path::PathBuf> {
        self.state
            .items
            .iter()
            .find(|i| i.id == id)
            .and_then(|i| i.source_path.clone())
    }

    /// Asset tile menu (Swift AssetThumbnailView.contextMenuItems subset).
    /// Reveal is omitted when no local source file resolves (project-card
    /// missing-package pattern). Order defines activation indices.
    fn asset_menu_entries(can_reveal: bool) -> Vec<crate::context_menu::MenuEntry> {
        use crate::context_menu::MenuEntry;
        let mut entries = vec![MenuEntry::item("rename", "Rename")];
        if can_reveal {
            entries.push(MenuEntry::item("reveal", "Reveal in File Manager"));
        }
        entries.push(MenuEntry::separator());
        entries.push(MenuEntry::destructive("delete", "Delete"));
        entries
    }

    /// Folder tile menu (Swift FolderTileView.contextMenuItems).
    fn folder_menu_entries() -> Vec<crate::context_menu::MenuEntry> {
        use crate::context_menu::MenuEntry;
        vec![
            MenuEntry::item("open", "Open"),
            MenuEntry::item("rename", "Rename"),
            MenuEntry::separator(),
            MenuEntry::destructive("delete", "Delete"),
        ]
    }

    fn library_menu_entries(
        &self,
        target: &LibraryMenuTarget,
    ) -> Vec<crate::context_menu::MenuEntry> {
        match target {
            LibraryMenuTarget::Asset { id } => {
                Self::asset_menu_entries(self.asset_source_path(id).is_some())
            }
            LibraryMenuTarget::Folder { .. } => Self::folder_menu_entries(),
        }
    }

    /// Delete from the tile menu: the whole selection when the clicked asset
    /// is part of it (Swift contextTargetIds), else just that asset.
    fn delete_asset_via_menu(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.library.selection.iter().any(|s| s == id) {
            self.delete_selection(cx);
        } else {
            Self::run_shared_tool("delete_media", serde_json::json!({ "mediaId": id }));
            cx.notify();
        }
    }

    fn activate_library_menu_item(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.library_menu.target().cloned() else {
            return;
        };
        let entries = self.library_menu_entries(&target);
        if let crate::context_menu::Activation::Perform(action) =
            self.library_menu.activate(index, &entries)
        {
            match target {
                LibraryMenuTarget::Asset { id } => match action {
                    "rename" => self.begin_asset_rename(&id, window, cx),
                    "reveal" => {
                        if let Some(path) = self.asset_source_path(&id) {
                            crate::platform_adapter::reveal_in_file_manager(&path);
                        }
                    }
                    "delete" => self.delete_asset_via_menu(&id, cx),
                    _ => {}
                },
                LibraryMenuTarget::Folder { id, name } => match action {
                    "open" => {
                        self.library.current_folder = Some(id);
                        self.library.clear_selection();
                    }
                    "rename" => self.begin_folder_rename(&id, &name, window, cx),
                    "delete" => {
                        // Executor semantics: direct child assets move to the
                        // library root; subfolders are not re-parented.
                        Self::run_shared_tool(
                            "delete_folder",
                            serde_json::json!({ "folderId": id }),
                        );
                    }
                    _ => {}
                },
            }
        }
        cx.notify();
    }

    /// Asset ids in the grid's current display order (folders excluded) —
    /// the ordering shift-click ranges over.
    fn ordered_visible_ids(&self) -> Vec<String> {
        if !self.library.search_active() && self.library.view_mode == LibraryViewMode::Grouped {
            grouped_sections(&self.manifest, &self.library)
                .into_iter()
                .flat_map(|(_, _, entries)| entries.into_iter().map(|e| e.id.clone()))
                .collect()
        } else {
            visible_entries(&self.manifest, &self.library)
                .into_iter()
                .map(|e| e.id.clone())
                .collect()
        }
    }

    /// Click on an asset tile with the mouse-down modifiers applied.
    fn handle_asset_click(&mut self, id: &str, e: &gpui::MouseDownEvent, cx: &mut Context<Self>) {
        self.open_menu = None;
        // A click on another tile commits pending inline renames (the tile
        // stops propagation, so the grid's click-away can't).
        self.commit_folder_rename(cx);
        self.commit_asset_rename(cx);
        if e.modifiers.shift {
            let ordered = self.ordered_visible_ids();
            self.library.select_range(&ordered, id);
        } else if e.modifiers.platform || e.modifiers.control {
            self.library.select_toggle(id);
        } else {
            self.library.select_click(id);
        }
        cx.notify();
    }

    /// Escape closes the context menu, then cancels inline renames, then an
    /// open toolbar menu, then the selection (the rename TextField lets
    /// Escape bubble here).
    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key.as_str() != "escape" {
            return;
        }
        if self.library_menu.is_open() {
            self.library_menu.close();
            cx.stop_propagation();
            cx.notify();
        } else if self.folder_editing.take().is_some()
            || self.asset_editing.take().is_some()
            || self.open_menu.take().is_some()
        {
            cx.stop_propagation();
            cx.notify();
        } else if !self.library.selection.is_empty() {
            self.library.clear_selection();
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Focusable for MediaPanelView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Tab button: 26px square (Swift: IconSize.lg = 26).
/// Active: white@10% bg + 2.5px left-edge capsule in BorderColors::PRIMARY
/// (Swift: HoverHighlight(isActive) + Capsule overlay on leading edge).
fn tab_btn(id: &str, label: &str, is_active: bool) -> gpui::Stateful<gpui::Div> {
    let btn_size = IconSize::LG; // 26px
    let bg = if is_active {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.10,
        }
    } else {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        }
    };
    div()
        .id(id.to_string())
        .relative()
        .w(px(btn_size))
        .h(px(btn_size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .bg(bg)
        .text_color(if is_active {
            Text::PRIMARY
        } else {
            Text::TERTIARY
        })
        .text_size(px(FontSize::SM_MD))
        .child(label.to_string())
        // Left-edge accent capsule (Swift: Capsule overlay at topLeading)
        .when(is_active, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(5.0))
                    .w(px(2.5))
                    .h(px(16.0))
                    .rounded_full()
                    .bg(BorderColors::PRIMARY),
            )
        })
}

/// Media library empty state (Swift emptyStateView).
fn media_empty_state() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(Spacing::XS))
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::TITLE_1))
                .child("No media yet"),
        )
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child("Drop files here or import from disk"),
        )
}

/// Owned per-tile data assembled each render.
struct AssetTileData {
    id: String,
    name: String,
    kind: ClipType,
    image: Option<std::path::PathBuf>,
    selected: bool,
}

/// 80×60 thumbnail + name strip (Swift AssetThumbnailView) with a selection
/// ring; click handling is attached by the caller. A rename field replaces
/// the name strip while an inline rename is active.
fn asset_tile(
    data: &AssetTileData,
    rename_field: Option<Entity<crate::text_field::TextField>>,
) -> gpui::Stateful<gpui::Div> {
    let mut thumb = div()
        .w(px(80.0))
        .h(px(60.0))
        .rounded(px(Radius::XS_SM))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center();
    if let Some(path) = &data.image {
        thumb = thumb.child(
            gpui::img(path.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Cover),
        );
    } else {
        let hue = crate::media_panel_model::tile_hue(&data.id);
        thumb = thumb
            .bg(gpui::Hsla {
                h: hue,
                s: 0.35,
                l: 0.18,
                a: 1.0,
            })
            .text_color(gpui::Hsla {
                h: hue,
                s: 0.60,
                l: 0.65,
                a: 1.0,
            })
            .text_size(px(FontSize::LG))
            .child(crate::media_panel_model::tile_icon(&data.kind).to_string());
    }
    if data.selected {
        thumb = thumb.border_2().border_color(Accent::PRIMARY);
    }
    let name_strip: AnyElement = if let Some(field) = rename_field {
        div()
            .w(px(80.0))
            .pt(px(Spacing::XXS))
            .text_size(px(FontSize::XS))
            .text_color(Text::PRIMARY)
            .child(field)
            .into_any_element()
    } else {
        div()
            .w(px(80.0))
            .pt(px(Spacing::XXS))
            .text_color(if data.selected {
                Text::PRIMARY
            } else {
                Text::SECONDARY
            })
            .text_size(px(FontSize::XS))
            .overflow_hidden()
            .child(data.name.clone())
            .into_any_element()
    };
    div()
        .id(SharedString::from(format!("tile-{}", data.id)))
        .flex()
        .flex_col()
        .w(px(80.0))
        .cursor_pointer()
        .child(thumb)
        .child(name_strip)
}

/// 22×22 toolbar icon button (Swift toolbarMenuIcon).
fn toolbar_icon(id: &str, glyph: &str, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .w(px(IconSize::MD))
        .h(px(IconSize::MD))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .text_color(color)
        .text_size(px(FontSize::SM))
        .child(glyph.to_string())
}

/// Dropdown row with a leading check column; on_click attached by the caller.
fn menu_row(id: SharedString, label: String, checked: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .px(px(Spacing::MD))
        .py(px(Spacing::XS))
        .cursor_pointer()
        .child(
            div()
                .w(px(IconSize::XXS))
                .text_size(px(FontSize::XS))
                .text_color(Accent::PRIMARY)
                .child(if checked { "✓" } else { "" }),
        )
        .child(
            div()
                .text_size(px(FontSize::SM))
                .text_color(Text::SECONDARY)
                .child(label),
        )
}

fn menu_divider() -> gpui::Div {
    div()
        .h(px(BorderWidth::HAIRLINE))
        .mx(px(Spacing::SM))
        .my(px(Spacing::XXS))
        .bg(BorderColors::SUBTLE)
}

fn section_header(title: &str, count: usize) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .child(
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::SECONDARY)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::MUTED)
                .child(count.to_string()),
        )
}

impl MediaPanelView {
    fn asset_tile_data(&self, e: &MediaManifestEntry) -> AssetTileData {
        let source_path = self
            .state
            .items
            .iter()
            .find(|i| i.id == e.id)
            .and_then(|i| i.source_path.clone());
        let image = match e.r#type {
            ClipType::Image => source_path,
            ClipType::Video => source_path
                .as_deref()
                .and_then(crate::video_thumbnails::request_thumbnail),
            _ => None,
        };
        AssetTileData {
            id: e.id.clone(),
            name: e.name.clone(),
            kind: e.r#type,
            image,
            selected: self.library.selection.iter().any(|s| s == &e.id),
        }
    }

    /// Asset tile with selection mouse handling; draggable onto timeline
    /// tracks and generation reference tiles (AssetDrag payload). Right-click
    /// opens the context menu; while renaming inline the selection/drag
    /// handlers are dropped so typing can't fight them.
    fn render_asset_tile(
        &self,
        e: &MediaManifestEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let data = self.asset_tile_data(e);
        let editing = self.asset_editing.as_deref() == Some(e.id.as_str());
        let id = data.id.clone();
        let menu_id = data.id.clone();
        let drag_name = data.name.clone();
        let rename_field = editing.then(|| self.asset_rename_field.clone());
        let tile = asset_tile(&data, rename_field).on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.library_menu.open_at(
                    ev.position.x.as_f32(),
                    ev.position.y.as_f32(),
                    LibraryMenuTarget::Asset { id: menu_id.clone() },
                );
                cx.notify();
            }),
        );
        if editing {
            return tile;
        }
        tile.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.handle_asset_click(&id, ev, cx);
            }),
        )
        .on_drag(
            AssetDrag {
                asset_id: e.id.clone(),
                media_type: e.r#type,
            },
            move |_, _, _, cx| {
                let name = drag_name.clone();
                cx.new(|_| AssetDragPreview { name })
            },
        )
    }

    /// Folder tile: div-drawn folder glyph, count badge, double-click to
    /// open, double-click the name to rename inline.
    fn render_folder_tile(&self, folder: &MediaFolder, cx: &mut Context<Self>) -> AnyElement {
        let id = folder.id.clone();
        let name = folder.name.clone();
        let count = folder_child_count(&self.manifest, &folder.id);
        let editing = self.folder_editing.as_deref() == Some(folder.id.as_str());
        let open_id = id.clone();
        let rename_id = id.clone();
        let rename_seed = name.clone();
        let menu_folder_id = id.clone();
        let menu_folder_name = name.clone();
        let accent = gpui::Hsla {
            a: 0.85,
            ..Accent::PRIMARY
        };
        let name_strip: AnyElement = if editing {
            div()
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_size(px(FontSize::XS))
                .text_color(Text::PRIMARY)
                .child(self.folder_rename_field.clone())
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("folder-name-{id}")))
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::XS))
                .overflow_hidden()
                .child(name.clone())
                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    if e.click_count() == 2 {
                        cx.stop_propagation();
                        this.begin_folder_rename(&rename_id, &rename_seed, window, cx);
                    }
                }))
                .into_any_element()
        };
        div()
            .id(SharedString::from(format!("folder-{id}")))
            .flex()
            .flex_col()
            .w(px(80.0))
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .w(px(80.0))
                    .h(px(60.0))
                    .rounded(px(Radius::XS_SM))
                    .bg(gpui::Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::SUBTLE,
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    // Folder glyph: tab + body, drawn with divs.
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .child(div().w(px(18.0)).h(px(5.0)).rounded(px(Radius::XS)).bg(accent))
                            .child(
                                div()
                                    .w(px(44.0))
                                    .h(px(28.0))
                                    .rounded(px(Radius::XS_SM))
                                    .bg(accent),
                            ),
                    )
                    .when(count > 0, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(Spacing::XS))
                                .right(px(Spacing::XS))
                                .px(px(Spacing::SM))
                                .py(px(Spacing::XXS))
                                .rounded_full()
                                .bg(Background::PROMINENT)
                                .text_size(px(FontSize::XXS))
                                .text_color(Text::PRIMARY)
                                .child(count.to_string()),
                        )
                    }),
            )
            .child(name_strip)
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 && this.folder_editing.is_none() {
                    this.library.current_folder = Some(open_id.clone());
                    this.library.clear_selection();
                    this.open_menu = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.library_menu.open_at(
                        ev.position.x.as_f32(),
                        ev.position.y.as_f32(),
                        LibraryMenuTarget::Folder {
                            id: menu_folder_id.clone(),
                            name: menu_folder_name.clone(),
                        },
                    );
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    /// Wrap grid of folder tiles + asset tiles inside the scroll body.
    fn render_wrap_grid(
        &self,
        folders: &[&MediaFolder],
        entries: &[&MediaManifestEntry],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(Spacing::SM_MD))
            .p(px(Spacing::SM_MD));
        for folder in folders {
            grid = grid.child(self.render_folder_tile(folder, cx));
        }
        for e in entries {
            grid = grid.child(self.render_asset_tile(e, cx));
        }
        grid
    }

    /// Scroll container with background click clearing selection and menus.
    fn grid_scroll(&self, id: &str, content: gpui::Div, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(id.to_string())
            .flex_1()
            .overflow_y_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let renaming = this.folder_editing.is_some() || this.asset_editing.is_some();
                    if renaming {
                        this.commit_folder_rename(cx);
                        this.commit_asset_rename(cx);
                    }
                    if renaming || this.open_menu.is_some() || !this.library.selection.is_empty()
                    {
                        this.open_menu = None;
                        this.library.clear_selection();
                        cx.notify();
                    }
                }),
            )
            .child(content)
            .into_any_element()
    }

    /// Search results: name matches under a "Files" header (moment/transcript
    /// sections need a search-index host, not yet wired on the Rust side).
    fn render_search_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let entries = visible_entries(&self.manifest, &self.library);
        let content = if entries.is_empty() {
            div().p(px(Spacing::SM_MD)).child(
                div()
                    .pt(px(Spacing::XL))
                    .w_full()
                    .flex()
                    .justify_center()
                    .text_size(px(FontSize::SM))
                    .text_color(Text::TERTIARY)
                    .child(format!("No matches for \u{201c}{}\u{201d}", self.library.trimmed_query())),
            )
        } else {
            div()
                .flex()
                .flex_col()
                .child(section_header("Files", entries.len()))
                .child(self.render_wrap_grid(&[], &entries, cx))
        };
        self.grid_scroll("media-grid-scroll", content, cx)
    }

    /// Grouped view: Library section + one per folder, ordered by path.
    fn render_grouped(&self, cx: &mut Context<Self>) -> AnyElement {
        let sections = grouped_sections(&self.manifest, &self.library);
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD))
            .p(px(Spacing::SM_MD));
        for (i, (folder_id, title, entries)) in sections.iter().enumerate() {
            let mut header = div()
                .id(SharedString::from(format!("media-group-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(Text::PRIMARY)
                        .child(title.clone()),
                )
                .child(
                    div()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::MUTED)
                        .child(entries.len().to_string()),
                );
            if let Some(fid) = folder_id {
                let open_id = fid.to_string();
                header = header.cursor_pointer().on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.library.view_mode = LibraryViewMode::Folders;
                        this.library.current_folder = Some(open_id.clone());
                        this.library.clear_selection();
                        cx.notify();
                    },
                ));
            }
            let body: AnyElement = if entries.is_empty() {
                div()
                    .py(px(Spacing::SM))
                    .text_size(px(FontSize::XS))
                    .text_color(Text::MUTED)
                    .child("Empty")
                    .into_any_element()
            } else {
                self.render_wrap_grid(&[], entries, cx).into_any_element()
            };
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(header)
                    .child(
                        div()
                            .h(px(BorderWidth::HAIRLINE))
                            .bg(BorderColors::SUBTLE),
                    )
                    .child(body),
            );
        }
        self.grid_scroll("media-grid-scroll", col, cx)
    }

    /// The grid body for the current state.
    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let lib_empty = self.manifest.entries.is_empty() && self.manifest.folders.is_empty();
        if lib_empty {
            return media_empty_state().into_any_element();
        }
        if self.library.search_active() {
            return self.render_search_results(cx);
        }
        match self.library.view_mode {
            LibraryViewMode::Folders => {
                let folders = visible_folders(&self.manifest, &self.library);
                let entries = visible_entries(&self.manifest, &self.library);
                let grid = self.render_wrap_grid(&folders, &entries, cx);
                self.grid_scroll("media-grid-scroll", grid, cx)
            }
            LibraryViewMode::Flat => {
                let entries = visible_entries(&self.manifest, &self.library);
                let grid = self.render_wrap_grid(&[], &entries, cx);
                self.grid_scroll("media-grid-scroll", grid, cx)
            }
            LibraryViewMode::Grouped => self.render_grouped(cx),
        }
    }

    /// Toolbar: actions row, search row, context bar (Swift MediaTab.toolbar).
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_filters = self.library.has_active_filters();
        let clear_visible = !self.library.search_query.is_empty();
        let sel_count = self.library.selection.len();
        let item_count = visible_folders(&self.manifest, &self.library).len()
            + visible_entries(&self.manifest, &self.library).len();
        let in_folder_crumbs =
            self.library.view_mode == LibraryViewMode::Folders && !self.library.search_active();

        // Context path: breadcrumb chips in Folders view, mode title otherwise.
        let context_path: AnyElement = if in_folder_crumbs {
            let mut crumbs: Vec<(Option<String>, String)> = vec![(None, "Library".to_string())];
            for f in folder_path(&self.manifest, self.library.current_folder.as_deref()) {
                crumbs.push((Some(f.id.clone()), f.name.clone()));
            }
            let last = crumbs.len() - 1;
            let mut row = div().flex().flex_row().items_center().gap(px(Spacing::XS));
            for (i, (target, label)) in crumbs.into_iter().enumerate() {
                if i > 0 {
                    row = row.child(
                        div()
                            .text_size(px(FontSize::XXS))
                            .text_color(Text::MUTED)
                            .child("›"),
                    );
                }
                let is_leaf = i == last;
                let mut chip = div()
                    .id(SharedString::from(format!("media-crumb-{i}")))
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XXS))
                    .rounded(px(Radius::XS_SM))
                    .text_size(px(FontSize::XS))
                    .text_color(if is_leaf {
                        Text::PRIMARY
                    } else {
                        Text::TERTIARY
                    })
                    .child(label);
                if !is_leaf {
                    chip = chip.cursor_pointer().on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.library.current_folder = target.clone();
                            this.library.clear_selection();
                            cx.notify();
                        },
                    ));
                }
                row = row.child(chip);
            }
            row.into_any_element()
        } else {
            let title = if self.library.search_active() {
                "Search".to_string()
            } else {
                self.library.view_mode.title().to_string()
            };
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::PRIMARY)
                .child(title)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .px(px(Spacing::SM))
            .pt(px(Spacing::SM))
            .pb(px(Spacing::XS))
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::SUBTLE)
            // Actions row: Import + Generate + overflow ⋯ | index status
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .child(
                        div()
                            .id("btn-import-media")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .h(px(IconSize::MD_LG))
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child("+ Import"),
                    )
                    .child(
                        div()
                            .id("btn-generate-media")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .h(px(IconSize::MD_LG))
                            .rounded(px(Radius::SM))
                            .bg(Accent::PRIMARY)
                            .cursor_pointer()
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::SM))
                            .child("✦ Generate"),
                    )
                    .child(
                        toolbar_icon("btn-media-overflow", "⋯", Text::TERTIARY).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::Overflow) => None,
                                    _ => Some(ToolbarMenu::Overflow),
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(div().flex_1())
                    .when(!self.search_status.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(FontSize::XS))
                                .text_color(Text::TERTIARY)
                                .child(self.search_status.clone()),
                        )
                    }),
            )
            // Search row: live field + view/sort/filter menu buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .flex_1()
                            .px(px(Spacing::SM_MD))
                            .h(px(IconSize::MD))
                            .rounded_full()
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .bg(gpui::Hsla {
                                h: 0.0,
                                s: 0.0,
                                l: 1.0,
                                a: Opacity::SUBTLE,
                            })
                            .text_size(px(FontSize::XS))
                            .text_color(Text::PRIMARY)
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child("⌕"),
                            )
                            .child(div().flex_1().child(self.search_field.clone()))
                            .when(clear_visible, |el| {
                                el.child(
                                    div()
                                        .id("media-search-clear")
                                        .cursor_pointer()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::MUTED)
                                        .child("✕")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.search_field.update(cx, |field, cx| {
                                                field.set_text("", cx);
                                            });
                                            this.library.search_query.clear();
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(
                        toolbar_icon("btn-media-view", "⊞", Text::TERTIARY).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::View) => None,
                                    _ => Some(ToolbarMenu::View),
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        toolbar_icon("btn-media-sort", "↕", Text::TERTIARY).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::Sort) => None,
                                    _ => Some(ToolbarMenu::Sort),
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        toolbar_icon(
                            "btn-media-filter",
                            "≡",
                            if has_filters {
                                Accent::PRIMARY
                            } else {
                                Text::TERTIARY
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_menu = match this.open_menu {
                                Some(ToolbarMenu::Filter) => None,
                                _ => Some(ToolbarMenu::Filter),
                            };
                            cx.notify();
                        })),
                    ),
            )
            // Context bar: breadcrumb/mode title | Delete (n) + item count
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(MediaPanel::CONTEXT_ROW_HEIGHT))
                    .child(div().flex_1().child(context_path))
                    .when(sel_count > 0, |el| {
                        el.child(
                            div()
                                .id("media-delete-selected")
                                .px(px(Spacing::SM))
                                .py(px(Spacing::XXS))
                                .rounded(px(Radius::SM))
                                .bg(Background::PROMINENT)
                                .cursor_pointer()
                                .text_size(px(FontSize::XS))
                                .text_color(Text::PRIMARY)
                                .child(format!("Delete ({sel_count})"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.delete_selection(cx);
                                })),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(Text::MUTED)
                            .child(if item_count == 1 {
                                "1 item".to_string()
                            } else {
                                format!("{item_count} items")
                            }),
                    ),
            )
    }

    /// Open dropdown, absolutely positioned over the grid (painted last so it
    /// stacks above the body).
    fn render_menu_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.open_menu?;
        let actions_bottom = Spacing::SM + Layout::PANEL_HEADER_HEIGHT + Spacing::XXS;
        let search_bottom =
            Spacing::SM + Layout::PANEL_HEADER_HEIGHT * 2.0 + Spacing::XS + Spacing::XXS;
        let mut panel = div()
            .id("media-menu")
            .absolute()
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .rounded(px(Radius::SM))
            .flex()
            .flex_col()
            .py(px(Spacing::XS));
        panel = match menu {
            ToolbarMenu::Overflow => panel.top(px(actions_bottom)).left(px(Spacing::SM)),
            _ => panel.top(px(search_bottom)).right(px(Spacing::SM)),
        };
        panel = match menu {
            ToolbarMenu::View => {
                let mut p = panel;
                for (i, mode) in LibraryViewMode::all().into_iter().enumerate() {
                    let checked = self.library.view_mode == mode;
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-view-{i}")),
                            mode.title().to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.view_mode = mode;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p
            }
            ToolbarMenu::Sort => {
                let mut p = panel;
                for (i, key) in LibrarySortKey::all().into_iter().enumerate() {
                    let checked = self.library.sort_key == key;
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-sort-{i}")),
                            key.title().to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.sort_key = key;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p
            }
            ToolbarMenu::Filter => {
                let mut p = panel;
                // Only types a MediaAsset can carry (Swift filterableTypes).
                for (i, (t, label)) in [
                    (ClipType::Video, "Video"),
                    (ClipType::Audio, "Audio"),
                    (ClipType::Image, "Image"),
                ]
                .into_iter()
                .enumerate()
                {
                    let checked = self.library.type_filter.contains(&t);
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-filter-{i}")),
                            label.to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.toggle_type_filter(t);
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p.child(menu_divider())
                    .child(
                        menu_row(
                            SharedString::from("media-filter-ai"),
                            "AI Generated".to_string(),
                            self.library.filter_ai,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library.filter_ai = !this.library.filter_ai;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    )
                    .child(menu_divider())
                    .child(
                        menu_row(
                            SharedString::from("media-filter-clear"),
                            "Clear Filters".to_string(),
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library.clear_filters();
                            this.open_menu = None;
                            cx.notify();
                        })),
                    )
            }
            ToolbarMenu::Overflow => panel.child(
                menu_row(
                    SharedString::from("media-new-folder"),
                    "New Folder".to_string(),
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_menu = None;
                    this.create_folder_in_current(window, cx);
                })),
            ),
        };
        // Occlude + deferred + outside-click dismiss (context_menu.rs
        // pattern): the menu owns its hit area so a click on it can't bleed
        // into the grid, and the first outside click only dismisses (M1).
        let panel = panel
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.open_menu = None;
                cx.notify();
            }));
        Some(deferred(panel).with_priority(1).into_any_element())
    }

    /// The whole Media tab: toolbar + body + generation strip + menu overlay.
    fn render_media_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("media-tab-root")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .on_key_down(cx.listener(Self::handle_key_down))
            // External file drop → the same import flow as the menu (Swift
            // MediaPanelDropArea). Unknown extensions are skipped with a log.
            .on_drop::<ExternalPaths>(cx.listener(|_, paths: &ExternalPaths, _, cx| {
                crate::media_import::import_files_into_shared_state(paths.paths());
                cx.notify();
            }))
            .drag_over::<ExternalPaths>(|style, _, _, _| {
                style
                    .rounded(px(Radius::MD))
                    .border_2()
                    .border_color(DropZone::BORDER)
                    .bg(DropZone::FILL)
            })
            .child(self.render_toolbar(cx))
            .child(self.render_body(cx))
            // GenerationView anchored to BOTTOM with padding (Swift:
            // .padding(.horizontal, sm).padding(.bottom, sm))
            .child(
                div()
                    .px(px(Spacing::SM))
                    .pb(px(Spacing::SM))
                    .child(self.generation.clone()),
            )
            .children(self.render_menu_overlay(cx))
            // Asset/folder tile context menu (deferred popover, above all)
            .when_some(self.library_menu.open_menu().cloned(), |el, open| {
                let entries = self.library_menu_entries(&open.target);
                el.child(crate::context_menu::render_context_menu(
                    gpui::point(px(open.x), px(open.y)),
                    entries,
                    open.confirming,
                    cx,
                    |this: &mut MediaPanelView, index, window, cx| {
                        this.activate_library_menu_item(index, window, cx)
                    },
                    |this: &mut MediaPanelView, _window, cx| {
                        this.library_menu.close();
                        cx.notify();
                    },
                ))
            })
            .into_any_element()
    }
}

fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XXS))
        .child(text.to_uppercase())
}

/// Label + right-aligned control row (Swift InspectorRow).
fn tab_row(label: &str, control: AnyElement) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(Spacing::SM))
        .min_h(px(Layout::PANEL_HEADER_HEIGHT))
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(control)
}

/// Menu value button (Swift menuValueLabel): "value ↕".
fn menu_value_button(id: &str, value: String) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XXS))
        .cursor_pointer()
        .text_color(Text::TERTIARY)
        .text_size(px(FontSize::SM))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(value)
        .child(
            div()
                .text_size(px(FontSize::XXS))
                .child("↕"),
        )
}

/// Inline dropdown panel below a row. Mouse-downs stay inside so the scroll
/// container's click-away close can't eat the row click.
fn tab_menu_panel() -> gpui::Div {
    div()
        .bg(Background::RAISED)
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .rounded(px(Radius::SM))
        .flex()
        .flex_col()
        .py(px(Spacing::XS))
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
}

/// Mini toggle switch (Swift .toggleStyle(.switch).controlSize(.mini)).
fn toggle_switch(id: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    let knob = IconSize::XS - Spacing::XS;
    let track = if on {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: Opacity::STRONG,
        }
    } else {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: Opacity::MUTED,
        }
    };
    div()
        .id(id.to_string())
        .w(px(IconSize::LG))
        .h(px(IconSize::XS))
        .p(px(Spacing::XXS))
        .rounded_full()
        .cursor_pointer()
        .bg(track)
        .flex()
        .items_center()
        .when(on, |el| el.justify_end())
        .child(
            div()
                .w(px(knob))
                .h(px(knob))
                .rounded_full()
                .bg(Background::BASE),
        )
}

/// Note line above the generate bar buttons (Swift error-red note).
fn generate_note(note: &str) -> gpui::Div {
    div()
        .text_color(Status::ERROR)
        .text_size(px(FontSize::XS))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(note.to_string())
}

/// Full-tab overlay shown while a real run is in flight (Swift GeneratingOverlay).
fn generating_overlay(label: &str) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(Hsla {
            a: Opacity::PROMINENT,
            ..Background::SURFACE
        })
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
}

impl MediaPanelView {
    // ── Shared tab plumbing ─────────────────────────────────────────

    fn tab_scrub_start(&self, field: TabScrubField) -> f64 {
        match field {
            TabScrubField::CaptionSize => self.captions.font_size,
            TabScrubField::CaptionX => self.captions.center_x,
            TabScrubField::CaptionY => self.captions.center_y,
            TabScrubField::MusicDuration => self.music.text_duration,
        }
    }

    fn set_tab_scrub_value(&mut self, field: TabScrubField, value: f64) {
        match field {
            TabScrubField::CaptionSize => self.captions.font_size = value,
            TabScrubField::CaptionX => self.captions.center_x = value,
            TabScrubField::CaptionY => self.captions.center_y = value,
            TabScrubField::MusicDuration => self.music.text_duration = value,
        }
    }

    /// Accent-colored scrubbable value (inspector scrub_row pattern).
    fn scrub_value_el(
        &self,
        id: &str,
        field: TabScrubField,
        display: String,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id.to_string())
            .cursor_pointer()
            .text_color(Accent::PRIMARY)
            .text_size(px(FontSize::SM))
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(display)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, _| {
                    this.tab_scrub = Some(TabScrubSession {
                        field,
                        start_x: event.position.x.as_f32(),
                        start_value: this.tab_scrub_start(field),
                    });
                }),
            )
            .on_drag(TabScrub, |_, _offset, _window, cx: &mut App| {
                cx.new(|_| TabScrubPreview)
            })
    }

    /// Route an Agent Mode task to the chat panel (Swift handoff); when the
    /// seam isn't installed the tab notes it instead.
    fn caption_agent_task(&mut self, task: &str, cx: &mut Context<Self>) {
        self.captions_menu = None;
        self.captions.note = send_agent_handoff(&caption_agent_prompt(task));
        cx.notify();
    }

    fn music_agent_task(&mut self, prompt: &str, cx: &mut Context<Self>) {
        self.music_menu = None;
        self.music.note = send_agent_handoff(prompt);
        cx.notify();
    }

    // ── Captions tab ────────────────────────────────────────────────

    /// Swift CaptionTab.generate: resolve sources, run add_captions, surface
    /// why when nothing can run. `is_generating` only turns on for a queued
    /// real run — never for an unavailable/failed one.
    fn generate_captions(&mut self, cx: &mut Context<Self>) {
        self.captions.note = None;
        self.captions_menu = None;
        let ids = caption_source_ids(&self.timeline, self.captions.selected_track_id.as_deref());
        if ids.is_empty() {
            if self.captions.selected_track_id.is_some() {
                self.captions.note = Some("No audio selected.".to_string());
            }
            cx.notify();
            return;
        }
        let mut args = serde_json::json!({ "clipIds": ids });
        if let Some(lang) = &self.captions.language {
            args["language"] = serde_json::Value::String(lang.clone());
        }
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let result = match executor.lock() {
            Ok(mut exec) => exec.execute("add_captions", &args),
            Err(_) => Err("Editor state lock poisoned".to_string()),
        };
        self.captions.note = caption_generate_note(&result);
        cx.notify();
    }

    /// Caption preview box: canvas-aspect frame, centre guides, sample chip
    /// at the configured centre (Swift previewBox + centerGuides).
    fn render_caption_preview(&self) -> gpui::Div {
        let aspect = self.timeline.width.max(1) as f32 / self.timeline.height.max(1) as f32;
        let box_h = ComponentSize::CAPTION_PREVIEW_MAX_HEIGHT;
        let box_w = box_h * aspect;
        let scale = box_h as f64 / CaptionTheme::REFERENCE_CANVAS_HEIGHT;
        let font_px = (self.captions.font_size * scale).max(1.0) as f32;
        let x = self.captions.center_x;
        let y = self.captions.center_y;
        let guide = Hsla {
            a: Opacity::PROMINENT,
            ..Accent::TIMECODE
        };
        let mut chip = div()
            .relative()
            .left(px(((x - 0.5) * box_w as f64) as f32))
            .top(px(((y - 0.5) * box_h as f64) as f32))
            .px(px(Spacing::XS))
            .py(px(Spacing::XXS))
            .text_color(gpui::rgb(self.captions.color))
            .text_size(px(font_px))
            .font_family(self.captions.font_name.clone())
            .child(CAPTION_PREVIEW_TEXT);
        if self.captions.background_enabled {
            chip = chip.bg(gpui::rgb(self.captions.background_color));
        }
        div().w_full().flex().justify_center().child(
            div()
                .relative()
                .w(px(box_w))
                .h(px(box_h))
                .max_w_full()
                .bg(gpui::black())
                .rounded(px(Radius::SM))
                .border_1()
                .border_color(BorderColors::SUBTLE)
                .overflow_hidden()
                .when(x == CaptionTheme::CENTER_SNAP_VALUE, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(box_w / 2.0))
                            .w(px(BorderWidth::HAIRLINE))
                            .h_full()
                            .bg(guide),
                    )
                })
                .when(y == CaptionTheme::CENTER_SNAP_VALUE, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(box_h / 2.0))
                            .h(px(BorderWidth::HAIRLINE))
                            .w_full()
                            .bg(guide),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(chip),
                ),
        )
    }

    /// Color swatch strip; `background` picks which state field it drives.
    fn swatch_strip(&self, background: bool, cx: &mut Context<Self>) -> gpui::Div {
        let selected = if background {
            self.captions.background_color
        } else {
            self.captions.color
        };
        let dimmed = background && !self.captions.background_enabled;
        let mut strip = div().flex().flex_row().items_center().gap(px(Spacing::XS));
        if dimmed {
            strip = strip.opacity(Opacity::MEDIUM);
        }
        for (name, hex) in CAPTION_SWATCHES {
            let mut swatch = div()
                .id(SharedString::from(format!(
                    "caption-swatch-{}-{name}",
                    if background { "bg" } else { "fg" }
                )))
                .w(px(IconSize::XS))
                .h(px(IconSize::XS))
                .rounded(px(Radius::XS))
                .bg(gpui::rgb(hex))
                .border_1()
                .border_color(if selected == hex {
                    Accent::PRIMARY
                } else {
                    BorderColors::SUBTLE
                });
            if !dimmed {
                swatch = swatch.cursor_pointer().on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        if background {
                            this.captions.background_color = hex;
                        } else {
                            this.captions.color = hex;
                        }
                        cx.notify();
                    },
                ));
            }
            strip = strip.child(swatch);
        }
        strip
    }

    /// The captions Agent Mode dropdown (tasks + translate submenu).
    fn render_captions_agent_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut panel = tab_menu_panel()
            .absolute()
            .bottom(px(Spacing::MD * 2.0 + GENERATE_BUTTON_HEIGHT))
            .right(px(Spacing::LG_XL));
        if self.captions_menu == Some(CaptionsMenu::AgentTranslate) {
            panel = panel.child(
                menu_row(
                    SharedString::from("captions-agent-back"),
                    "‹ Translate".to_string(),
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.captions_menu = Some(CaptionsMenu::Agent);
                    cx.notify();
                })),
            );
            panel = panel.child(menu_divider());
            for language in CAPTION_TRANSLATE_LANGUAGES {
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("captions-translate-{language}")),
                        language.to_string(),
                        false,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.caption_agent_task(&caption_translate_task(language), cx);
                    })),
                );
            }
        } else {
            for (label, task) in CAPTION_AGENT_TASKS {
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("captions-agent-{label}")),
                        label.to_string(),
                        false,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.caption_agent_task(task, cx);
                    })),
                );
            }
            panel = panel.child(
                menu_row(
                    SharedString::from("captions-agent-translate"),
                    "Translate ›".to_string(),
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.captions_menu = Some(CaptionsMenu::AgentTranslate);
                    cx.notify();
                })),
            );
        }
        panel
    }

    /// Agent Mode pill (Swift aiGradient approximated with the accent color).
    fn agent_mode_button(&self, id: &str, captions: bool, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(id.to_string())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::XS))
            .px(px(Spacing::MD_LG))
            .py(px(Spacing::SM_MD))
            .rounded(px(Radius::SM))
            .bg(Background::RAISED)
            .border_1()
            .border_color(Hsla {
                a: Opacity::MEDIUM,
                ..Accent::PRIMARY
            })
            .cursor_pointer()
            .text_color(Accent::PRIMARY)
            .text_size(px(FontSize::SM))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child("Agent Mode")
            .child(div().text_size(px(FontSize::XS)).child("⌄"))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if captions {
                    this.captions_menu = match this.captions_menu {
                        Some(CaptionsMenu::Agent) | Some(CaptionsMenu::AgentTranslate) => None,
                        _ => Some(CaptionsMenu::Agent),
                    };
                } else {
                    this.music_menu = match this.music_menu {
                        Some(MusicMenu::Agent) | Some(MusicMenu::AgentMood) => None,
                        _ => Some(MusicMenu::Agent),
                    };
                }
                cx.notify();
            }))
            .into_any_element()
    }

    /// The whole Captions tab (Swift CaptionTab).
    fn render_captions_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let tl = &self.timeline;
        let source_summary = caption_source_summary(tl, self.captions.selected_track_id.as_deref());
        let effective_count =
            caption_effective_count(tl, self.captions.selected_track_id.as_deref());
        let track_entries = caption_track_entries(tl);
        let auto_summary = caption_source_summary(tl, None);
        let language_label = self
            .captions
            .language
            .as_deref()
            .and_then(|tag| {
                CAPTION_LANGUAGES
                    .iter()
                    .find(|(t, _)| *t == tag)
                    .map(|(_, name)| name.to_string())
            })
            .or_else(|| self.captions.language.clone())
            .unwrap_or_else(|| "Auto".to_string());
        let disabled = effective_count == 0 || self.captions.is_generating;

        // ── Source section ──
        let mut source_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Source"))
            .child(tab_row(
                "Source",
                menu_value_button("captions-source-btn", source_summary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions_menu = match this.captions_menu {
                            Some(CaptionsMenu::Source) => None,
                            _ => Some(CaptionsMenu::Source),
                        };
                        cx.notify();
                    }))
                    .into_any_element(),
            ));
        if self.captions_menu == Some(CaptionsMenu::Source) {
            let mut panel = tab_menu_panel().child(
                menu_row(
                    SharedString::from("captions-source-auto"),
                    auto_summary,
                    self.captions.selected_track_id.is_none(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.captions.selected_track_id = None;
                    this.captions_menu = None;
                    cx.notify();
                })),
            );
            panel = panel.child(menu_divider());
            if track_entries.is_empty() {
                panel = panel.child(
                    div()
                        .px(px(Spacing::MD))
                        .py(px(Spacing::XS))
                        .text_size(px(FontSize::SM))
                        .text_color(Text::MUTED)
                        .child("No Tracks"),
                );
            } else {
                for (i, (track_id, label, count)) in track_entries.iter().enumerate() {
                    let checked = self.captions.selected_track_id.as_deref()
                        == Some(track_id.as_str());
                    let id = track_id.clone();
                    panel = panel.child(
                        menu_row(
                            SharedString::from(format!("captions-source-{i}")),
                            format!(
                                "{label} · {count} {}",
                                if *count == 1 { "clip" } else { "clips" }
                            ),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.captions.selected_track_id = Some(id.clone());
                            this.captions_menu = None;
                            cx.notify();
                        })),
                    );
                }
            }
            source_section = source_section.child(panel);
        }
        source_section = source_section.child(tab_row(
            "Language",
            menu_value_button("captions-language-btn", language_label)
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.captions_menu = match this.captions_menu {
                        Some(CaptionsMenu::Language) => None,
                        _ => Some(CaptionsMenu::Language),
                    };
                    cx.notify();
                }))
                .into_any_element(),
        ));
        if self.captions_menu == Some(CaptionsMenu::Language) {
            let mut panel = tab_menu_panel().child(
                menu_row(
                    SharedString::from("captions-lang-auto"),
                    "Auto".to_string(),
                    self.captions.language.is_none(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.captions.language = None;
                    this.captions.language_touched = true;
                    this.captions_menu = None;
                    cx.notify();
                })),
            );
            panel = panel.child(menu_divider());
            for (tag, name) in CAPTION_LANGUAGES {
                let checked = self.captions.language.as_deref() == Some(tag);
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("captions-lang-{tag}")),
                        name.to_string(),
                        checked,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions.language = Some(tag.to_string());
                        this.captions.language_touched = true;
                        this.captions_menu = None;
                        cx.notify();
                    })),
                );
            }
            source_section = source_section.child(panel);
        }

        // ── Style section ──
        let mut style_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Style"))
            .child(tab_row(
                "Font",
                menu_value_button("captions-font-btn", self.captions.font_name.clone())
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions_menu = match this.captions_menu {
                            Some(CaptionsMenu::Font) => None,
                            _ => Some(CaptionsMenu::Font),
                        };
                        cx.notify();
                    }))
                    .into_any_element(),
            ));
        if self.captions_menu == Some(CaptionsMenu::Font) {
            let mut panel = tab_menu_panel().child(
                div()
                    .px(px(Spacing::MD))
                    .py(px(Spacing::XS))
                    .text_size(px(FontSize::XXS))
                    .text_color(Text::MUTED)
                    .child("FEATURED"),
            );
            for family in BUNDLED_FONT_FAMILIES {
                let checked = self.captions.font_name == family;
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("captions-font-{family}")),
                        family.to_string(),
                        checked,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions.font_name = family.to_string();
                        this.captions_menu = None;
                        cx.notify();
                    })),
                );
            }
            style_section = style_section.child(panel);
        }
        style_section = style_section
            .child(tab_row(
                "Size",
                self.scrub_value_el(
                    "captions-size",
                    TabScrubField::CaptionSize,
                    format!("{:.0} pt", self.captions.font_size),
                    cx,
                )
                .into_any_element(),
            ))
            .child(tab_row("Color", self.swatch_strip(false, cx).into_any_element()))
            .child(tab_row(
                "Background",
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .child(self.swatch_strip(true, cx))
                    .child(
                        toggle_switch("captions-bg-toggle", self.captions.background_enabled)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.captions.background_enabled =
                                    !this.captions.background_enabled;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            ))
            .child(tab_row(
                "Case",
                menu_value_button("captions-case-btn", self.captions.text_case.label().to_string())
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions_menu = match this.captions_menu {
                            Some(CaptionsMenu::Case) => None,
                            _ => Some(CaptionsMenu::Case),
                        };
                        cx.notify();
                    }))
                    .into_any_element(),
            ));
        if self.captions_menu == Some(CaptionsMenu::Case) {
            let mut panel = tab_menu_panel();
            for case in CaptionCase::all() {
                let checked = self.captions.text_case == case;
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("captions-case-{}", case.config_value())),
                        case.label().to_string(),
                        checked,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.captions.text_case = case;
                        this.captions_menu = None;
                        cx.notify();
                    })),
                );
            }
            style_section = style_section.child(panel);
        }
        style_section = style_section.child(tab_row(
            "Censor profanity",
            toggle_switch("captions-censor-toggle", self.captions.censor_profanity)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    this.captions.censor_profanity = !this.captions.censor_profanity;
                    cx.notify();
                }))
                .into_any_element(),
        ));

        // ── Placement section ──
        let placement_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Placement"))
            .child(self.render_caption_preview())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(Spacing::MD_LG))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XXS))
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("X"),
                            )
                            .child(self.scrub_value_el(
                                "captions-pos-x",
                                TabScrubField::CaptionX,
                                format!("{:.0}%", self.captions.center_x * 100.0),
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XXS))
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Y"),
                            )
                            .child(self.scrub_value_el(
                                "captions-pos-y",
                                TabScrubField::CaptionY,
                                format!("{:.0}%", self.captions.center_y * 100.0),
                                cx,
                            )),
                    ),
            );

        // ── Generate bar ──
        let mut bar = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::SM))
            .px(px(Spacing::LG_XL))
            .py(px(Spacing::MD))
            .border_t_1()
            .border_color(BorderColors::SUBTLE);
        if let Some(note) = &self.captions.note {
            bar = bar.child(generate_note(note));
        }
        bar = bar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::SM))
                .child(
                    div()
                        .id("btn-gen-captions")
                        .flex_1()
                        .h(px(GENERATE_BUTTON_HEIGHT))
                        .rounded(px(Radius::SM))
                        .bg(Accent::PRIMARY)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(Background::BASE)
                        .text_size(px(FontSize::SM))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .opacity(if disabled {
                            Opacity::MEDIUM
                        } else {
                            Opacity::OPAQUE
                        })
                        .child("Generate Captions")
                        .when(!disabled, |el| {
                            el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.generate_captions(cx);
                            }))
                        }),
                )
                .child(self.agent_mode_button("btn-captions-agent", true, cx)),
        );

        div()
            .id("captions-tab-root")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" && this.captions_menu.take().is_some()
                {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .id("captions-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(Spacing::LG_XL))
                    .pt(px(Spacing::MD))
                    .pb(px(Spacing::MD))
                    .gap(px(Spacing::MD_LG))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.captions_menu.take().is_some() {
                                cx.notify();
                            }
                        }),
                    )
                    .child(source_section)
                    .child(style_section)
                    .child(placement_section),
            )
            .child(bar)
            .when(
                matches!(
                    self.captions_menu,
                    Some(CaptionsMenu::Agent) | Some(CaptionsMenu::AgentTranslate)
                ),
                |el| el.child(self.render_captions_agent_menu(cx)),
            )
            .when(self.captions.is_generating, |el| {
                el.child(generating_overlay("Transcribing…"))
            })
            .into_any_element()
    }

    // ── Music tab ───────────────────────────────────────────────────

    /// Swift MusicTab.generate: validate, run generate_music, surface the
    /// outcome. The overlay only turns on for a genuinely queued job.
    fn generate_music(&mut self, cx: &mut Context<Self>) {
        self.music.note = None;
        self.music_menu = None;
        let model = music_model_for(self.music.selected_model_id.as_deref());
        let text_mode = effective_music_mode(self.music.mode, model) == MusicMode::TextToMusic;
        let fps = self.timeline.fps.max(1);
        let span_seconds = self.timeline.total_frames() as f64 / fps as f64;
        let duration = if text_mode {
            self.music.text_duration
        } else {
            span_seconds
        };
        let cost = model.and_then(|m| music_cost(m, &self.music.prompt, duration));
        let credits = self.generation.read(cx).state.credits_remaining;
        if music_validation_note(model, text_mode, &self.music.prompt, span_seconds, cost, credits)
            .is_some()
        {
            cx.notify();
            return;
        }
        let args = serde_json::json!({
            "prompt": self.music.prompt.trim(),
            "duration": duration.round(),
        });
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let result = match executor.lock() {
            Ok(mut exec) => exec.execute("generate_music", &args),
            Err(_) => Err("Editor state lock poisoned".to_string()),
        };
        self.music.note = music_generate_note(&result);
        if self.music.note.is_none() {
            self.music.is_generating = true;
        }
        cx.notify();
    }

    /// The music Agent Mode dropdown (timeline task + mood submenu).
    fn render_music_agent_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut panel = tab_menu_panel()
            .absolute()
            .bottom(px(Spacing::MD * 2.0 + GENERATE_BUTTON_HEIGHT))
            .right(px(Spacing::LG_XL));
        if self.music_menu == Some(MusicMenu::AgentMood) {
            panel = panel.child(
                menu_row(
                    SharedString::from("music-agent-back"),
                    "‹ Mood".to_string(),
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    cx.stop_propagation();
                    this.music_menu = Some(MusicMenu::Agent);
                    cx.notify();
                })),
            );
            panel = panel.child(menu_divider());
            for mood in MUSIC_MOODS {
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("music-mood-{mood}")),
                        mood.to_string(),
                        false,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.music_agent_task(&music_mood_prompt(mood), cx);
                    })),
                );
            }
        } else {
            panel = panel
                .child(
                    menu_row(
                        SharedString::from("music-agent-timeline"),
                        "Generate music for the timeline".to_string(),
                        false,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.music_agent_task(MUSIC_TIMELINE_PROMPT, cx);
                    })),
                )
                .child(
                    menu_row(
                        SharedString::from("music-agent-mood"),
                        "Mood ›".to_string(),
                        false,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.music_menu = Some(MusicMenu::AgentMood);
                        cx.notify();
                    })),
                );
        }
        panel
    }

    /// The whole Music tab (Swift MusicTab).
    fn render_music_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = music_model_for(self.music.selected_model_id.as_deref());
        let text_mode = effective_music_mode(self.music.mode, model) == MusicMode::TextToMusic;
        let fps = self.timeline.fps.max(1);
        let total_frames = self.timeline.total_frames();
        let span_seconds = total_frames as f64 / fps as f64;
        let duration = if text_mode {
            self.music.text_duration
        } else {
            span_seconds
        };
        let cost = model.and_then(|m| music_cost(m, &self.music.prompt, duration));
        let credits = self.generation.read(cx).state.credits_remaining;
        let validation = music_validation_note(
            model,
            text_mode,
            &self.music.prompt,
            span_seconds,
            cost,
            credits,
        );
        let note = self.music.note.clone().or_else(|| validation.clone());
        let can_generate = model.is_some() && validation.is_none() && !self.music.is_generating;
        let mode_label = effective_music_mode(self.music.mode, model).label().to_string();
        let model_label = model
            .map(|m| m.display_name.to_string())
            .unwrap_or_else(|| "None".to_string());

        // ── Source section ──
        let mut source_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Source"));
        if model.is_some_and(music_supports_text_mode) {
            source_section = source_section.child(tab_row(
                "Input",
                menu_value_button("music-input-btn", mode_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.music_menu = match this.music_menu {
                            Some(MusicMenu::Input) => None,
                            _ => Some(MusicMenu::Input),
                        };
                        cx.notify();
                    }))
                    .into_any_element(),
            ));
            if self.music_menu == Some(MusicMenu::Input) {
                let mut panel = tab_menu_panel();
                for mode in [MusicMode::VideoToMusic, MusicMode::TextToMusic] {
                    let checked = effective_music_mode(self.music.mode, model) == mode;
                    panel = panel.child(
                        menu_row(
                            SharedString::from(format!("music-mode-{}", mode.label())),
                            mode.label().to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.music.mode = mode;
                            this.music_menu = None;
                            cx.notify();
                        })),
                    );
                }
                source_section = source_section.child(panel);
            }
        }
        if text_mode {
            source_section = source_section.child(tab_row(
                "Duration",
                self.scrub_value_el(
                    "music-duration",
                    TabScrubField::MusicDuration,
                    format!("{:.0} s", self.music.text_duration),
                    cx,
                )
                .into_any_element(),
            ));
        } else {
            source_section = source_section.child(tab_row(
                "Video",
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::SM))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(music_source_summary(total_frames, fps))
                    .into_any_element(),
            ));
        }

        // ── Model section ──
        let mut model_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Model"))
            .child(tab_row(
                "Model",
                menu_value_button("music-model-btn", model_label)
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.music_menu = match this.music_menu {
                            Some(MusicMenu::Model) => None,
                            _ => Some(MusicMenu::Model),
                        };
                        cx.notify();
                    }))
                    .into_any_element(),
            ));
        if self.music_menu == Some(MusicMenu::Model) {
            let mut panel = tab_menu_panel();
            for m in music_models() {
                let checked = model.is_some_and(|sel| sel.id == m.id);
                let model_id = m.id;
                panel = panel.child(
                    menu_row(
                        SharedString::from(format!("music-model-{}", m.id)),
                        m.display_name.to_string(),
                        checked,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.music.selected_model_id = Some(model_id.to_string());
                        this.music_menu = None;
                        cx.notify();
                    })),
                );
            }
            model_section = model_section.child(panel);
        }

        // ── Prompt section ──
        let prompt_section = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(section_label("Prompt"))
            .child(
                div()
                    .rounded(px(Radius::SM))
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::RAISED)
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::SM))
                    .child(self.music_prompt_area.clone()),
            );

        // ── Generate bar ──
        let mut bar = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::SM))
            .px(px(Spacing::LG_XL))
            .py(px(Spacing::MD))
            .border_t_1()
            .border_color(BorderColors::SUBTLE);
        if let Some(note) = &note {
            bar = bar.child(generate_note(note));
        }
        bar = bar.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::SM))
                .child(
                    div()
                        .id("btn-gen-music")
                        .flex_1()
                        .h(px(GENERATE_BUTTON_HEIGHT))
                        .rounded(px(Radius::SM))
                        .bg(Accent::PRIMARY)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(Background::BASE)
                        .text_size(px(FontSize::SM))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .opacity(if can_generate {
                            Opacity::OPAQUE
                        } else {
                            Opacity::MEDIUM
                        })
                        .child(music_generate_label(cost))
                        .when(can_generate, |el| {
                            el.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.generate_music(cx);
                            }))
                        }),
                )
                .child(self.agent_mode_button("btn-music-agent", false, cx)),
        );

        div()
            .id("music-tab-root")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" && this.music_menu.take().is_some() {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .child(
                div()
                    .id("music-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(Spacing::LG_XL))
                    .pt(px(Spacing::MD))
                    .pb(px(Spacing::MD))
                    .gap(px(Spacing::MD_LG))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if this.music_menu.take().is_some() {
                                cx.notify();
                            }
                        }),
                    )
                    .child(source_section)
                    .child(model_section)
                    .child(prompt_section),
            )
            .child(bar)
            .when(
                matches!(
                    self.music_menu,
                    Some(MusicMenu::Agent) | Some(MusicMenu::AgentMood)
                ),
                |el| el.child(self.render_music_agent_menu(cx)),
            )
            .when(self.music.is_generating, |el| {
                el.child(generating_overlay("Generating…"))
            })
            .into_any_element()
    }
}

impl Render for MediaPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sync_from_shared_state() {
            cx.notify();
        }
        let active = self.state.active_tab.clone();
        let media_active = active == MediaPanelTab::Media;
        let captions_active = active == MediaPanelTab::Captions;
        let music_active = active == MediaPanelTab::Music;
        let weak_drag = cx.entity().downgrade();

        div()
            .id("media-panel")
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::SURFACE)
            // Scrub drags on captions/music numeric fields (inspector pattern).
            .on_drag_move::<TabScrub>(
                move |event: &DragMoveEvent<TabScrub>, _window, cx: &mut App| {
                    let _ = weak_drag.update(cx, |this: &mut MediaPanelView, inner_cx| {
                        if let Some(session) = this.tab_scrub.clone() {
                            let dx = event.event.position.x.as_f32() - session.start_x;
                            let value =
                                tab_scrub_value(session.field, session.start_value, dx as f64);
                            this.set_tab_scrub_value(session.field, value);
                            inner_cx.notify();
                        }
                    });
                },
            )
            // ── Left tab rail ──
            .child(
                div()
                    .id("tab-rail-container")
                    .flex()
                    .flex_row()
                    .h_full()
                    .child(
                        div()
                            .id("tab-rail")
                            .flex()
                            .flex_col()
                            .items_center()
                            .w(px(MediaPanel::TAB_RAIL_WIDTH))
                            .h_full()
                            .pt(px(Spacing::SM))
                            .pb(px(Spacing::SM))
                            .gap(px(Spacing::XS))
                            .bg(Background::RAISED)
                            .child(
                                tab_btn("tab-media", "M", media_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Media, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Media".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-captions", "C", captions_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Captions, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Captions".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-music", "♪", music_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Music, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Music".into(),
                                        })
                                        .into()
                                    }),
                            ),
                    )
                    // Hairline border separator
                    .child(div().w(px(1.0)).h_full().bg(BorderColors::PRIMARY)),
            )
            // ── Tab content area ──
            .child(
                div()
                    .id("tab-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(Background::SURFACE)
                    .child(match active {
                        MediaPanelTab::Media => self.render_media_tab(cx),
                        MediaPanelTab::Captions => self.render_captions_tab(cx),
                        MediaPanelTab::Music => self.render_music_tab(cx),
                    }),
            )
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;

    /// Library fixture: root has A-roll.mp4 + music.wav; folder f1 "Shoot"
    /// holds B-roll.mp4 + Sunset.png (AI-generated); f2 "Nested" is inside f1
    /// and holds take.mov.
    fn manifest() -> MediaManifest {
        serde_json::from_str(
            r#"{"version":1,
                "entries":[
                    {"id":"m1","name":"A-roll.mp4","type":"video","source":{"project":{"relativePath":"media/a.mp4"}},"duration":5.0},
                    {"id":"m2","name":"music.wav","type":"audio","source":{"project":{"relativePath":"media/b.wav"}},"duration":30.0},
                    {"id":"m3","name":"B-roll.mp4","type":"video","source":{"project":{"relativePath":"media/c.mp4"}},"duration":9.0,"folderId":"f1"},
                    {"id":"m4","name":"Sunset.png","type":"image","source":{"project":{"relativePath":"media/d.png"}},"duration":0.0,"folderId":"f1",
                     "generationInput":{"prompt":"sunset","model":"m","duration":5,"aspectRatio":"16:9"}},
                    {"id":"m5","name":"take.mov","type":"video","source":{"project":{"relativePath":"media/e.mov"}},"duration":2.0,"folderId":"f2"}
                ],
                "folders":[
                    {"id":"f1","name":"Shoot"},
                    {"id":"f2","name":"Nested","parentFolderId":"f1"}
                ]}"#,
        )
        .unwrap()
    }

    fn ids(entries: &[&MediaManifestEntry]) -> Vec<String> {
        entries.iter().map(|e| e.id.clone()).collect()
    }

    fn state() -> LibraryState {
        LibraryState::default()
    }

    // ── 1.1 visible_entries: search dimension ──

    #[test]
    fn search_filters_by_name_substring_and_clear_restores() {
        let m = manifest();
        let mut s = state();
        s.search_query = "roll".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3"]);
        s.search_query.clear();
        // Folders view, root bucket after clearing.
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2"]);
    }

    #[test]
    fn search_is_case_insensitive_and_trims() {
        let m = manifest();
        let mut s = state();
        s.search_query = "  B-ROLL  ".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m3"]);
        s.search_query = "   ".into();
        assert!(!s.search_active(), "whitespace-only query is not a search");
    }

    #[test]
    fn search_spans_all_folders_even_inside_one() {
        let m = manifest();
        let mut s = state();
        s.current_folder = Some("f1".into());
        s.search_query = "a".into();
        let got = ids(&visible_entries(&m, &s));
        assert!(
            got.contains(&"m1".to_string()),
            "root asset found from inside f1"
        );
        assert!(got.contains(&"m5".to_string()), "nested asset found too");
    }

    // ── 1.1 visible_entries: filter dimension ──

    #[test]
    fn type_filter_restricts_and_toggle_roundtrips() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.toggle_type_filter(ClipType::Audio);
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2"]);
        s.toggle_type_filter(ClipType::Video);
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2", "m3", "m5"]);
        s.toggle_type_filter(ClipType::Audio);
        s.toggle_type_filter(ClipType::Video);
        assert!(!s.has_active_filters(), "toggling off clears the filter");
        assert_eq!(visible_entries(&m, &s).len(), 5);
    }

    #[test]
    fn ai_filter_keeps_generated_only() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.filter_ai = true;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m4"]);
        assert!(s.has_active_filters());
    }

    #[test]
    fn filters_and_search_combine_with_and() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.type_filter = vec![ClipType::Video];
        s.search_query = "roll".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3"]);
        s.filter_ai = true;
        assert!(visible_entries(&m, &s).is_empty());
    }

    #[test]
    fn clear_filters_resets_types_and_ai() {
        let mut s = state();
        s.type_filter = vec![ClipType::Audio];
        s.filter_ai = true;
        s.clear_filters();
        assert!(!s.has_active_filters());
        assert!(s.type_filter.is_empty());
        assert!(!s.filter_ai);
    }

    // ── 1.1 visible_entries: folder scope dimension ──

    #[test]
    fn folders_mode_scopes_to_current_folder() {
        let m = manifest();
        let mut s = state();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2"], "root bucket");
        s.current_folder = Some("f1".into());
        assert_eq!(ids(&visible_entries(&m, &s)), ["m3", "m4"]);
        s.current_folder = Some("f2".into());
        assert_eq!(ids(&visible_entries(&m, &s)), ["m5"]);
    }

    #[test]
    fn flat_mode_spans_library_and_hides_folders() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.current_folder = Some("f1".into());
        assert_eq!(visible_entries(&m, &s).len(), 5, "folder scope ignored");
        assert!(visible_folders(&m, &s).is_empty(), "no folder tiles in flat");
    }

    #[test]
    fn visible_folders_lists_current_subfolders_only() {
        let m = manifest();
        let mut s = state();
        let root: Vec<&str> = visible_folders(&m, &s)
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(root, ["f1"]);
        s.current_folder = Some("f1".into());
        let inner: Vec<&str> = visible_folders(&m, &s)
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(inner, ["f2"]);
        s.search_query = "Shoot".into();
        assert!(
            visible_folders(&m, &s).is_empty(),
            "search view has no folder tiles"
        );
    }

    // ── 1.1 visible_entries: sort dimension ──

    #[test]
    fn sort_name_is_case_insensitive() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Name;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3", "m2", "m4", "m5"]);
    }

    #[test]
    fn sort_date_added_keeps_manifest_order() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::DateAdded;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2", "m3", "m4", "m5"]);
    }

    #[test]
    fn sort_duration_is_descending() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Duration;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2", "m3", "m1", "m5", "m4"]);
    }

    #[test]
    fn sort_type_groups_by_kind() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Type;
        // audio < image < video; stable within a kind.
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2", "m4", "m1", "m3", "m5"]);
    }

    // ── 1.1 grouped sections + folder helpers ──

    #[test]
    fn grouped_sections_cover_root_and_folders_by_path() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Grouped;
        let sections = grouped_sections(&m, &s);
        let titles: Vec<&str> = sections.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Library", "Shoot", "Shoot / Nested"]);
        assert_eq!(ids(&sections[0].2), ["m1", "m2"]);
        assert_eq!(ids(&sections[1].2), ["m3", "m4"]);
        assert_eq!(ids(&sections[2].2), ["m5"]);
    }

    #[test]
    fn grouped_sections_skip_empty_root_and_filter_buckets() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Grouped;
        s.type_filter = vec![ClipType::Image];
        let sections = grouped_sections(&m, &s);
        let titles: Vec<&str> = sections.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(
            titles,
            ["Shoot", "Shoot / Nested"],
            "empty root section skipped"
        );
        assert_eq!(ids(&sections[0].2), ["m4"]);
        assert!(sections[1].2.is_empty(), "empty folder sections stay visible");
    }

    #[test]
    fn folder_path_walks_to_root_and_survives_cycles() {
        let m = manifest();
        let path: Vec<&str> = folder_path(&m, Some("f2"))
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(path, ["f1", "f2"]);
        assert!(folder_path(&m, None).is_empty());
        assert!(folder_path(&m, Some("ghost")).is_empty());

        let cyclic: MediaManifest = serde_json::from_str(
            r#"{"version":1,"entries":[],"folders":[
                {"id":"a","name":"A","parentFolderId":"b"},
                {"id":"b","name":"B","parentFolderId":"a"}]}"#,
        )
        .unwrap();
        let _ = folder_path(&cyclic, Some("a")); // must terminate
    }

    #[test]
    fn folder_child_count_sums_subfolders_and_assets() {
        let m = manifest();
        assert_eq!(folder_child_count(&m, "f1"), 3, "f2 + m3 + m4");
        assert_eq!(folder_child_count(&m, "f2"), 1);
        assert_eq!(folder_child_count(&m, "ghost"), 0);
    }

    // ── 1.2 selection ──

    fn ordered() -> Vec<String> {
        ["m1", "m2", "m3", "m4", "m5"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn click_replaces_selection_and_sets_anchor() {
        let mut s = state();
        s.selection = vec!["m1".into(), "m2".into()];
        s.select_click("m3");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m3"));
    }

    #[test]
    fn toggle_adds_and_removes() {
        let mut s = state();
        s.select_toggle("m1");
        s.select_toggle("m3");
        assert_eq!(s.selection, ["m1", "m3"]);
        s.select_toggle("m1");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(
            s.selection_anchor.as_deref(),
            Some("m1"),
            "toggle moves the anchor"
        );
    }

    #[test]
    fn range_selects_inclusive_span_in_order() {
        let mut s = state();
        s.select_click("m2");
        s.select_range(&ordered(), "m5");
        assert_eq!(s.selection, ["m2", "m3", "m4", "m5"]);
    }

    #[test]
    fn range_works_backwards() {
        let mut s = state();
        s.select_click("m4");
        s.select_range(&ordered(), "m1");
        assert_eq!(s.selection, ["m1", "m2", "m3", "m4"]);
    }

    #[test]
    fn range_reextends_from_same_anchor() {
        let mut s = state();
        s.select_click("m2");
        s.select_range(&ordered(), "m5");
        s.select_range(&ordered(), "m3");
        assert_eq!(
            s.selection,
            ["m2", "m3"],
            "second shift-click re-extends from anchor"
        );
        assert_eq!(s.selection_anchor.as_deref(), Some("m2"));
    }

    #[test]
    fn range_without_anchor_falls_back_to_click() {
        let mut s = state();
        s.select_range(&ordered(), "m3");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m3"));
    }

    #[test]
    fn range_with_vanished_anchor_falls_back_to_click() {
        let mut s = state();
        s.select_click("ghost");
        s.select_range(&ordered(), "m2");
        assert_eq!(s.selection, ["m2"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m2"));
    }

    #[test]
    fn clear_selection_resets_ids_and_anchor() {
        let mut s = state();
        s.select_click("m1");
        s.clear_selection();
        assert!(s.selection.is_empty());
        assert!(s.selection_anchor.is_none());
    }

    // ── Tile context menus (context-menu-system 2.2/2.3) ────────────────

    fn entry_ids(entries: &[crate::context_menu::MenuEntry]) -> Vec<&'static str> {
        entries
            .iter()
            .filter_map(|e| match e {
                crate::context_menu::MenuEntry::Item(item) => Some(item.id),
                crate::context_menu::MenuEntry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn asset_menu_lists_rename_reveal_delete() {
        let ids = entry_ids(&MediaPanelView::asset_menu_entries(true));
        assert_eq!(ids, vec!["rename", "reveal", "delete"]);
    }

    #[test]
    fn asset_menu_drops_reveal_without_local_file() {
        let ids = entry_ids(&MediaPanelView::asset_menu_entries(false));
        assert_eq!(ids, vec!["rename", "delete"]);
    }

    #[test]
    fn folder_menu_lists_open_rename_delete() {
        let ids = entry_ids(&MediaPanelView::folder_menu_entries());
        assert_eq!(ids, vec!["open", "rename", "delete"]);
    }

    #[test]
    fn asset_delete_is_plain_destructive_not_confirm() {
        // delete_media only drops the manifest entry (no disk delete), so it
        // must not use the arm-then-confirm step reserved for data loss.
        for entry in MediaPanelView::asset_menu_entries(true) {
            if let crate::context_menu::MenuEntry::Item(item) = entry {
                if item.id == "delete" {
                    assert!(item.destructive);
                    assert!(item.confirm_label.is_none());
                }
            }
        }
    }
}

#[cfg(test)]
mod captions_music_tests {
    use super::*;

    /// Timeline fixture: one text track (never captionable), one video track
    /// with two clips, one audio track with one clip. 30fps, 1920×1080.
    fn timeline() -> Timeline {
        serde_json::from_str(
            r#"{
                "fps": 30, "width": 1920, "height": 1080,
                "tracks": [
                    {"id":"t-text","type":"text","clips":[
                        {"id":"c-text","mediaRef":"m0","sourceClipType":"text","startFrame":0,"durationFrames":30}]},
                    {"id":"t-video","type":"video","clips":[
                        {"id":"c-v1","mediaRef":"m1","sourceClipType":"video","startFrame":0,"durationFrames":120},
                        {"id":"c-v2","mediaRef":"m2","sourceClipType":"video","startFrame":120,"durationFrames":120}]},
                    {"id":"t-audio","type":"audio","clips":[
                        {"id":"c-a1","mediaRef":"m3","sourceClipType":"audio","startFrame":0,"durationFrames":300}]}
                ]
            }"#,
        )
        .unwrap()
    }

    fn empty_timeline() -> Timeline {
        serde_json::from_str(r#"{"fps":30,"width":1920,"height":1080,"tracks":[]}"#).unwrap()
    }

    // ── 1.1 caption state carrier ───────────────────────────────────

    #[test]
    fn caption_defaults_mirror_swift() {
        let s = CaptionsState::default();
        assert_eq!(s.font_size, 48.0, "AppTheme.Caption.defaultFontSize");
        assert_eq!(s.center_x, 0.5);
        assert_eq!(s.center_y, 0.9, "AppTheme.Caption.defaultCenterY");
        assert!(!s.background_enabled, "TextStyle background default off");
        assert_eq!(s.text_case, CaptionCase::Auto);
        assert!(!s.censor_profanity);
        assert!(s.language.is_none());
        assert!(s.selected_track_id.is_none(), "Auto source");
        assert!(!s.is_generating);
    }

    #[test]
    fn caption_case_wire_values_match_search_core_serde() {
        // search_core::caption::TextCase serializes lowercase variant names.
        assert_eq!(CaptionCase::Auto.config_value(), "auto");
        assert_eq!(CaptionCase::Upper.config_value(), "upper");
        assert_eq!(CaptionCase::Lower.config_value(), "lower");
        assert_eq!(CaptionCase::Upper.label(), "UPPERCASE");
        assert_eq!(CaptionCase::Lower.label(), "lowercase");
    }

    // ── caption source resolution ───────────────────────────────────

    #[test]
    fn caption_targets_cover_audio_and_video_not_text() {
        let tl = timeline();
        assert_eq!(
            captionable_clip_ids(&tl, None),
            ["c-v1", "c-v2", "c-a1"],
            "text clips are not caption sources"
        );
    }

    #[test]
    fn caption_targets_scope_to_track() {
        let tl = timeline();
        assert_eq!(captionable_clip_ids(&tl, Some("t-audio")), ["c-a1"]);
        assert!(captionable_clip_ids(&tl, Some("t-text")).is_empty());
        assert!(captionable_clip_ids(&tl, Some("ghost")).is_empty());
    }

    #[test]
    fn selected_captionable_ids_intersects_selection() {
        let mut tl = timeline();
        tl.selected_clip_ids = ["c-v2", "c-text"].iter().map(|s| s.to_string()).collect();
        assert_eq!(selected_captionable_ids(&tl), ["c-v2"]);
    }

    #[test]
    fn caption_track_entries_skip_empty_tracks() {
        let tl = timeline();
        let entries = caption_track_entries(&tl);
        let ids: Vec<&str> = entries.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(ids, ["t-video", "t-audio"], "text track has no targets");
        assert_eq!(entries[0].1, "V1");
        assert_eq!(entries[0].2, 2);
        assert_eq!(entries[1].1, "A1");
    }

    #[test]
    fn caption_source_summary_branches() {
        let tl = timeline();
        assert_eq!(caption_source_summary(&tl, None), "Auto");
        assert_eq!(caption_source_summary(&tl, Some("t-video")), "V1 · 2");
        assert_eq!(caption_source_summary(&tl, Some("ghost")), "No track");
        assert_eq!(caption_source_summary(&empty_timeline(), None), "No audio");

        let mut sel = timeline();
        sel.selected_clip_ids = ["c-a1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(caption_source_summary(&sel, None), "Selected Clips · 1");
    }

    #[test]
    fn caption_effective_count_prefers_selection() {
        let tl = timeline();
        assert_eq!(caption_effective_count(&tl, None), 3, "auto = all targets");
        assert_eq!(caption_effective_count(&tl, Some("t-audio")), 1);
        assert_eq!(caption_effective_count(&empty_timeline(), None), 0);

        let mut sel = timeline();
        sel.selected_clip_ids = ["c-v1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(caption_effective_count(&sel, None), 1);
        assert_eq!(
            caption_source_ids(&sel, None),
            ["c-v1"],
            "generation targets the selection"
        );
        assert_eq!(
            caption_source_ids(&sel, Some("t-audio")),
            ["c-a1"],
            "explicit track wins over selection"
        );
    }

    // ── placement snapping + scrub ──────────────────────────────────

    #[test]
    fn snap_center_thresholds() {
        assert_eq!(snap_center(0.49), 0.5, "inside snap threshold");
        assert_eq!(snap_center(0.515), 0.5);
        assert_eq!(snap_center(0.52), 0.52, "exactly at threshold stays");
        assert_eq!(snap_center(0.4799), 0.4799);
        assert_eq!(snap_center(0.9), 0.9);
    }

    #[test]
    fn tab_scrub_clamps_and_snaps() {
        assert_eq!(
            tab_scrub_value(TabScrubField::CaptionSize, 48.0, 300.0),
            300.0,
            "max font size"
        );
        assert_eq!(
            tab_scrub_value(TabScrubField::CaptionSize, 48.0, -100.0),
            12.0,
            "min font size"
        );
        // 0.4 + 19px * 0.005 = 0.495 → snapped to centre.
        assert_eq!(tab_scrub_value(TabScrubField::CaptionX, 0.4, 19.0), 0.5);
        assert_eq!(
            tab_scrub_value(TabScrubField::CaptionY, 0.9, 10_000.0),
            1.0,
            "max position"
        );
        assert_eq!(
            tab_scrub_value(TabScrubField::MusicDuration, 90.0, -200.0),
            1.0,
            "duration floor"
        );
        assert_eq!(
            tab_scrub_value(TabScrubField::MusicDuration, 90.0, 10_000.0),
            600.0,
            "duration ceiling"
        );
    }

    // ── 1.4 generate gating notes ───────────────────────────────────

    #[test]
    fn caption_generate_note_maps_outcomes() {
        let unavailable = Ok(serde_json::json!({
            "content": [{"type": "text", "text": "Actual transcription requires a remote API."}],
            "isError": true,
        }));
        assert_eq!(
            caption_generate_note(&unavailable).as_deref(),
            Some("Transcription unavailable — no speech engine is connected.")
        );
        let failed = Err("Clip 'x' not found".to_string());
        assert_eq!(
            caption_generate_note(&failed).as_deref(),
            Some("Clip 'x' not found")
        );
        let queued = Ok(serde_json::json!({
            "content": [{"type": "text", "text": "Transcribing 2 clips"}]
        }));
        assert_eq!(caption_generate_note(&queued), None, "real run: no note");
    }

    #[test]
    fn caption_agent_prompts_mirror_swift() {
        let p = caption_agent_prompt("add relevant emoji.");
        assert!(p.starts_with(
            "If the timeline has no captions yet, transcribe the spoken audio and add captions on word boundaries first. Then "
        ));
        assert!(p.ends_with("add relevant emoji."));
        assert_eq!(
            caption_translate_task("Japanese"),
            "translate the captions to Japanese, keeping each caption's timing unchanged."
        );
        assert_eq!(CAPTION_TRANSLATE_LANGUAGES.len(), 10);
        assert_eq!(CAPTION_AGENT_TASKS.len(), 3);
    }

    #[test]
    fn agent_handoff_unset_notes_instead_of_pretending() {
        // The OnceLock is unset in tests — the seam must surface a note.
        assert_eq!(
            send_agent_handoff("prompt").as_deref(),
            Some(AGENT_CHAT_UNWIRED_NOTE)
        );
    }

    // ── 2.1 music models + state ────────────────────────────────────

    #[test]
    fn music_models_are_catalog_music_entries() {
        let ids: Vec<&str> = music_models().iter().map(|m| m.id).collect();
        assert_eq!(ids, ["minimax-music-v2.6", "elevenlabs-music"]);
    }

    #[test]
    fn music_model_falls_back_to_first() {
        assert_eq!(music_model_for(None).unwrap().id, "minimax-music-v2.6");
        assert_eq!(
            music_model_for(Some("elevenlabs-music")).unwrap().id,
            "elevenlabs-music"
        );
        assert_eq!(
            music_model_for(Some("ghost")).unwrap().id,
            "minimax-music-v2.6",
            "unknown id falls back"
        );
        // TTS models never appear via the music selector.
        assert_eq!(
            music_model_for(Some("elevenlabs-tts-v3")).unwrap().id,
            "minimax-music-v2.6"
        );
    }

    #[test]
    fn music_state_defaults() {
        let s = MusicState::default();
        assert_eq!(s.mode, MusicMode::VideoToMusic);
        assert_eq!(s.text_duration, 90.0);
        assert!(!s.is_generating);
        assert_eq!(
            effective_music_mode(MusicMode::TextToMusic, music_model_for(None)),
            MusicMode::TextToMusic,
            "music models are prompt-driven → text mode kept"
        );
        assert_eq!(
            effective_music_mode(MusicMode::TextToMusic, None),
            MusicMode::VideoToMusic,
            "no model → video mode"
        );
    }

    // ── 2.3 cost + validation ───────────────────────────────────────

    #[test]
    fn music_cost_and_label() {
        let eleven = music_model_for(Some("elevenlabs-music")).unwrap();
        let cost = music_cost(eleven, "beat", 60.0).unwrap();
        assert!((cost - 0.12).abs() < 1e-9, "per-second 0.002 × 60");
        assert_eq!(music_generate_label(Some(cost)), "Generate · $0.12");

        let minimax = music_model_for(Some("minimax-music-v2.6")).unwrap();
        let flat = music_cost(minimax, "lofi hip hop", 60.0).unwrap();
        assert!((flat - 0.03).abs() < 1e-9, "flat pricing");

        assert_eq!(music_cost(eleven, "beat", 0.0), None, "empty span");
        assert_eq!(music_generate_label(None), "Generate");
    }

    #[test]
    fn music_validation_note_order() {
        let model = music_model_for(None);
        assert_eq!(
            music_validation_note(None, true, "x", 0.0, None, None).as_deref(),
            Some("No music models available.")
        );
        assert_eq!(
            music_validation_note(model, true, "   ", 0.0, None, None).as_deref(),
            Some("Describe the music to generate.")
        );
        assert_eq!(
            music_validation_note(model, false, "", 0.0, None, None).as_deref(),
            Some("Add video to the timeline, then mark a range to score only part of it.")
        );
        let too_long = music_validation_note(model, false, "", 1000.0, None, None).unwrap();
        assert!(too_long.contains("at most 900s"), "{too_long}");
        let too_short = music_validation_note(model, false, "", 0.2, None, None).unwrap();
        assert!(too_short.contains("at least 1s"), "{too_short}");
        let shortfall =
            music_validation_note(model, true, "beat", 0.0, Some(5.0), Some(1.0)).unwrap();
        assert!(shortfall.contains("needed"), "{shortfall}");
        assert_eq!(
            music_validation_note(model, false, "", 30.0, Some(0.03), Some(10.0)),
            None,
            "valid video-mode run"
        );
        assert_eq!(
            music_validation_note(model, true, "beat", 0.0, None, None),
            None,
            "valid text-mode run ignores the span"
        );
    }

    #[test]
    fn music_span_note_bounds() {
        let model = music_model_for(None).unwrap();
        assert!(music_span_note(model, 0.4).unwrap().contains("at least 1s"));
        assert_eq!(music_span_note(model, 1.0), None);
        assert_eq!(music_span_note(model, 900.4), None, "rounds to 900");
        assert!(music_span_note(model, 901.0).unwrap().contains("at most 900s"));
    }

    // ── 2.2 source span summary ─────────────────────────────────────

    #[test]
    fn music_source_summary_and_clock() {
        assert_eq!(music_source_summary(0, 30), "No video");
        assert_eq!(
            music_source_summary(960, 30),
            "Whole timeline · 0:00 – 0:32 · 32.0s"
        );
        assert_eq!(music_clock(5400, 30), "3:00");
        assert_eq!(music_clock(0, 0), "0:00", "fps floor");
        let tl = timeline();
        assert_eq!(tl.total_frames(), 300, "fixture span");
    }

    #[test]
    fn music_generate_note_maps_outcomes() {
        let unavailable = Ok(serde_json::json!({
            "content": [{"type": "text", "text": "Music generation requires a remote API."}],
            "isError": true,
        }));
        assert_eq!(
            music_generate_note(&unavailable).as_deref(),
            Some("Generation unavailable — no backend is connected.")
        );
        let queued = Ok(serde_json::json!({
            "content": [{"type": "text", "text": "Queued music generation"}]
        }));
        assert_eq!(music_generate_note(&queued), None);
    }

    #[test]
    fn music_agent_prompts_mirror_swift() {
        assert!(MUSIC_TIMELINE_PROMPT.starts_with("Score my timeline"));
        assert_eq!(
            music_mood_prompt("Lo-fi"),
            "Generate lo-fi music for my timeline and place it on an audio track aligned to the edit."
        );
        assert_eq!(MUSIC_MOODS.len(), 5);
    }

    // ── fonts ───────────────────────────────────────────────────────

    #[test]
    fn bundled_font_families_match_render_core_set() {
        // render_core::text::font_for matches these bundled families.
        for family in ["Anton", "Bebas Neue", "Permanent Marker", "Shrikhand", "Basement Grotesque", "Poppins"] {
            assert!(BUNDLED_FONT_FAMILIES.contains(&family), "{family}");
        }
        assert_eq!(BUNDLED_FONT_FAMILIES.len(), 6, "no dupes / strays");
    }
}
