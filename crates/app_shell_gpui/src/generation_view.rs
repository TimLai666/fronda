//! AI Generation panel gpui view — embedded inside the Media panel's media tab.
//!
//! Covers the Swift GenerationView: type picker, reference tiles, prompt,
//! caps-driven settings, cost estimate, and the real submission path.
//! Model data comes from `generation_core::model_catalog`; cost math mirrors
//! the Fal-era Swift `CostEstimator` (upstream `9dfde8d^`, USD prices).

use crate::text_area::{TextArea, TextAreaEvent};
use crate::theme::{
    Accent, Background, BorderColors, DropZone, FontSize, GenerationPanel, Opacity, Radius,
    Spacing, Status, Text, TrackColor,
};
use core_model::{ClipType, GenerationInput};
use generation_core::model_catalog::{self, ModelCaps, ModelConfig};
use generation_core::ModelKind;
use gpui::{
    div, prelude::*, px, svg, App, ClickEvent, Context, Entity, FocusHandle, Focusable, Hsla,
    InteractiveElement, ParentElement, Render, SharedString, Styled, Window,
};
use timeline_core::AssetDrag;

/// AI generation type (matches Swift GenerationType).
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum GenerationType {
    Video,
    Image,
    Audio,
}

impl GenerationType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Image => "Image",
            Self::Audio => "Audio",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            Self::Video => "icons/video.svg",
            Self::Image => "icons/photo.svg",
            Self::Audio => "icons/waveform.svg",
        }
    }

    pub fn accent_color(&self) -> Hsla {
        match self {
            Self::Video => TrackColor::VIDEO,
            Self::Image => TrackColor::IMAGE,
            Self::Audio => TrackColor::AUDIO,
        }
    }

    pub fn kind(&self) -> ModelKind {
        match self {
            Self::Video => ModelKind::Video,
            Self::Image => ModelKind::Image,
            Self::Audio => ModelKind::Audio,
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Video, Self::Image, Self::Audio]
    }
}

/// Catalog entries of a kind, in catalog order.
pub fn models_for(kind: ModelKind) -> Vec<&'static ModelConfig> {
    model_catalog::catalog()
        .iter()
        .filter(|m| m.kind() == kind)
        .collect()
}

/// Per-model generation parameters (Swift GenerationView @State params).
/// Shared across models; `sanitize_for` clamps them to a model's caps.
#[derive(Debug, Clone, PartialEq)]
pub struct GenParams {
    pub duration: i64,
    pub aspect_ratio: String,
    pub resolution: String,
    pub quality: String,
    pub num_images: i64,
    pub voice: String,
    pub instrumental: bool,
    pub audio_duration: i64,
    pub generate_audio: bool,
}

impl Default for GenParams {
    fn default() -> Self {
        Self {
            duration: 5,
            aspect_ratio: "16:9".into(),
            resolution: "1080p".into(),
            quality: "high".into(),
            num_images: 1,
            voice: String::new(),
            instrumental: false,
            audio_duration: 30,
            generate_audio: true,
        }
    }
}

impl GenParams {
    /// Swift `resetSettings` + `resetAudioState`: any value the model's caps
    /// don't list falls back to that model's default.
    pub fn sanitize_for(&mut self, model: &ModelConfig) {
        match &model.caps {
            ModelCaps::Video(c) => {
                if !c.aspect_ratios.iter().any(|a| *a == self.aspect_ratio) {
                    self.aspect_ratio = c.aspect_ratios.first().unwrap_or(&"16:9").to_string();
                }
                if let Some(resolutions) = &c.resolutions {
                    if !resolutions.iter().any(|r| *r == self.resolution) {
                        self.resolution = resolutions.first().unwrap_or(&"1080p").to_string();
                    }
                }
                if !c.durations.contains(&self.duration) {
                    self.duration = c.durations.first().copied().unwrap_or(5);
                }
                self.generate_audio = true;
            }
            ModelCaps::Image(c) => {
                if !c.aspect_ratios.iter().any(|a| *a == self.aspect_ratio) {
                    self.aspect_ratio = c.aspect_ratios.first().unwrap_or(&"16:9").to_string();
                }
                if let Some(resolutions) = &c.resolutions {
                    if !resolutions.iter().any(|r| *r == self.resolution) {
                        self.resolution = resolutions.first().unwrap_or(&"1080p").to_string();
                    }
                }
                if let Some(qualities) = &c.qualities {
                    if !qualities.iter().any(|q| *q == self.quality) {
                        self.quality = qualities.last().unwrap_or(&"high").to_string();
                    }
                }
                self.num_images = self.num_images.clamp(1, c.max_images.max(1));
            }
            ModelCaps::Audio(c) => {
                self.voice = c.default_voice.unwrap_or("").to_string();
                if !c.supports_instrumental {
                    self.instrumental = false;
                }
                if let Some(durations) = &c.durations {
                    if !durations.contains(&self.audio_duration) {
                        self.audio_duration = durations.first().copied().unwrap_or(30);
                    }
                }
            }
        }
    }
}

/// A referenced media-library asset, kind captured at assignment time.
#[derive(Debug, Clone, PartialEq)]
pub struct RefAsset {
    pub id: String,
    pub kind: ClipType,
}

/// Which reference slot an asset-picker click targets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefSlot {
    FirstFrame,
    LastFrame,
    Reference,
}

/// State for the generation panel.
#[derive(Debug, Clone)]
pub struct GenerationState {
    pub selected_type: GenerationType,
    pub prompt: String,
    pub lyrics: String,
    pub style_instructions: String,
    /// USD balance (placeholder until an account backend lands; None = unknown).
    pub credits_remaining: Option<f64>,
    /// Whether the user is on a paid plan (affects gating + credit popover).
    pub is_paid_plan: bool,
    pub video_model_id: String,
    pub image_model_id: String,
    pub audio_model_id: String,
    pub params: GenParams,
    /// Frames vs references mode for `frames_and_references_exclusive` models.
    pub use_first_last: bool,
    pub first_frame: Option<String>,
    pub last_frame: Option<String>,
    pub references: Vec<RefAsset>,
    /// Panel status line (submission failures / backend unavailable).
    pub status: Option<String>,
    pub show_model_picker: bool,
    pub show_voice_picker: bool,
    pub show_settings: bool,
    pub show_credit_popover: bool,
    pub asset_picker: Option<RefSlot>,
}

impl Default for GenerationState {
    fn default() -> Self {
        let default_id = |kind: ModelKind| {
            model_catalog::default_model(kind, false)
                .map(|m| m.id.to_string())
                .unwrap_or_default()
        };
        let mut state = Self {
            selected_type: GenerationType::Video,
            prompt: String::new(),
            lyrics: String::new(),
            style_instructions: String::new(),
            credits_remaining: Some(1_250.0),
            is_paid_plan: false,
            video_model_id: default_id(ModelKind::Video),
            image_model_id: default_id(ModelKind::Image),
            audio_model_id: default_id(ModelKind::Audio),
            params: GenParams::default(),
            use_first_last: true,
            first_frame: None,
            last_frame: None,
            references: Vec::new(),
            status: None,
            show_model_picker: false,
            show_voice_picker: false,
            show_settings: false,
            show_credit_popover: false,
            asset_picker: None,
        };
        state.params.sanitize_for(state.selected_model());
        state
    }
}

impl GenerationState {
    pub fn selected_model_id(&self) -> &str {
        match self.selected_type {
            GenerationType::Video => &self.video_model_id,
            GenerationType::Image => &self.image_model_id,
            GenerationType::Audio => &self.audio_model_id,
        }
    }

    /// The selected model, falling back to the first catalog entry of the kind
    /// (Swift `selectedModel` safe-index behavior).
    pub fn selected_model(&self) -> &'static ModelConfig {
        let kind = self.selected_type.kind();
        model_catalog::model_by_id(self.selected_model_id())
            .filter(|m| m.kind() == kind)
            .or_else(|| models_for(kind.clone()).first().copied())
            .expect("catalog has entries for every kind")
    }

    /// Swift `onChange(selectedType)`: re-derive settings, clear references.
    /// Re-selecting the active tab is a no-op — it must not wipe state (F3).
    pub fn select_type(&mut self, t: GenerationType) {
        if t == self.selected_type {
            return;
        }
        self.selected_type = t;
        self.params.sanitize_for(self.selected_model());
        self.clear_references();
        self.status = None;
        self.close_popovers();
    }

    /// Swift `onChange(selectedModelIndex)`: re-derive settings; model
    /// switches reset the refs mode/pool. Re-selecting the current model is
    /// a no-op (F3); image models that don't accept references drop them (F1).
    pub fn select_model(&mut self, id: &str) {
        let Some(model) = model_catalog::model_by_id(id) else {
            return;
        };
        let slot = match model.kind() {
            ModelKind::Video => &mut self.video_model_id,
            ModelKind::Image => &mut self.image_model_id,
            ModelKind::Audio => &mut self.audio_model_id,
            ModelKind::Upscale => return,
        };
        if slot.as_str() == id {
            return;
        }
        *slot = id.to_string();
        if model.kind() == self.selected_type.kind() {
            self.params.sanitize_for(model);
            match &model.caps {
                ModelCaps::Video(_) => {
                    self.use_first_last = true;
                    self.references.clear();
                }
                ModelCaps::Image(c) if !c.supports_image_reference => {
                    self.references.clear();
                }
                _ => {}
            }
        }
    }

    pub fn clear_references(&mut self) {
        self.first_frame = None;
        self.last_frame = None;
        self.references.clear();
    }

    /// Close every open popover. Returns true when one was open.
    pub fn close_popovers(&mut self) -> bool {
        let was_open = self.show_model_picker
            || self.show_voice_picker
            || self.show_settings
            || self.show_credit_popover
            || self.asset_picker.is_some();
        self.show_model_picker = false;
        self.show_voice_picker = false;
        self.show_settings = false;
        self.show_credit_popover = false;
        self.asset_picker = None;
        was_open
    }

    /// Assign a picked asset to a reference slot, enforcing model caps.
    pub fn assign_reference(&mut self, slot: RefSlot, id: &str, kind: ClipType) {
        match slot {
            RefSlot::FirstFrame => self.first_frame = Some(id.to_string()),
            RefSlot::LastFrame => self.last_frame = Some(id.to_string()),
            RefSlot::Reference => match can_add_reference(self, id, kind) {
                Ok(()) => self.references.push(RefAsset {
                    id: id.to_string(),
                    kind,
                }),
                Err(reason) => self.status = Some(reason),
            },
        }
    }
}

// ── Pure panel logic (unit-tested) ─────────────────────────────────

fn video_caps(state: &GenerationState) -> Option<&'static model_catalog::VideoCaps> {
    match &state.selected_model().caps {
        ModelCaps::Video(c) => Some(c),
        _ => None,
    }
}

fn audio_caps(state: &GenerationState) -> Option<&'static model_catalog::AudioCaps> {
    match &state.selected_model().caps {
        ModelCaps::Audio(c) => Some(c),
        _ => None,
    }
}

/// Swift `effectiveResolution`: only models with a resolutions list get one.
pub fn effective_resolution(state: &GenerationState) -> Option<String> {
    let has_resolutions = match &state.selected_model().caps {
        ModelCaps::Video(c) => c.resolutions.is_some(),
        ModelCaps::Image(c) => c.resolutions.is_some(),
        ModelCaps::Audio(_) => false,
    };
    has_resolutions.then(|| state.params.resolution.clone())
}

/// Swift `supportsAudioToggle`.
pub fn supports_audio_toggle(state: &GenerationState) -> bool {
    video_caps(state).is_some_and(|c| c.audio_discount_rate.is_some())
}

fn effective_generate_audio(state: &GenerationState) -> bool {
    if supports_audio_toggle(state) {
        state.params.generate_audio
    } else {
        true
    }
}

/// Live USD estimate for the current form state (Swift `estimatedCost`).
pub fn estimated_cost(state: &GenerationState) -> Option<f64> {
    match &state.selected_model().caps {
        ModelCaps::Video(c) => {
            // No source-video input yet, so edit models estimate on 0 seconds.
            let seconds = if c.requires_source_video {
                0
            } else {
                state.params.duration
            };
            model_catalog::video_cost(
                c,
                seconds,
                effective_resolution(state).as_deref(),
                effective_generate_audio(state),
            )
        }
        ModelCaps::Image(c) => {
            let quality = c.qualities.as_ref().map(|_| state.params.quality.clone());
            model_catalog::image_cost(
                c,
                effective_resolution(state).as_deref(),
                quality.as_deref(),
                state.params.num_images,
            )
        }
        ModelCaps::Audio(c) => {
            let duration = c.durations.as_ref().map(|_| state.params.audio_duration);
            model_catalog::audio_cost(c, state.prompt.trim(), duration)
        }
    }
}

/// Swift `hasInsufficientCredits`: both sides known and cost exceeds balance.
pub fn has_insufficient_credits(cost: Option<f64>, remaining: Option<f64>) -> bool {
    matches!((cost, remaining), (Some(c), Some(l)) if c > l)
}

/// Swift `canAffordGeneration`: unknown balance never blocks.
pub fn can_afford(cost: Option<f64>, remaining: Option<f64>) -> bool {
    match (cost, remaining) {
        (_, None) => true,
        (Some(c), Some(l)) => c <= l,
        (None, Some(l)) => l > 0.0,
    }
}

/// Shortfall line shown when the estimate exceeds the balance.
pub fn credit_shortfall_message(cost: f64, remaining: f64) -> String {
    format!(
        "{} needed. Only {} remaining.",
        model_catalog::format_usd(Some(cost)),
        model_catalog::format_usd(Some(remaining))
    )
}

/// The selected model is gated behind a paid plan the account doesn't have.
pub fn model_locked(state: &GenerationState) -> bool {
    !model_catalog::model_available(state.is_paid_plan, state.selected_model().paid_only)
}

/// Swift `canSubmit` (minus the API-key gate) + credit affordability.
pub fn can_submit(state: &GenerationState) -> bool {
    if model_locked(state) {
        return false;
    }
    if !can_afford(estimated_cost(state), state.credits_remaining) {
        return false;
    }
    let trimmed = state.prompt.trim();
    match &state.selected_model().caps {
        ModelCaps::Video(c) => {
            if c.requires_source_video {
                // Edit-video source strip is not wired yet.
                return false;
            }
            if c.frames_and_references_exclusive
                && !state.use_first_last
                && state.references.is_empty()
            {
                return false;
            }
            !trimmed.is_empty()
        }
        ModelCaps::Audio(c) => trimmed.chars().count() as i64 >= c.min_prompt_length,
        ModelCaps::Image(_) => !trimmed.is_empty(),
    }
}

// ── Reference layout + caps (Swift showsFrameStrip / showsRefSections) ──

pub fn shows_mode_toggle(state: &GenerationState) -> bool {
    video_caps(state)
        .is_some_and(|c| c.frames_and_references_exclusive && !c.requires_source_video)
}

pub fn shows_frame_strip(state: &GenerationState) -> bool {
    video_caps(state).is_some_and(|c| {
        c.supports_first_frame
            && !c.requires_source_video
            && (!c.frames_and_references_exclusive || state.use_first_last)
    })
}

pub fn shows_ref_sections(state: &GenerationState) -> bool {
    video_caps(state).is_some_and(|c| {
        c.supports_references()
            && !c.requires_source_video
            && (!c.frames_and_references_exclusive || !state.use_first_last)
    })
}

pub fn shows_image_refs(state: &GenerationState) -> bool {
    matches!(&state.selected_model().caps, ModelCaps::Image(c) if c.supports_image_reference)
}

fn kind_label(kind: ClipType) -> &'static str {
    match kind {
        ClipType::Image => "image",
        ClipType::Video => "video",
        ClipType::Audio => "audio",
        _ => "media",
    }
}

/// Media kinds the picker offers for a slot, per the selected model's caps.
pub fn accepted_kinds(state: &GenerationState, slot: RefSlot) -> Vec<ClipType> {
    match slot {
        RefSlot::FirstFrame | RefSlot::LastFrame => vec![ClipType::Image],
        RefSlot::Reference => match &state.selected_model().caps {
            ModelCaps::Video(c) => {
                let mut kinds = Vec::new();
                if c.max_reference_images > 0 {
                    kinds.push(ClipType::Image);
                }
                if c.max_reference_videos > 0 {
                    kinds.push(ClipType::Video);
                }
                if c.max_reference_audios > 0 {
                    kinds.push(ClipType::Audio);
                }
                kinds
            }
            ModelCaps::Image(c) if c.supports_image_reference => vec![ClipType::Image],
            _ => Vec::new(),
        },
    }
}

/// Kind gate for dropping an asset on a reference slot (Swift
/// `validatedDropZone`): None = accepted, Some(msg) = rejection status line.
/// Cap rules still apply afterwards via `can_add_reference`.
pub fn drop_rejection_message(
    state: &GenerationState,
    slot: RefSlot,
    kind: ClipType,
) -> Option<String> {
    let accepted = accepted_kinds(state, slot);
    if accepted.contains(&kind) {
        return None;
    }
    if accepted.is_empty() {
        return Some(format!(
            "{} doesn't accept references.",
            state.selected_model().display_name
        ));
    }
    let kinds = accepted
        .iter()
        .map(|k| kind_label(*k))
        .collect::<Vec<_>>()
        .join(" or ");
    Some(format!("Drop {kinds} here."))
}

/// Per-model reference cap check (Swift `addRefAsset` + `isRefCapReached`).
pub fn can_add_reference(
    state: &GenerationState,
    id: &str,
    kind: ClipType,
) -> Result<(), String> {
    if state.references.iter().any(|r| r.id == id) {
        return Err("Already a reference.".into());
    }
    let model = state.selected_model();
    match &model.caps {
        ModelCaps::Video(c) => {
            if let Some(total) = c.max_total_references {
                if state.references.len() as i64 >= total {
                    return Err(format!("Reference limit reached ({total})."));
                }
            }
            let cap = match kind {
                ClipType::Image => c.max_reference_images,
                ClipType::Video => c.max_reference_videos,
                ClipType::Audio => c.max_reference_audios,
                _ => 0,
            };
            if cap == 0 {
                return Err(format!(
                    "{} doesn't accept {} references.",
                    model.display_name,
                    kind_label(kind)
                ));
            }
            let count = state.references.iter().filter(|r| r.kind == kind).count() as i64;
            if count >= cap {
                return Err(format!(
                    "{} reference limit reached ({cap}).",
                    kind_label(kind)
                ));
            }
            Ok(())
        }
        ModelCaps::Image(c) => {
            if !c.supports_image_reference {
                return Err(format!(
                    "{} doesn't accept reference images.",
                    model.display_name
                ));
            }
            if kind != ClipType::Image {
                return Err("Reference expects an image.".into());
            }
            Ok(())
        }
        ModelCaps::Audio(_) => Err("Audio generation doesn't take references.".into()),
    }
}

/// Swift `isRefCapReached` — hides the "+" tile when nothing more fits.
pub fn ref_cap_reached(state: &GenerationState) -> bool {
    match &state.selected_model().caps {
        ModelCaps::Video(c) => {
            if let Some(total) = c.max_total_references {
                if state.references.len() as i64 >= total {
                    return true;
                }
            }
            let full = |kind: ClipType, cap: i64| {
                cap == 0
                    || state.references.iter().filter(|r| r.kind == kind).count() as i64 >= cap
            };
            full(ClipType::Image, c.max_reference_images)
                && full(ClipType::Video, c.max_reference_videos)
                && full(ClipType::Audio, c.max_reference_audios)
        }
        _ => false,
    }
}

/// Swift `refCounterLabel` (simplified): "n/cap".
pub fn ref_counter_label(state: &GenerationState) -> String {
    match &state.selected_model().caps {
        ModelCaps::Video(c) => format!("{}/{}", state.references.len(), c.max_references()),
        _ => format!("{}", state.references.len()),
    }
}

// ── Settings popover model (Swift settingsPopoverContent) ──────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingField {
    Duration,
    AudioDuration,
    AspectRatio,
    Resolution,
    Quality,
    Count,
}

impl SettingField {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Duration | Self::AudioDuration => "Duration",
            Self::AspectRatio => "Aspect Ratio",
            Self::Resolution => "Resolution",
            Self::Quality => "Quality",
            Self::Count => "Count",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingToggle {
    Instrumental,
    GenerateAudio,
}

impl SettingToggle {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Instrumental => "Instrumental",
            Self::GenerateAudio => "Generate audio",
        }
    }
}

/// Which pickers the current model's caps produce, in Swift popover order.
pub fn visible_setting_fields(state: &GenerationState) -> Vec<SettingField> {
    let mut fields = Vec::new();
    match &state.selected_model().caps {
        ModelCaps::Video(c) => {
            if !c.durations.is_empty() {
                fields.push(SettingField::Duration);
            }
            if !c.aspect_ratios.is_empty() {
                fields.push(SettingField::AspectRatio);
            }
            if c.resolutions.is_some() {
                fields.push(SettingField::Resolution);
            }
        }
        ModelCaps::Image(c) => {
            if !c.aspect_ratios.is_empty() {
                fields.push(SettingField::AspectRatio);
            }
            if c.resolutions.is_some() {
                fields.push(SettingField::Resolution);
            }
            if c.qualities.is_some() {
                fields.push(SettingField::Quality);
            }
            if c.max_images > 1 {
                fields.push(SettingField::Count);
            }
        }
        ModelCaps::Audio(c) => {
            if c.durations.is_some() {
                fields.push(SettingField::AudioDuration);
            }
        }
    }
    fields
}

pub fn visible_setting_toggles(state: &GenerationState) -> Vec<SettingToggle> {
    let mut toggles = Vec::new();
    match &state.selected_model().caps {
        ModelCaps::Audio(c) if c.supports_instrumental => toggles.push(SettingToggle::Instrumental),
        ModelCaps::Video(c) if c.audio_discount_rate.is_some() => {
            toggles.push(SettingToggle::GenerateAudio)
        }
        _ => {}
    }
    toggles
}

/// Swift `hasAnySettings` — gear button visibility.
pub fn has_any_settings(state: &GenerationState) -> bool {
    !visible_setting_fields(state).is_empty() || !visible_setting_toggles(state).is_empty()
}

/// Image resolution ids like "1024x1024" get a friendly label (Swift
/// `ImageModelConfig.resolutionDisplayLabel`); everything else displays raw.
fn resolution_label(state: &GenerationState, id: &str) -> String {
    if !matches!(state.selected_type, GenerationType::Image) {
        return id.to_string();
    }
    use generation_core::generation_payload::ImageGenerationPayload as Payload;
    match Payload::parse_resolution_label(id) {
        Some((w, h)) => {
            let label = Payload::resolution_display_label(w, h);
            if label.is_empty() {
                id.to_string()
            } else {
                label
            }
        }
        None => id.to_string(),
    }
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Options for a settings field as (value, display label) pairs.
pub fn setting_options(state: &GenerationState, field: SettingField) -> Vec<(String, String)> {
    let model = state.selected_model();
    match (field, &model.caps) {
        (SettingField::Duration, ModelCaps::Video(c)) => c
            .durations
            .iter()
            .map(|d| (d.to_string(), format!("{d}s")))
            .collect(),
        (SettingField::AudioDuration, ModelCaps::Audio(c)) => c
            .durations
            .as_ref()
            .map(|ds| ds.iter().map(|d| (d.to_string(), format!("{d}s"))).collect())
            .unwrap_or_default(),
        (SettingField::AspectRatio, ModelCaps::Video(c)) => c
            .aspect_ratios
            .iter()
            .map(|a| (a.to_string(), model_catalog::aspect_ratio_display_label(a)))
            .collect(),
        (SettingField::AspectRatio, ModelCaps::Image(c)) => c
            .aspect_ratios
            .iter()
            .map(|a| (a.to_string(), model_catalog::aspect_ratio_display_label(a)))
            .collect(),
        (SettingField::Resolution, ModelCaps::Video(c)) => c
            .resolutions
            .as_ref()
            .map(|rs| {
                rs.iter()
                    .map(|r| (r.to_string(), r.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        (SettingField::Resolution, ModelCaps::Image(c)) => c
            .resolutions
            .as_ref()
            .map(|rs| {
                rs.iter()
                    .map(|r| (r.to_string(), resolution_label(state, r)))
                    .collect()
            })
            .unwrap_or_default(),
        (SettingField::Quality, ModelCaps::Image(c)) => c
            .qualities
            .as_ref()
            .map(|qs| qs.iter().map(|q| (q.to_string(), capitalized(q))).collect())
            .unwrap_or_default(),
        (SettingField::Count, ModelCaps::Image(c)) => (1..=c.max_images.max(1))
            .map(|n| (n.to_string(), n.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// The currently selected value for a settings field.
pub fn setting_value(state: &GenerationState, field: SettingField) -> String {
    match field {
        SettingField::Duration => state.params.duration.to_string(),
        SettingField::AudioDuration => state.params.audio_duration.to_string(),
        SettingField::AspectRatio => state.params.aspect_ratio.clone(),
        SettingField::Resolution => state.params.resolution.clone(),
        SettingField::Quality => state.params.quality.clone(),
        SettingField::Count => state.params.num_images.to_string(),
    }
}

/// Apply a picked option value to the panel state.
pub fn apply_setting(state: &mut GenerationState, field: SettingField, value: &str) {
    match field {
        SettingField::Duration => {
            if let Ok(v) = value.parse() {
                state.params.duration = v;
            }
        }
        SettingField::AudioDuration => {
            if let Ok(v) = value.parse() {
                state.params.audio_duration = v;
            }
        }
        SettingField::AspectRatio => state.params.aspect_ratio = value.to_string(),
        SettingField::Resolution => state.params.resolution = value.to_string(),
        SettingField::Quality => state.params.quality = value.to_string(),
        SettingField::Count => {
            if let Ok(v) = value.parse() {
                state.params.num_images = v;
            }
        }
    }
}

/// Swift `settingsSummary` — the gear button label.
pub fn settings_summary(state: &GenerationState) -> String {
    let p = &state.params;
    if let Some(c) = audio_caps(state) {
        let mut parts = Vec::new();
        if c.durations.is_some() {
            parts.push(format!("{}s", p.audio_duration));
        }
        if c.supports_instrumental && p.instrumental {
            parts.push("Instrumental".to_string());
        }
        return if parts.is_empty() {
            "Settings".to_string()
        } else {
            parts.join(" · ")
        };
    }
    let model = state.selected_model();
    let mut parts = Vec::new();
    match &model.caps {
        ModelCaps::Video(c) => {
            if c.resolutions.is_some() {
                parts.push(p.resolution.clone());
            }
            parts.push(format!("{}s", p.duration));
            if !p.aspect_ratio.is_empty() && !c.aspect_ratios.is_empty() {
                parts.push(p.aspect_ratio.clone());
            }
        }
        ModelCaps::Image(c) => {
            if c.resolutions.is_some() {
                parts.push(resolution_label(state, &p.resolution));
            }
            if c.qualities.is_some() {
                parts.push(p.quality.clone());
            }
            if !p.aspect_ratio.is_empty() && !c.aspect_ratios.is_empty() {
                parts.push(p.aspect_ratio.clone());
            }
            if c.max_images > 1 && p.num_images > 1 {
                parts.push(format!("×{}", p.num_images));
            }
        }
        ModelCaps::Audio(_) => {}
    }
    parts.join(" · ")
}

// ── Submission (Swift submitGeneration → executor generate path) ───

/// Build the full GenerationInput for the current form state (Swift
/// `submitGeneration`'s genInput, including reference asset ids).
pub fn build_generation_input(state: &GenerationState) -> GenerationInput {
    let model = state.selected_model();
    let p = &state.params;
    // Only models that actually offer aspect ratios transmit one (review F5).
    let has_aspect = match &model.caps {
        ModelCaps::Video(c) => !c.aspect_ratios.is_empty(),
        ModelCaps::Image(c) => !c.aspect_ratios.is_empty(),
        _ => false,
    };
    let mut input = GenerationInput {
        prompt: state.prompt.clone(),
        model: model.id.to_string(),
        aspect_ratio: if has_aspect {
            p.aspect_ratio.clone()
        } else {
            String::new()
        },
        resolution: effective_resolution(state),
        ..GenerationInput::default()
    };
    match &model.caps {
        ModelCaps::Video(c) => {
            input.duration = if c.requires_source_video { 0 } else { p.duration };
            if c.audio_discount_rate.is_some() {
                input.generate_audio = Some(p.generate_audio);
            }
            let mut primary = Vec::new();
            if shows_frame_strip(state) {
                if let Some(f) = &state.first_frame {
                    primary.push(f.clone());
                }
                if c.supports_last_frame {
                    if let Some(l) = &state.last_frame {
                        primary.push(l.clone());
                    }
                }
            }
            if !primary.is_empty() {
                input.image_url_asset_ids = Some(primary);
            }
            if shows_ref_sections(state) {
                let by_kind = |kind: ClipType| -> Option<Vec<String>> {
                    let ids: Vec<String> = state
                        .references
                        .iter()
                        .filter(|r| r.kind == kind)
                        .map(|r| r.id.clone())
                        .collect();
                    (!ids.is_empty()).then_some(ids)
                };
                input.reference_image_asset_ids = by_kind(ClipType::Image);
                input.reference_video_asset_ids = by_kind(ClipType::Video);
                input.reference_audio_asset_ids = by_kind(ClipType::Audio);
            }
        }
        ModelCaps::Image(c) => {
            if c.qualities.is_some() {
                input.quality = Some(p.quality.clone());
            }
            if c.max_images > 1 {
                let clamped = p.num_images.clamp(1, c.max_images);
                if clamped > 1 {
                    input.num_images = Some(clamped);
                }
            }
            // References only for models that accept them (review F1).
            if c.supports_image_reference {
                let ids: Vec<String> = state.references.iter().map(|r| r.id.clone()).collect();
                if !ids.is_empty() {
                    input.image_url_asset_ids = Some(ids);
                }
            }
        }
        ModelCaps::Audio(c) => {
            input.duration = if c.durations.is_some() {
                p.audio_duration
            } else {
                0
            };
            if c.voices.is_some() && !p.voice.is_empty() {
                input.voice = Some(p.voice.clone());
            }
            if c.supports_lyrics && !state.lyrics.is_empty() {
                input.lyrics = Some(state.lyrics.clone());
            }
            if c.supports_style_instructions && !state.style_instructions.is_empty() {
                input.style_instructions = Some(state.style_instructions.clone());
            }
            if c.supports_instrumental {
                input.instrumental = Some(p.instrumental);
            }
        }
    }
    input
}

/// Map the form state to the shared executor's generate tool + args.
/// Extra args beyond today's stub schema ride along for the future backend.
pub fn generation_tool_call(state: &GenerationState) -> (&'static str, serde_json::Value) {
    let input = build_generation_input(state);
    let model = state.selected_model();
    let mut args = serde_json::json!({
        "prompt": input.prompt,
        "model": input.model,
    });
    let obj = args.as_object_mut().expect("args is an object");
    let mut set = |key: &str, value: serde_json::Value| {
        obj.insert(key.to_string(), value);
    };
    if input.duration > 0 {
        set("duration", serde_json::json!(input.duration as f64));
    }
    if !input.aspect_ratio.is_empty() {
        set("aspectRatio", serde_json::json!(input.aspect_ratio));
    }
    if let Some(r) = &input.resolution {
        set("resolution", serde_json::json!(r));
    }
    if let Some(q) = &input.quality {
        set("quality", serde_json::json!(q));
    }
    if let Some(n) = input.num_images {
        set("numImages", serde_json::json!(n));
    }
    if let Some(v) = &input.voice {
        set("voice", serde_json::json!(v));
    }
    if let Some(l) = &input.lyrics {
        set("lyrics", serde_json::json!(l));
    }
    if let Some(s) = &input.style_instructions {
        set("style", serde_json::json!(s));
    }
    if let Some(i) = input.instrumental {
        set("instrumental", serde_json::json!(i));
    }
    if let Some(g) = input.generate_audio {
        set("generateAudio", serde_json::json!(g));
    }
    if let Some(ids) = &input.image_url_asset_ids {
        set("imageURLAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_image_asset_ids {
        set("referenceImageAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_video_asset_ids {
        set("referenceVideoAssetIds", serde_json::json!(ids));
    }
    if let Some(ids) = &input.reference_audio_asset_ids {
        set("referenceAudioAssetIds", serde_json::json!(ids));
    }
    let tool = match &model.caps {
        ModelCaps::Video(_) => "generate_video",
        ModelCaps::Image(_) => "generate_image",
        ModelCaps::Audio(_) => "generate_audio",
    };
    (tool, args)
}

/// What a generate tool result means for the panel.
#[derive(Debug, Clone, PartialEq)]
pub enum SubmitOutcome {
    /// The job was accepted; the manifest holds a placeholder entry.
    Queued(String),
    /// No generation backend is connected (the executor stubs mark this
    /// with "requires a remote API").
    Unavailable,
    Failed(String),
}

pub fn interpret_submission(result: &Result<serde_json::Value, String>) -> SubmitOutcome {
    match result {
        Err(reason) => SubmitOutcome::Failed(reason.clone()),
        Ok(value) => {
            let text = value["content"][0]["text"].as_str().unwrap_or("").to_string();
            let is_error = value
                .get("isError")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            if is_error && text.contains("requires a remote API") {
                SubmitOutcome::Unavailable
            } else if is_error {
                SubmitOutcome::Failed(text)
            } else {
                SubmitOutcome::Queued(text)
            }
        }
    }
}

/// True while any manifest entry has an in-flight generation status —
/// the only thing `is_generating` may reflect.
pub fn has_inflight_generation(manifest: &core_model::MediaManifest) -> bool {
    manifest.entries.iter().any(|e| {
        matches!(
            e.generation_status.as_deref(),
            Some("preparing" | "generating" | "downloading")
        )
    })
}

// ── The view ────────────────────────────────────────────────────────

/// gpui Generation panel view, embedded in the Media panel.
pub struct GenerationView {
    pub state: GenerationState,
    focus_handle: FocusHandle,
    /// Multiline prompt editor (IME-capable); `state.prompt` mirrors it.
    prompt_area: Entity<TextArea>,
    lyrics_area: Entity<TextArea>,
    style_area: Entity<TextArea>,
    /// Media-library items for the reference asset picker.
    media_items: Vec<crate::media_panel_model::MediaItem>,
    /// True while any generation is in flight (derived from the manifest).
    inflight: bool,
    media_revision: u64,
}

impl GenerationView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let prompt_area = cx.new(|cx| {
            TextArea::new(cx, "Describe what to generate…")
                .with_min_lines(3)
                .with_max_lines(8)
        });
        cx.subscribe(&prompt_area, |this: &mut Self, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                this.state.prompt = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let lyrics_area = cx.new(|cx| {
            TextArea::new(cx, "Lyrics (optional) — [Verse], [Chorus] tags supported")
                .with_min_lines(2)
                .with_max_lines(6)
        });
        cx.subscribe(&lyrics_area, |this: &mut Self, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                this.state.lyrics = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let style_area = cx.new(|cx| {
            TextArea::new(cx, "Style instructions (optional) — e.g. warm and slow")
                .with_min_lines(1)
                .with_max_lines(4)
        });
        cx.subscribe(&style_area, |this: &mut Self, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                this.state.style_instructions = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            state: GenerationState::default(),
            focus_handle: cx.focus_handle(),
            prompt_area,
            lyrics_area,
            style_area,
            media_items: Vec::new(),
            inflight: false,
            media_revision: u64::MAX,
        };
        view.sync_media_items();
        view
    }

    /// Refresh picker items + inflight flag from the shared manifest.
    fn sync_media_items(&mut self) {
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if revision == self.media_revision {
            return;
        }
        self.media_revision = revision;
        let executor = hub.executor();
        let Ok(exec) = executor.lock() else {
            return;
        };
        let root = hub.project_root();
        let mut panel = crate::media_panel_model::MediaPanelState::new();
        panel.sync_from_manifest(exec.media_manifest(), root.as_deref());
        self.media_items = panel.items;
        self.inflight = has_inflight_generation(exec.media_manifest());
    }

    fn media_item(&self, id: &str) -> Option<&crate::media_panel_model::MediaItem> {
        self.media_items.iter().find(|i| i.id == id)
    }

    /// Run the generate tool on the shared executor and reflect the outcome.
    fn submit(&mut self, cx: &mut Context<Self>) {
        if !can_submit(&self.state) {
            return;
        }
        let (tool, args) = generation_tool_call(&self.state);
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let result = match executor.lock() {
            Ok(mut exec) => exec.execute(tool, &args),
            Err(_) => Err("Editor state lock poisoned".to_string()),
        };
        match interpret_submission(&result) {
            SubmitOutcome::Queued(_) => {
                // Swift submitGeneration tail: reset the form.
                self.state.prompt.clear();
                self.state.lyrics.clear();
                self.state.style_instructions.clear();
                self.prompt_area.update(cx, |a, cx| a.set_text("", cx));
                self.lyrics_area.update(cx, |a, cx| a.set_text("", cx));
                self.style_area.update(cx, |a, cx| a.set_text("", cx));
                self.state.clear_references();
                self.state.status = None;
            }
            SubmitOutcome::Unavailable => {
                self.state.status =
                    Some("Generation unavailable — no backend is connected.".to_string());
            }
            SubmitOutcome::Failed(reason) => self.state.status = Some(reason),
        }
        cx.notify();
    }

    fn open_asset_picker(&mut self, slot: RefSlot, cx: &mut Context<Self>) {
        self.state.close_popovers();
        self.state.asset_picker = Some(slot);
        cx.notify();
    }

    /// Drop path into a reference slot — same type and cap rules as
    /// click-to-pick (`accepted_kinds` gate, then `assign_reference`).
    fn handle_asset_drop(
        &mut self,
        slot: RefSlot,
        id: &str,
        kind: ClipType,
        cx: &mut Context<Self>,
    ) {
        match drop_rejection_message(&self.state, slot, kind) {
            Some(reason) => self.state.status = Some(reason),
            None => self.state.assign_reference(slot, id, kind),
        }
        cx.notify();
    }

    // ── Render helpers ──────────────────────────────────────────────

    /// An empty reference tile ("+") that opens the asset picker.
    fn empty_tile(&self, label: &str, slot: RefSlot, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Spacing::XXS))
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::XXS))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .id(SharedString::from(format!("gen-tile-{label}")))
                    .w(px(GenerationPanel::REFERENCE_TILE_WIDTH))
                    .h(px(GenerationPanel::REFERENCE_TILE_HEIGHT))
                    .rounded(px(Radius::SM))
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::RAISED)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.open_asset_picker(slot, cx);
                    }))
                    // Second assignment path: drop a media asset on the slot.
                    .on_drop::<AssetDrag>(cx.listener(move |this, drag: &AssetDrag, _, cx| {
                        let id = drag.asset_id.clone();
                        this.handle_asset_drop(slot, &id, drag.media_type, cx);
                    }))
                    .drag_over::<AssetDrag>(|style, _, _, _| {
                        style.border_color(DropZone::BORDER).bg(DropZone::FILL)
                    })
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::MD))
                            .child("+"),
                    ),
            )
    }

    /// A filled reference tile: thumbnail + clear button.
    fn assigned_tile(
        &self,
        label: &str,
        asset_id: &str,
        on_clear: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let item = self.media_item(asset_id);
        let thumb = item.and_then(|i| match i.kind {
            ClipType::Image => i.source_path.clone(),
            ClipType::Video => i
                .source_path
                .as_deref()
                .and_then(crate::video_thumbnails::request_thumbnail),
            _ => None,
        });
        let icon = item
            .map(|i| crate::media_panel_model::tile_icon(&i.kind))
            .unwrap_or("?");
        let mut tile = div()
            .id(SharedString::from(format!("gen-ref-{label}-{asset_id}")))
            .relative()
            .w(px(GenerationPanel::REFERENCE_TILE_WIDTH))
            .h(px(GenerationPanel::REFERENCE_TILE_HEIGHT))
            .rounded(px(Radius::SM))
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .bg(Background::RAISED)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center();
        if let Some(path) = thumb {
            tile = tile.child(
                gpui::img(path)
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover),
            );
        } else {
            tile = tile.child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::MD))
                    .child(icon),
            );
        }
        tile = tile.child(
            div()
                .id(SharedString::from(format!("gen-ref-clear-{asset_id}")))
                .absolute()
                .top(px(Spacing::XXS))
                .right(px(Spacing::XXS))
                .w(px(14.0))
                .h(px(14.0))
                .rounded_full()
                .bg(Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: Opacity::STRONG,
                })
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::MICRO))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    cx.stop_propagation();
                    on_clear(this, cx);
                    cx.notify();
                }))
                .child("✕"),
        );
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Spacing::XXS))
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::XXS))
                    .child(label.to_string()),
            )
            .child(tile)
    }
}

impl Focusable for GenerationView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GenerationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_media_items();
        let st = self.state.clone();
        let model = st.selected_model();
        let selected = st.selected_type;
        let cost = estimated_cost(&st);
        let insufficient = has_insufficient_credits(cost, st.credits_remaining);
        let submit_enabled = can_submit(&st);
        let inflight = self.inflight;
        let requires_source = video_caps(&st).is_some_and(|c| c.requires_source_video);
        let last_frame_supported = video_caps(&st).is_some_and(|c| c.supports_last_frame);

        // Active tab bg matches HoverHighlight(isActive: true)
        let active_tab_bg: Hsla = Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.10,
        };

        // Status line: submission status, else the credit shortfall.
        let status_line: Option<String> = st.status.clone().or_else(|| {
            insufficient.then(|| {
                credit_shortfall_message(
                    cost.unwrap_or_default(),
                    st.credits_remaining.unwrap_or_default(),
                )
            })
        });

        div()
            .id("generation-panel")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            // aiGradientDark approximation: white(0.06)→white(0.11) gradient; avg ≈ SURFACE
            .bg(Background::SURFACE)
            .rounded(px(Radius::LG))
            .overflow_hidden()
            // Click on any unclaimed area / Esc closes open popovers.
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if this.state.close_popovers() {
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key.as_str() == "escape" && this.state.close_popovers() {
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            // ── Resize handle (Swift: resizeHandle — 24×2 capsule, white@soft, cursor ns-resize) ──
            .child(
                div()
                    .id("gen-resize-handle")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(Spacing::MD))
                    .cursor_ns_resize()
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(2.0))
                            .rounded_full()
                            .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::SOFT }),
                    ),
            )
            // ── Header: type tabs (left) + credit chip + activity + close (right) ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XS))
                    .gap(px(Spacing::XXS))
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    .child({
                        let mut pill = div()
                            .id("gen-type-pill")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XXS))
                            .px(px(Spacing::XXS))
                            .py(px(Spacing::XXS))
                            .rounded(px(Radius::SM))
                            .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::SUBTLE })
                            .border_1()
                            .border_color(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::FAINT });
                        for gen_type in GenerationType::all() {
                            let is_active = *gen_type == selected;
                            let gt = *gen_type;
                            let icon_path = gen_type.icon_path();
                            let accent = gen_type.accent_color();
                            let icon_color = if is_active { accent } else { Text::TERTIARY };
                            let text_color = if is_active { Text::PRIMARY } else { Text::TERTIARY };
                            pill = pill.child(
                                div()
                                    .id(SharedString::from(format!("gen-type-{}", gen_type.label())))
                                    .px(px(Spacing::SM_MD))
                                    .h(px(22.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .rounded(px(Radius::XS_SM))
                                    .cursor_pointer()
                                    .bg(if is_active { active_tab_bg } else { Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } })
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.state.select_type(gt);
                                        cx.notify();
                                    }))
                                    .child(svg().path(icon_path).w(px(11.0)).h(px(11.0)).text_color(icon_color))
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM))
                                            .text_color(text_color)
                                            .child(gen_type.label()),
                                    ),
                            );
                        }
                        pill
                    })
                    .child(div().flex_1())
                    // Credit chip (CreditSummaryView.compact) — only when credits known
                    .when_some(st.credits_remaining, |el, c| {
                        let show_popover = st.show_credit_popover;
                        let is_paid = st.is_paid_plan;
                        let display = if c.fract() == 0.0 {
                            format!("{c:.0}")
                        } else {
                            format!("{c:.2}")
                        };
                        el.child(
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id("credit-chip")
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(Spacing::XS))
                                        .px(px(Spacing::SM))
                                        .py(px(Spacing::XXS))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(BorderColors::SUBTLE)
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            cx.stop_propagation();
                                            let open = this.state.show_credit_popover;
                                            this.state.close_popovers();
                                            this.state.show_credit_popover = !open;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::SM))
                                                .child("$"),
                                        )
                                        .child(
                                            div()
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::XS))
                                                .child(display),
                                        ),
                                )
                                .when(show_popover, |el| el.child(credit_popover(is_paid, cx))),
                        )
                    })
                    // Project activity icon button
                    .child(
                        div()
                            .id("btn-gen-activity")
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child("≡"),
                    )
                    // Close button (xmark)
                    .child(
                        div()
                            .id("btn-gen-close")
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child("✕"),
                    ),
            )
            // ── Reference tiles area (caps-driven; audio has none) ──
            .when(selected != GenerationType::Audio, |el| {
                el.child(self.render_reference_area(&st, requires_source, last_frame_supported, cx))
            })
            // ── Status line (drop errors / unavailable / shortfall) ──
            .when_some(status_line, |el, message| {
                el.child(
                    div()
                        .px(px(Spacing::MD))
                        .pb(px(Spacing::XS))
                        .text_size(px(FontSize::XS))
                        .text_color(Status::ERROR)
                        .child(message),
                )
            })
            // ── Prompt input box ──
            .child(self.render_prompt_box(&st, model, cost, insufficient, submit_enabled, inflight, cx))
    }
}

impl GenerationView {
    fn render_reference_area(
        &self,
        st: &GenerationState,
        requires_source: bool,
        last_frame_supported: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mode_toggle = shows_mode_toggle(st);
        let frame_strip = shows_frame_strip(st);
        let ref_sections = shows_ref_sections(st);
        let image_refs = shows_image_refs(st);
        let use_first_last = st.use_first_last;

        let mut area = div()
            .relative()
            .flex()
            .flex_col()
            .px(px(Spacing::MD))
            .py(px(Spacing::SM_MD))
            .gap(px(Spacing::XS))
            .min_h(px(GenerationPanel::MEDIA_AREA_MIN_HEIGHT));

        if requires_source {
            // Edit-video source strip depends on the selection system (deferred).
            return area.child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .child("Source video input isn't available yet."),
            );
        }

        if mode_toggle {
            let seg_bg: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::HINT };
            let active_seg: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.14 };
            area = area.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(1.0))
                    .rounded(px(Radius::XS_SM))
                    .bg(seg_bg)
                    .p(px(1.0))
                    .w(px(GenerationPanel::REFERENCE_TILE_WIDTH * 2.0))
                    .child(
                        div()
                            .id("gen-seg-first-last")
                            .px(px(Spacing::SM))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .bg(if use_first_last { active_seg } else { Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } })
                            .text_size(px(FontSize::XXS))
                            .text_color(if use_first_last { Text::PRIMARY } else { Text::MUTED })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.state.use_first_last = true;
                                this.state.references.clear();
                                cx.notify();
                            }))
                            .child("First / Last"),
                    )
                    .child(
                        div()
                            .id("gen-seg-reference")
                            .px(px(Spacing::SM))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .bg(if !use_first_last { active_seg } else { Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } })
                            .text_size(px(FontSize::XXS))
                            .text_color(if !use_first_last { Text::PRIMARY } else { Text::MUTED })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.state.use_first_last = false;
                                this.state.first_frame = None;
                                this.state.last_frame = None;
                                cx.notify();
                            }))
                            .child("Reference"),
                    ),
            );
        }

        if ref_sections || image_refs {
            area = area.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child("References"),
                    )
                    .when(ref_sections, |el| {
                        el.child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XXS))
                                .child(ref_counter_label(st)),
                        )
                    }),
            );
        }

        let mut tiles = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_start()
            .gap(px(Spacing::SM));

        if frame_strip {
            tiles = match &st.first_frame {
                Some(id) => tiles.child(self.assigned_tile(
                    "First Frame",
                    id,
                    |this, _| this.state.first_frame = None,
                    cx,
                )),
                None => tiles.child(self.empty_tile("First Frame", RefSlot::FirstFrame, cx)),
            };
            if last_frame_supported {
                tiles = match &st.last_frame {
                    Some(id) => tiles.child(self.assigned_tile(
                        "Last Frame",
                        id,
                        |this, _| this.state.last_frame = None,
                        cx,
                    )),
                    None => tiles.child(self.empty_tile("Last Frame", RefSlot::LastFrame, cx)),
                };
            }
        }

        if ref_sections || image_refs {
            for r in &st.references {
                let rid = r.id.clone();
                tiles = tiles.child(self.assigned_tile(
                    &capitalized(kind_label(r.kind)),
                    &r.id,
                    move |this, _| this.state.references.retain(|x| x.id != rid),
                    cx,
                ));
            }
            if !ref_cap_reached(st) {
                tiles = tiles.child(self.empty_tile("Reference", RefSlot::Reference, cx));
            }
        }

        area = area.child(tiles);

        // ── Asset picker popover ──
        if let Some(slot) = st.asset_picker {
            let kinds = accepted_kinds(st, slot);
            let mut list = div()
                .id("gen-asset-picker-list")
                .flex()
                .flex_col()
                .max_h(px(GenerationPanel::LOADING_HEIGHT))
                .overflow_y_scroll();
            let items: Vec<_> = self
                .media_items
                .iter()
                .filter(|i| kinds.contains(&i.kind))
                .cloned()
                .collect();
            if items.is_empty() {
                list = list.child(
                    div()
                        .px(px(Spacing::MD))
                        .py(px(Spacing::XS))
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::XS))
                        .child("No matching media in the library."),
                );
            }
            for item in items {
                let id = item.id.clone();
                let kind = item.kind;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("gen-pick-{}", item.id)))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::SM))
                        .px(px(Spacing::MD))
                        .py(px(Spacing::XS))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.state.assign_reference(slot, &id, kind);
                            this.state.asset_picker = None;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_color(Text::TERTIARY)
                                .text_size(px(FontSize::XS))
                                .child(crate::media_panel_model::tile_icon(&item.kind)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::XS))
                                .overflow_hidden()
                                .child(item.name.clone()),
                        ),
                );
            }
            area = area.child(
                div()
                    .id("gen-asset-picker")
                    .absolute()
                    .top(px(Spacing::XS))
                    .left(px(Spacing::MD))
                    .w(px(240.0))
                    .rounded(px(Radius::MD))
                    .bg(Background::RAISED)
                    .border_1()
                    .border_color(BorderColors::PRIMARY)
                    .shadow_lg()
                    .py(px(Spacing::XS))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(list),
            );
        }

        area
    }

    #[allow(clippy::too_many_arguments)]
    fn render_prompt_box(
        &self,
        st: &GenerationState,
        model: &'static ModelConfig,
        cost: Option<f64>,
        insufficient: bool,
        submit_enabled: bool,
        inflight: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = st.selected_type;
        let show_lyrics = audio_caps(st).is_some_and(|c| c.supports_lyrics);
        let show_style = audio_caps(st).is_some_and(|c| c.supports_style_instructions);
        let voices = audio_caps(st).and_then(|c| c.voices.clone());
        let gear_visible = has_any_settings(st);

        let divider = || {
            div()
                .h(px(1.0))
                .w_full()
                .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::HINT })
        };

        let mut prompt_box = div()
            .relative()
            .flex()
            .flex_col()
            .mx(px(Spacing::MD))
            .mb(px(Spacing::SM_MD))
            .rounded(px(Radius::MD))
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .bg(Background::RAISED)
            .min_h(px(GenerationPanel::PROMPT_MIN_HEIGHT))
            .child(
                div()
                    .id("gen-prompt-input")
                    .flex_1()
                    .px(px(Spacing::SM_MD))
                    .pt(px(Spacing::SM_MD))
                    .pb(px(Spacing::XS))
                    .text_size(px(FontSize::SM))
                    .text_color(Text::PRIMARY)
                    .cursor_text()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.prompt_area.focus_handle(cx), cx);
                        cx.notify();
                    }))
                    .child(self.prompt_area.clone()),
            );

        if show_lyrics {
            prompt_box = prompt_box.child(divider()).child(
                div()
                    .id("gen-lyrics-input")
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .text_size(px(FontSize::SM))
                    .text_color(Text::PRIMARY)
                    .cursor_text()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.lyrics_area.focus_handle(cx), cx);
                        cx.notify();
                    }))
                    .child(self.lyrics_area.clone()),
            );
        }
        if show_style {
            prompt_box = prompt_box.child(divider()).child(
                div()
                    .id("gen-style-input")
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .text_size(px(FontSize::SM))
                    .text_color(Text::PRIMARY)
                    .cursor_text()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.style_area.focus_handle(cx), cx);
                        cx.notify();
                    }))
                    .child(self.style_area.clone()),
            );
        }

        // ── Footer: model · voice · gear · spacer · cost · generate ──
        let mut footer = div()
            .flex()
            .flex_row()
            .items_center()
            .px(px(Spacing::SM_MD))
            .pb(px(Spacing::SM_MD))
            .pt(px(Spacing::XXS))
            .gap(px(Spacing::XS))
            .child(
                div()
                    .id("btn-gen-model-picker")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        let open = this.state.show_model_picker;
                        this.state.close_popovers();
                        this.state.show_model_picker = !open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XXS))
                            .child(model.display_name),
                    )
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XXS))
                            .child("⌄"),
                    ),
            );

        if let Some(_voices) = &voices {
            let voice_label = if st.params.voice.is_empty() {
                "Voice".to_string()
            } else {
                st.params.voice.clone()
            };
            footer = footer.child(
                div()
                    .id("btn-gen-voice-picker")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        let open = this.state.show_voice_picker;
                        this.state.close_popovers();
                        this.state.show_voice_picker = !open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XXS))
                            .child(voice_label),
                    )
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XXS))
                            .child("⌄"),
                    ),
            );
        }

        if gear_visible {
            footer = footer.child(
                div()
                    .id("btn-gen-settings")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(20.0))
                    .px(px(Spacing::XS))
                    .rounded(px(Radius::XS))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        let open = this.state.show_settings;
                        this.state.close_popovers();
                        this.state.show_settings = !open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child(settings_summary(st)),
                    )
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("⚙"),
                    ),
            );
        }

        footer = footer.child(div().flex_1());

        if inflight {
            footer = footer.child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XXS))
                    .child("Generating…"),
            );
        }

        footer = footer
            .child(
                div()
                    .text_color(if insufficient { Status::ERROR } else { Text::TERTIARY })
                    .text_size(px(FontSize::XS))
                    .child(model_catalog::format_usd(cost)),
            )
            .child(
                div()
                    .id("btn-generate")
                    .w(px(28.0))
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .cursor_pointer()
                    .bg(if submit_enabled {
                        Accent::PRIMARY
                    } else {
                        Background::PROMINENT
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.submit(cx);
                    }))
                    .child(
                        div()
                            .text_size(px(FontSize::SM_MD))
                            .text_color(if submit_enabled {
                                Background::BASE
                            } else {
                                Text::MUTED
                            })
                            .child("✦"),
                    ),
            );

        prompt_box = prompt_box.child(footer);

        // ── Model picker dropdown (catalog-driven) ──
        if st.show_model_picker {
            let mut list = div()
                .id("gen-model-list")
                .flex()
                .flex_col()
                .max_h(px(GenerationPanel::LOADING_HEIGHT))
                .overflow_y_scroll();
            for m in models_for(selected.kind()) {
                let is_selected = m.id == st.selected_model_id();
                let locked = !model_catalog::model_available(st.is_paid_plan, m.paid_only);
                let model_id = m.id;
                list = list.child(
                    div()
                        .id(SharedString::from(format!("gen-model-{}", m.id)))
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(Spacing::SM_MD))
                        .py(px(Spacing::XS))
                        .gap(px(Spacing::SM))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.state.select_model(model_id);
                            this.state.show_model_picker = false;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_color(if is_selected {
                                    Accent::PRIMARY
                                } else {
                                    Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.0 }
                                })
                                .text_size(px(FontSize::XS))
                                .child("✓"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::SM))
                                .child(m.display_name),
                        )
                        .when(locked, |el| {
                            el.child(
                                div()
                                    .px(px(Spacing::XS))
                                    .rounded(px(Radius::XS))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .text_color(Accent::PRIMARY)
                                    .text_size(px(FontSize::XXS))
                                    .child("Upgrade"),
                            )
                        }),
                );
            }
            prompt_box = prompt_box.child(
                div()
                    .id("gen-model-picker-dropdown")
                    .border_t_1()
                    .border_color(BorderColors::SUBTLE)
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(list),
            );
        }

        // ── Voice picker dropdown (audio caps.voices) ──
        if st.show_voice_picker {
            if let Some(voices) = &voices {
                let mut list = div()
                    .id("gen-voice-list")
                    .flex()
                    .flex_col()
                    .max_h(px(GenerationPanel::LOADING_HEIGHT))
                    .overflow_y_scroll();
                for voice in voices {
                    let is_selected = *voice == st.params.voice;
                    let v: &'static str = voice;
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("gen-voice-{voice}")))
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::SM_MD))
                            .py(px(Spacing::XS))
                            .gap(px(Spacing::SM))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.state.params.voice = v.to_string();
                                this.state.show_voice_picker = false;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .text_color(if is_selected {
                                        Accent::PRIMARY
                                    } else {
                                        Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.0 }
                                    })
                                    .text_size(px(FontSize::XS))
                                    .child("✓"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::SM))
                                    .child(*voice),
                            ),
                    );
                }
                prompt_box = prompt_box.child(
                    div()
                        .id("gen-voice-picker-dropdown")
                        .border_t_1()
                        .border_color(BorderColors::SUBTLE)
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(list),
                );
            }
        }

        // ── Settings popover (caps-driven, anchored above the footer) ──
        if st.show_settings {
            prompt_box = prompt_box.child(self.render_settings_popover(st, cx));
        }

        prompt_box
    }

    /// Swift `settingsPopoverContent`: pickers + toggles from the model caps.
    fn render_settings_popover(
        &self,
        st: &GenerationState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut popover = div()
            .id("gen-settings-popover")
            .absolute()
            .bottom(px(34.0))
            .left(px(Spacing::SM_MD))
            .w(px(220.0))
            .rounded(px(Radius::MD))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .shadow_lg()
            .p(px(Spacing::MD))
            .flex()
            .flex_col()
            .gap(px(Spacing::SM))
            .on_click(|_, _, cx| cx.stop_propagation());

        for field in visible_setting_fields(st) {
            let current = setting_value(st, field);
            let mut options_row = div().flex().flex_row().flex_wrap().gap(px(Spacing::XS));
            for (value, label) in setting_options(st, field) {
                let is_selected = value == current;
                options_row = options_row.child(
                    div()
                        .id(SharedString::from(format!(
                            "gen-set-{}-{value}",
                            field.label()
                        )))
                        .px(px(Spacing::SM))
                        .py(px(Spacing::XXS))
                        .rounded(px(Radius::XS_SM))
                        .cursor_pointer()
                        .bg(if is_selected {
                            Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.10 }
                        } else {
                            Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::SUBTLE }
                        })
                        .text_size(px(FontSize::XXS))
                        .text_color(if is_selected { Text::PRIMARY } else { Text::TERTIARY })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            apply_setting(&mut this.state, field, &value);
                            cx.notify();
                        }))
                        .child(label),
                );
            }
            popover = popover.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child(field.label()),
                    )
                    .child(options_row),
            );
        }

        for toggle in visible_setting_toggles(st) {
            let on = match toggle {
                SettingToggle::Instrumental => st.params.instrumental,
                SettingToggle::GenerateAudio => st.params.generate_audio,
            };
            popover = popover.child(
                div()
                    .id(SharedString::from(format!("gen-toggle-{}", toggle.label())))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        match toggle {
                            SettingToggle::Instrumental => {
                                this.state.params.instrumental = !this.state.params.instrumental
                            }
                            SettingToggle::GenerateAudio => {
                                this.state.params.generate_audio =
                                    !this.state.params.generate_audio
                            }
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(if on { Accent::PRIMARY } else { Text::MUTED })
                            .text_size(px(FontSize::XS))
                            .child(if on { "◉" } else { "◯" }),
                    )
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child(toggle.label()),
                    ),
            );
        }

        popover
    }
}

/// Credit top-off popover (CreditActionsPopover in Swift).
/// Paid users: dollar-amount input + Buy button. Free users: upgrade prompt.
fn credit_popover(is_paid: bool, cx: &mut Context<GenerationView>) -> impl IntoElement {
    div()
        .id("credit-popover")
        .absolute()
        .bottom(px(28.0))
        .right(px(0.0))
        .w(px(220.0))
        .rounded(px(Radius::MD))
        .bg(Background::RAISED)
        .border_1()
        .border_color(BorderColors::PRIMARY)
        .shadow_lg()
        .p(px(Spacing::MD))
        .flex()
        .flex_col()
        .gap(px(Spacing::SM))
        .on_click(|_, _, cx| cx.stop_propagation())
        .when(is_paid, |el| {
            el.child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM))
                    .child("Add credits"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .flex_1()
                            .h(px(28.0))
                            .px(px(Spacing::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .rounded(px(Radius::SM))
                            .flex()
                            .items_center()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("$10.00"),
                    )
                    .child(
                        div()
                            .id("credit-buy-btn")
                            .px(px(Spacing::SM))
                            .h(px(28.0))
                            .rounded(px(Radius::SM))
                            .bg(Accent::PRIMARY)
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.state.show_credit_popover = false;
                                cx.notify();
                            }))
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::SM))
                            .child("Buy"),
                    ),
            )
        })
        .when(!is_paid, |el| {
            el.child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child("Upgrade to add credits"),
            )
            .child(
                div()
                    .id("credit-upgrade-btn")
                    .w_full()
                    .px(px(Spacing::MD))
                    .py(px(Spacing::XS))
                    .rounded(px(Radius::SM))
                    .bg(Accent::PRIMARY)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.state.show_credit_popover = false;
                        cx.notify();
                    }))
                    .text_color(Background::BASE)
                    .text_size(px(FontSize::SM))
                    .child("Account settings"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video_state(model_id: &str) -> GenerationState {
        let mut st = GenerationState::default();
        st.select_type(GenerationType::Video);
        st.select_model(model_id);
        st
    }

    fn audio_state(model_id: &str) -> GenerationState {
        let mut st = GenerationState::default();
        st.select_type(GenerationType::Audio);
        st.select_model(model_id);
        st
    }

    fn image_state(model_id: &str) -> GenerationState {
        let mut st = GenerationState::default();
        st.select_type(GenerationType::Image);
        st.select_model(model_id);
        st
    }

    // ── 1.1 state + caps correction ─────────────────────────────────

    #[test]
    fn default_state_selects_first_plan_available_models() {
        let st = GenerationState::default();
        assert_eq!(st.video_model_id, "seedance-2");
        assert_eq!(st.image_model_id, "nano-banana-pro");
        assert_eq!(st.audio_model_id, "elevenlabs-tts-v3");
        assert_eq!(st.selected_model().id, "seedance-2");
        // Default params are valid for the default model.
        assert_eq!(st.params.duration, 5);
        assert_eq!(st.params.resolution, "1080p");
        assert_eq!(st.params.aspect_ratio, "16:9");
    }

    #[test]
    fn switching_models_re_derives_invalid_settings() {
        // Spec scenario: a model whose caps lack the chosen duration falls
        // back to that model's default.
        let mut st = video_state("veo3.1");
        assert_eq!(st.params.duration, 4, "5 is not a Veo duration → first (4)");
        st.params.duration = 8;
        st.select_model("grok-imagine-video");
        assert_eq!(st.params.duration, 8, "8 valid for Grok (6..=15) → kept");
        st.select_model("veo3.1-lite");
        assert_eq!(st.params.duration, 8, "8 valid for Veo → kept");
        st.params.resolution = "4k".into();
        // Re-selecting the SAME model is a no-op (F3) — switching to a
        // different model without 4k re-derives.
        st.select_model("veo3.1-lite");
        assert_eq!(st.params.resolution, "4k", "same-model reselect is a no-op");
        st.select_model("seedance-2");
        assert_eq!(st.params.resolution, "480p", "4k unsupported → first");
    }

    #[test]
    fn select_type_audio_applies_voice_default() {
        let mut st = GenerationState::default();
        st.select_type(GenerationType::Audio);
        assert_eq!(st.params.voice, "Rachel");
        st.select_model("gemini-3.1-flash-tts");
        assert_eq!(st.params.voice, "Kore");
        st.select_model("elevenlabs-music");
        assert_eq!(st.params.voice, "", "music model has no voices");
        assert_eq!(st.params.audio_duration, 30, "30 is a valid music duration");
    }

    #[test]
    fn select_model_resets_video_ref_state() {
        let mut st = video_state("seedance-2");
        st.use_first_last = false;
        st.references.push(RefAsset {
            id: "a".into(),
            kind: ClipType::Image,
        });
        st.select_model("kling-o3");
        assert!(st.use_first_last);
        assert!(st.references.is_empty());
    }

    #[test]
    fn select_type_clears_references_and_status() {
        let mut st = video_state("seedance-2");
        st.first_frame = Some("f".into());
        st.status = Some("boom".into());
        st.select_type(GenerationType::Image);
        assert!(st.first_frame.is_none());
        assert!(st.status.is_none());
    }

    // ── 1.2 cost + credit gating ────────────────────────────────────

    #[test]
    fn estimated_cost_uses_selected_params() {
        let mut st = video_state("seedance-2");
        st.prompt = "a fox".into();
        // 5s @1080p = 5 × 0.68
        assert!((estimated_cost(&st).unwrap() - 3.40).abs() < 1e-9);
        apply_setting(&mut st, SettingField::Resolution, "720p");
        assert!((estimated_cost(&st).unwrap() - 1.512).abs() < 1e-9);

        let mut ist = image_state("gpt-image-2");
        ist.prompt = "a sunset".into();
        // sanitize picked first resolution 1024x768 + kept quality "high"
        assert!((estimated_cost(&ist).unwrap() - 0.15).abs() < 1e-9);

        let mut ast = audio_state("elevenlabs-tts-v3");
        ast.prompt = "x".repeat(100);
        assert!((estimated_cost(&ast).unwrap() - 0.01).abs() < 1e-9);
        ast.prompt.clear();
        assert_eq!(estimated_cost(&ast), None, "empty TTS prompt → no estimate");
    }

    #[test]
    fn edit_models_estimate_none_without_source() {
        let st = video_state("kling-o3-edit");
        assert_eq!(estimated_cost(&st), None);
    }

    #[test]
    fn insufficient_credits_disables_generate_with_message() {
        // Spec scenario: estimate exceeds credits_remaining.
        let mut st = video_state("seedance-2");
        st.prompt = "a fox".into();
        st.credits_remaining = Some(1.0);
        let cost = estimated_cost(&st).unwrap();
        assert!(cost > 1.0);
        assert!(has_insufficient_credits(Some(cost), st.credits_remaining));
        assert!(!can_submit(&st), "generate disabled on shortfall");
        let msg = credit_shortfall_message(cost, 1.0);
        assert_eq!(msg, "$3.40 needed. Only $1.00 remaining.");

        st.credits_remaining = None;
        assert!(can_submit(&st), "unknown balance never blocks");
        st.credits_remaining = Some(1_250.0);
        assert!(can_submit(&st));
    }

    #[test]
    fn can_afford_quadrants() {
        assert!(can_afford(None, None));
        assert!(can_afford(Some(5.0), None));
        assert!(can_afford(Some(5.0), Some(5.0)));
        assert!(!can_afford(Some(5.01), Some(5.0)));
        assert!(can_afford(None, Some(0.5)));
        assert!(!can_afford(None, Some(0.0)));
    }

    // ── can_submit rules ────────────────────────────────────────────

    #[test]
    fn can_submit_requires_prompt() {
        let mut st = video_state("seedance-2");
        assert!(!can_submit(&st), "empty prompt");
        st.prompt = "   ".into();
        assert!(!can_submit(&st), "whitespace prompt");
        st.prompt = "a fox".into();
        assert!(can_submit(&st));
    }

    #[test]
    fn can_submit_audio_min_prompt_length() {
        let mut st = audio_state("minimax-music-v2.6");
        st.prompt = "short".into();
        assert!(!can_submit(&st), "below min 10 chars");
        st.prompt = "lofi hip hop beat".into();
        assert!(can_submit(&st));
    }

    #[test]
    fn can_submit_exclusive_reference_mode_needs_refs() {
        let mut st = video_state("seedance-2");
        st.prompt = "a fox".into();
        st.use_first_last = false;
        assert!(!can_submit(&st), "reference mode with no refs");
        st.references.push(RefAsset {
            id: "a".into(),
            kind: ClipType::Image,
        });
        assert!(can_submit(&st));
    }

    #[test]
    fn can_submit_edit_models_blocked_without_source() {
        let mut st = video_state("kling-o3-edit");
        st.prompt = "restyle".into();
        assert!(!can_submit(&st));
    }

    // ── settings model ──────────────────────────────────────────────

    #[test]
    fn setting_fields_derive_from_caps() {
        use SettingField::*;
        assert_eq!(
            visible_setting_fields(&video_state("seedance-2")),
            vec![Duration, AspectRatio, Resolution]
        );
        assert_eq!(
            visible_setting_fields(&image_state("gpt-image-2")),
            vec![Resolution, Quality],
            "GPT has no aspect list and maxImages 1"
        );
        assert_eq!(
            visible_setting_fields(&image_state("nano-banana-pro")),
            vec![AspectRatio, Resolution, Count]
        );
        assert_eq!(
            visible_setting_fields(&audio_state("elevenlabs-music")),
            vec![AudioDuration]
        );
        assert!(visible_setting_fields(&audio_state("elevenlabs-tts-v3")).is_empty());
        assert!(visible_setting_fields(&video_state("kling-o3-edit")).is_empty());
    }

    #[test]
    fn setting_toggles_derive_from_caps() {
        assert_eq!(
            visible_setting_toggles(&video_state("kling-o3")),
            vec![SettingToggle::GenerateAudio]
        );
        assert!(visible_setting_toggles(&video_state("seedance-2")).is_empty());
        assert_eq!(
            visible_setting_toggles(&audio_state("minimax-music-v2.6")),
            vec![SettingToggle::Instrumental]
        );
    }

    #[test]
    fn has_any_settings_gates_gear() {
        assert!(has_any_settings(&video_state("seedance-2")));
        assert!(has_any_settings(&audio_state("elevenlabs-music")));
        assert!(!has_any_settings(&audio_state("elevenlabs-tts-v3")));
        assert!(!has_any_settings(&video_state("kling-o3-edit")));
    }

    #[test]
    fn setting_options_and_apply() {
        let mut st = video_state("veo3.1");
        let durations = setting_options(&st, SettingField::Duration);
        assert_eq!(
            durations,
            vec![
                ("4".to_string(), "4s".to_string()),
                ("6".to_string(), "6s".to_string()),
                ("8".to_string(), "8s".to_string())
            ]
        );
        apply_setting(&mut st, SettingField::Duration, "6");
        assert_eq!(st.params.duration, 6);
        assert_eq!(setting_value(&st, SettingField::Duration), "6");

        let ist = image_state("gpt-image-2");
        let resolutions = setting_options(&ist, SettingField::Resolution);
        assert_eq!(resolutions[1], ("1024x1024".to_string(), "Square".to_string()));
        assert_eq!(
            resolutions[5],
            ("3840x2160".to_string(), "Landscape 4K".to_string())
        );
        let qualities = setting_options(&ist, SettingField::Quality);
        assert_eq!(qualities[0], ("low".to_string(), "Low".to_string()));

        let mut nst = image_state("nano-banana-pro");
        let counts = setting_options(&nst, SettingField::Count);
        assert_eq!(counts.len(), 4);
        apply_setting(&mut nst, SettingField::Count, "3");
        assert_eq!(nst.params.num_images, 3);
    }

    #[test]
    fn settings_summary_strings() {
        let st = video_state("seedance-2");
        assert_eq!(settings_summary(&st), "1080p · 5s · 16:9");

        let ist = image_state("gpt-image-2");
        assert_eq!(settings_summary(&ist), "Landscape · high");

        let mut ast = audio_state("elevenlabs-music");
        assert_eq!(settings_summary(&ast), "30s");
        ast.params.instrumental = true;
        assert_eq!(settings_summary(&ast), "30s · Instrumental");

        let tts = audio_state("elevenlabs-tts-v3");
        assert_eq!(settings_summary(&tts), "Settings");
    }

    // ── reference caps ──────────────────────────────────────────────

    #[test]
    fn reference_caps_enforced_per_kind_and_total() {
        let mut st = video_state("kling-v3");
        st.use_first_last = true; // kling is not exclusive; refs always allowed
        for i in 0..3 {
            let id = format!("img{i}");
            assert!(can_add_reference(&st, &id, ClipType::Image).is_ok());
            st.references.push(RefAsset {
                id,
                kind: ClipType::Image,
            });
        }
        assert!(
            can_add_reference(&st, "img9", ClipType::Image)
                .unwrap_err()
                .contains("limit reached (3)"),
        );
        assert!(
            can_add_reference(&st, "vid1", ClipType::Video)
                .unwrap_err()
                .contains("doesn't accept video references"),
        );
        assert!(
            can_add_reference(&st, "img0", ClipType::Image)
                .unwrap_err()
                .contains("Already"),
        );
        assert!(ref_cap_reached(&st));
        assert_eq!(ref_counter_label(&st), "3/3");
    }

    #[test]
    fn seedance_total_reference_cap() {
        let mut st = video_state("seedance-2");
        st.use_first_last = false;
        for i in 0..9 {
            st.references.push(RefAsset {
                id: format!("i{i}"),
                kind: ClipType::Image,
            });
        }
        for i in 0..3 {
            st.references.push(RefAsset {
                id: format!("v{i}"),
                kind: ClipType::Video,
            });
        }
        assert_eq!(st.references.len(), 12);
        assert!(
            can_add_reference(&st, "a1", ClipType::Audio)
                .unwrap_err()
                .contains("limit reached (12)"),
            "total cap hits before the per-kind audio cap"
        );
        assert!(ref_cap_reached(&st));
    }

    #[test]
    fn image_references_gated_by_support() {
        let st = image_state("nano-banana-pro");
        assert!(can_add_reference(&st, "a", ClipType::Image).is_ok());
        assert!(can_add_reference(&st, "a", ClipType::Video).is_err());
        let rst = image_state("recraft-v4");
        assert!(can_add_reference(&rst, "a", ClipType::Image)
            .unwrap_err()
            .contains("doesn't accept reference images"));
    }

    #[test]
    fn accepted_kinds_per_slot() {
        let st = video_state("seedance-2");
        assert_eq!(
            accepted_kinds(&st, RefSlot::FirstFrame),
            vec![ClipType::Image]
        );
        assert_eq!(
            accepted_kinds(&st, RefSlot::Reference),
            vec![ClipType::Image, ClipType::Video, ClipType::Audio]
        );
        let kst = video_state("kling-v3");
        assert_eq!(accepted_kinds(&kst, RefSlot::Reference), vec![ClipType::Image]);
        let ist = image_state("recraft-v4");
        assert!(accepted_kinds(&ist, RefSlot::Reference).is_empty());
    }

    // ── drop validation (drag-drop; mirrors click-to-pick rules) ────

    #[test]
    fn drop_rejection_matches_accepted_kinds() {
        let st = video_state("seedance-2");
        // Frame slots take images only.
        assert_eq!(
            drop_rejection_message(&st, RefSlot::FirstFrame, ClipType::Image),
            None
        );
        assert_eq!(
            drop_rejection_message(&st, RefSlot::FirstFrame, ClipType::Audio),
            Some("Drop image here.".to_string())
        );
        // Seedance references take image/video/audio.
        assert_eq!(
            drop_rejection_message(&st, RefSlot::Reference, ClipType::Video),
            None
        );
        // Image-only reference model names the accepted kind.
        let kst = video_state("kling-v3");
        assert_eq!(
            drop_rejection_message(&kst, RefSlot::Reference, ClipType::Audio),
            Some("Drop image here.".to_string())
        );
    }

    #[test]
    fn drop_rejection_on_refless_model() {
        // A model with no reference support rejects every kind by name.
        let ist = image_state("recraft-v4");
        let msg = drop_rejection_message(&ist, RefSlot::Reference, ClipType::Image);
        assert!(
            msg.as_deref().is_some_and(|m| m.contains("doesn't accept")),
            "msg={msg:?}"
        );
    }

    // ── reference layout ────────────────────────────────────────────

    #[test]
    fn reference_layout_per_model() {
        let veo = video_state("veo3.1");
        assert!(shows_frame_strip(&veo) && !shows_ref_sections(&veo));
        assert!(!shows_mode_toggle(&veo));

        let kling = video_state("kling-o3");
        assert!(shows_frame_strip(&kling) && shows_ref_sections(&kling));

        let mut seedance = video_state("seedance-2");
        assert!(shows_mode_toggle(&seedance));
        assert!(shows_frame_strip(&seedance) && !shows_ref_sections(&seedance));
        seedance.use_first_last = false;
        assert!(!shows_frame_strip(&seedance) && shows_ref_sections(&seedance));

        let edit = video_state("kling-o3-edit");
        assert!(!shows_frame_strip(&edit) && !shows_ref_sections(&edit));
        assert!(!shows_mode_toggle(&edit));

        assert!(shows_image_refs(&image_state("nano-banana-pro")));
        assert!(!shows_image_refs(&image_state("recraft-v4")));

        let audio = audio_state("elevenlabs-tts-v3");
        assert!(!shows_frame_strip(&audio) && !shows_ref_sections(&audio));
        assert!(!shows_image_refs(&audio));
    }

    // ── submission path ─────────────────────────────────────────────

    #[test]
    fn build_generation_input_video_full() {
        let mut st = video_state("kling-o3");
        st.prompt = "a fox running".into();
        st.params.duration = 10;
        st.params.resolution = "4k".into();
        st.params.generate_audio = false;
        st.first_frame = Some("f1".into());
        st.last_frame = Some("l1".into());
        st.references = vec![
            RefAsset { id: "ri".into(), kind: ClipType::Image },
            RefAsset { id: "rv".into(), kind: ClipType::Video },
        ];
        let input = build_generation_input(&st);
        assert_eq!(input.prompt, "a fox running");
        assert_eq!(input.model, "kling-o3");
        assert_eq!(input.duration, 10);
        assert_eq!(input.aspect_ratio, "16:9");
        assert_eq!(input.resolution.as_deref(), Some("4k"));
        assert_eq!(input.generate_audio, Some(false));
        assert_eq!(
            input.image_url_asset_ids,
            Some(vec!["f1".to_string(), "l1".to_string()])
        );
        assert_eq!(input.reference_image_asset_ids, Some(vec!["ri".to_string()]));
        assert_eq!(input.reference_video_asset_ids, Some(vec!["rv".to_string()]));
        assert_eq!(input.reference_audio_asset_ids, None);
        assert_eq!(input.quality, None);
        assert_eq!(input.num_images, None);
    }

    #[test]
    fn build_generation_input_exclusive_mode_drops_hidden_side() {
        let mut st = video_state("seedance-2");
        st.prompt = "p".into();
        st.first_frame = Some("f1".into());
        st.references.push(RefAsset { id: "r1".into(), kind: ClipType::Image });
        st.use_first_last = true;
        let input = build_generation_input(&st);
        assert_eq!(input.image_url_asset_ids, Some(vec!["f1".to_string()]));
        assert_eq!(input.reference_image_asset_ids, None, "refs hidden in frames mode");
        st.use_first_last = false;
        let input = build_generation_input(&st);
        assert_eq!(input.image_url_asset_ids, None, "frames hidden in refs mode");
        assert_eq!(input.reference_image_asset_ids, Some(vec!["r1".to_string()]));
    }

    #[test]
    fn build_generation_input_audio_and_image() {
        let mut ast = audio_state("gemini-3.1-flash-tts");
        ast.prompt = "hello world".into();
        ast.style_instructions = "calm".into();
        ast.lyrics = "ignored".into();
        let input = build_generation_input(&ast);
        assert_eq!(input.voice.as_deref(), Some("Kore"));
        assert_eq!(input.style_instructions.as_deref(), Some("calm"));
        assert_eq!(input.lyrics, None, "TTS doesn't support lyrics");
        assert_eq!(input.duration, 0, "no duration caps");
        assert_eq!(input.instrumental, None);

        let mut ist = image_state("nano-banana-pro");
        ist.prompt = "a sunset".into();
        ist.params.num_images = 3;
        ist.references.push(RefAsset { id: "r1".into(), kind: ClipType::Image });
        let input = build_generation_input(&ist);
        assert_eq!(input.num_images, Some(3));
        assert_eq!(input.quality, None, "nano has no qualities");
        assert_eq!(input.image_url_asset_ids, Some(vec!["r1".to_string()]));
    }

    #[test]
    fn generation_tool_call_routes_by_kind_and_category() {
        let mut vst = video_state("seedance-2");
        vst.prompt = "a fox".into();
        let (tool, args) = generation_tool_call(&vst);
        assert_eq!(tool, "generate_video");
        assert_eq!(args["prompt"], "a fox");
        assert_eq!(args["model"], "seedance-2");
        assert_eq!(args["duration"], 5.0);
        assert_eq!(args["resolution"], "1080p");

        let (tool, args) = generation_tool_call(&image_state("gpt-image-2"));
        assert_eq!(tool, "generate_image");
        assert_eq!(args["quality"], "high");
        assert!(args.get("duration").is_none());

        let mut tts = audio_state("elevenlabs-tts-v3");
        tts.prompt = "say this".into();
        let (tool, args) = generation_tool_call(&tts);
        assert_eq!(tool, "generate_audio");
        assert_eq!(args["voice"], "Rachel");
        assert!(args.get("duration").is_none(), "TTS has no duration");

        let mut mm = audio_state("minimax-music-v2.6");
        mm.prompt = "lofi hip hop".into();
        mm.params.instrumental = true;
        let (tool, args) = generation_tool_call(&mm);
        assert_eq!(tool, "generate_audio");
        assert_eq!(args["instrumental"], true);
        assert!(args.get("duration").is_none(), "minimax has no duration caps");

        let mut em = audio_state("elevenlabs-music");
        em.prompt = "orchestral".into();
        let (tool, args) = generation_tool_call(&em);
        assert_eq!(tool, "generate_audio");
        assert_eq!(args["duration"], 30.0);
    }

    #[test]
    fn interpret_submission_maps_outcomes() {
        assert_eq!(
            interpret_submission(&Err("boom".into())),
            SubmitOutcome::Failed("boom".into())
        );
        let queued = serde_json::json!({"content": [{"type": "text", "text": "Queued ok"}]});
        assert_eq!(
            interpret_submission(&Ok(queued)),
            SubmitOutcome::Queued("Queued ok".into())
        );
        let failed = serde_json::json!({
            "content": [{"type": "text", "text": "some other error"}],
            "isError": true,
        });
        assert_eq!(
            interpret_submission(&Ok(failed)),
            SubmitOutcome::Failed("some other error".into())
        );
    }

    #[test]
    fn executor_stub_maps_to_unavailable() {
        // Pins the marker-text coupling with agent_contract's generate stubs:
        // no GenerationBackend → explicit unavailable state, never a fake spinner.
        let mut exec = agent_contract::ToolExecutor::new(
            core_model::Timeline::default(),
            core_model::MediaManifest::default(),
        );
        let mut st = video_state("seedance-2");
        st.prompt = "a fox".into();
        let (tool, args) = generation_tool_call(&st);
        let result = exec.execute(tool, &args);
        assert_eq!(interpret_submission(&result), SubmitOutcome::Unavailable);
    }

    #[test]
    fn inflight_derives_from_manifest_status() {
        let manifest: core_model::MediaManifest = serde_json::from_str(
            r#"{"version":1,"entries":[
                {"id":"m1","name":"a","type":"video","source":{"external":{"absolutePath":"/x"}},"duration":1.0,"generationStatus":"generating"}
            ]}"#,
        )
        .unwrap();
        assert!(has_inflight_generation(&manifest));
        let idle: core_model::MediaManifest = serde_json::from_str(
            r#"{"version":1,"entries":[
                {"id":"m1","name":"a","type":"video","source":{"external":{"absolutePath":"/x"}},"duration":1.0,"generationStatus":"none"},
                {"id":"m2","name":"b","type":"video","source":{"external":{"absolutePath":"/y"}},"duration":1.0}
            ]}"#,
        )
        .unwrap();
        assert!(!has_inflight_generation(&idle));
    }

    #[test]
    fn model_rows_and_gating() {
        let ids: Vec<&str> = models_for(ModelKind::Video).iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), 10);
        assert_eq!(ids[0], "seedance-2");
        // No paid-only entries in the transcribed catalog; the lock path is pure.
        let st = GenerationState::default();
        assert!(!model_locked(&st));
    }

    #[test]
    fn close_popovers_reports_and_clears() {
        let mut st = GenerationState::default();
        assert!(!st.close_popovers());
        st.show_settings = true;
        st.asset_picker = Some(RefSlot::Reference);
        assert!(st.close_popovers());
        assert!(!st.show_settings && st.asset_picker.is_none());
    }

    #[test]
    fn assign_reference_routes_slots_and_caps() {
        let mut st = video_state("kling-v3");
        st.assign_reference(RefSlot::FirstFrame, "f1", ClipType::Image);
        assert_eq!(st.first_frame.as_deref(), Some("f1"));
        st.assign_reference(RefSlot::Reference, "r1", ClipType::Image);
        assert_eq!(st.references.len(), 1);
        st.assign_reference(RefSlot::Reference, "r1", ClipType::Image);
        assert_eq!(st.references.len(), 1, "duplicate rejected");
        assert!(st.status.is_some(), "rejection surfaces a status line");
    }
}
