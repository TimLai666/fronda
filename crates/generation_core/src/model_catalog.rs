//! Static generation model catalog (agent-visible), mirroring the upstream
//! Swift catalog field-for-field.
//!
//! Transcription source: the last in-repo hardcoded catalog —
//! `Sources/PalmierPro/Generation/Fal/{Video,Image,Audio}ModelConfig.swift`
//! at upstream `9dfde8d^` (the lists were deleted when the catalog moved
//! server-side; the current client decodes the same shape from `models:list`).
//! `paid_only` mirrors upstream #249's `CatalogEntry.paidOnly`; every
//! transcribed entry is `false`, matching Swift's decode default (the in-repo
//! data predates the flag and the backend values are not in this repo).
//! Fal endpoint resolvers/payload builders are backend plumbing, not catalog
//! data, and are not carried.

use crate::ModelKind;

#[derive(Debug, Clone, PartialEq)]
pub struct ModelConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub paid_only: bool,
    pub caps: ModelCaps,
}

impl ModelConfig {
    pub fn kind(&self) -> ModelKind {
        match self.caps {
            ModelCaps::Video(_) => ModelKind::Video,
            ModelCaps::Image(_) => ModelKind::Image,
            ModelCaps::Audio(_) => ModelKind::Audio,
        }
    }

    pub fn kind_str(&self) -> &'static str {
        match self.caps {
            ModelCaps::Video(_) => "video",
            ModelCaps::Image(_) => "image",
            ModelCaps::Audio(_) => "audio",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelCaps {
    Video(VideoCaps),
    Image(ImageCaps),
    Audio(AudioCaps),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoCaps {
    pub durations: Vec<i64>,
    pub resolutions: Option<Vec<&'static str>>,
    pub aspect_ratios: Vec<&'static str>,
    pub supports_first_frame: bool,
    pub supports_last_frame: bool,
    pub max_reference_images: i64,
    pub max_reference_videos: i64,
    pub max_reference_audios: i64,
    pub max_total_references: Option<i64>,
    pub max_combined_video_ref_seconds: Option<f64>,
    pub max_combined_audio_ref_seconds: Option<f64>,
    pub frames_and_references_exclusive: bool,
    pub reference_tag_noun: &'static str,
    pub requires_source_video: bool,
    /// USD per output second, keyed by resolution ("" = flat).
    pub price_per_second: Vec<(&'static str, f64)>,
    /// Audio-off price multiplier per resolution; "" key is the default.
    pub audio_discount_rate: Option<Vec<(&'static str, f64)>>,
}

impl Default for VideoCaps {
    // Mirrors the Swift initializer defaults.
    fn default() -> Self {
        Self {
            durations: Vec::new(),
            resolutions: None,
            aspect_ratios: Vec::new(),
            supports_first_frame: true,
            supports_last_frame: false,
            max_reference_images: 0,
            max_reference_videos: 0,
            max_reference_audios: 0,
            max_total_references: None,
            max_combined_video_ref_seconds: None,
            max_combined_audio_ref_seconds: None,
            frames_and_references_exclusive: false,
            reference_tag_noun: "Image",
            requires_source_video: false,
            price_per_second: Vec::new(),
            audio_discount_rate: None,
        }
    }
}

impl VideoCaps {
    pub fn supports_references(&self) -> bool {
        self.max_reference_images > 0
            || self.max_reference_videos > 0
            || self.max_reference_audios > 0
    }

    /// Total reference count available across types. Used by agent tool info.
    pub fn max_references(&self) -> i64 {
        self.max_total_references.unwrap_or(
            self.max_reference_images + self.max_reference_videos + self.max_reference_audios,
        )
    }

    /// Audio-off price multiplier for a resolution; `""` key is the default
    /// (Swift `VideoModelConfig.audioDiscount(for:)`).
    pub fn audio_discount(&self, resolution: Option<&str>) -> Option<f64> {
        resolved_rate(self.audio_discount_rate.as_deref()?, resolution)
    }
}

// ── Cost estimation (Swift `CostEstimator` at 9dfde8d^, USD) ────
//
// The current upstream estimator is credits-based against the server catalog;
// this catalog carries the Fal-era USD prices, so the USD estimator is the
// matching math. Same structure, same lookup precedence.

fn resolved_rate(rates: &[(&'static str, f64)], key: Option<&str>) -> Option<f64> {
    if let Some(k) = key {
        if let Some((_, v)) = rates.iter().find(|(rk, _)| *rk == k) {
            return Some(*v);
        }
    }
    rates.iter().find(|(rk, _)| rk.is_empty()).map(|(_, v)| *v)
}

/// USD estimate for a video generation.
pub fn video_cost(
    caps: &VideoCaps,
    duration_seconds: i64,
    resolution: Option<&str>,
    generate_audio: bool,
) -> Option<f64> {
    if caps.price_per_second.is_empty() || duration_seconds <= 0 {
        return None;
    }
    let mut rate = resolved_rate(&caps.price_per_second, resolution)?;
    if !generate_audio {
        if let Some(discount) = caps.audio_discount(resolution) {
            rate *= discount;
        }
    }
    Some(rate * duration_seconds as f64)
}

/// USD estimate for an image generation.
pub fn image_cost(
    caps: &ImageCaps,
    resolution: Option<&str>,
    quality: Option<&str>,
    num_images: i64,
) -> Option<f64> {
    if caps.price_per_image.is_empty() {
        return None;
    }
    let count = num_images.max(1) as f64;
    // 2D matrix lookup first (e.g. GPT Image 2 varies on both axes).
    if let (Some(r), Some(q)) = (resolution, quality) {
        let key = format!("{r}|{q}");
        if let Some((_, price)) = caps.price_per_image.iter().find(|(k, _)| *k == key) {
            return Some(price * count);
        }
    }
    // Quality-only lookup when the model varies on quality but not resolution.
    if caps.qualities.is_some() {
        if let Some(q) = quality {
            if let Some((_, price)) = caps.price_per_image.iter().find(|(k, _)| *k == q) {
                return Some(price * count);
            }
        }
    }
    resolved_rate(&caps.price_per_image, resolution).map(|rate| rate * count)
}

/// USD estimate for an audio generation.
pub fn audio_cost(caps: &AudioCaps, prompt: &str, duration_seconds: Option<i64>) -> Option<f64> {
    match caps.pricing {
        AudioPricing::PerThousandChars(rate) => {
            let chars = prompt.chars().count();
            if chars == 0 {
                return None;
            }
            Some(rate * (chars as f64 / 1000.0))
        }
        AudioPricing::PerSecond(rate) => {
            let secs = duration_seconds?;
            if secs <= 0 {
                return None;
            }
            Some(rate * secs as f64)
        }
        AudioPricing::Flat(price) => Some(price),
        AudioPricing::Unknown => None,
    }
}

/// Swift `CostEstimator.format`: "—", "$0.00", "<$0.01", or "$X.XX".
pub fn format_usd(cost: Option<f64>) -> String {
    let Some(cost) = cost else {
        return "—".to_string();
    };
    if cost <= 0.0 {
        return "$0.00".to_string();
    }
    if cost < 0.01 {
        return "<$0.01".to_string();
    }
    format!("${cost:.2}")
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageCaps {
    pub resolutions: Option<Vec<&'static str>>,
    pub aspect_ratios: Vec<&'static str>,
    pub qualities: Option<Vec<&'static str>>,
    pub supports_image_reference: bool,
    pub max_images: i64,
    /// USD per image, keyed by the dimension the model varies on ("" = flat).
    pub price_per_image: Vec<(&'static str, f64)>,
}

impl Default for ImageCaps {
    fn default() -> Self {
        Self {
            resolutions: None,
            aspect_ratios: Vec::new(),
            qualities: None,
            supports_image_reference: false,
            max_images: 1,
            price_per_image: Vec::new(),
        }
    }
}

/// Swift `AudioModelConfig.Category`. Raw values are the wire category
/// strings (#294 made the enum String-backed and added cleanup/dubbing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCategory {
    Tts,
    Music,
    Sfx,
    Cleanup,
    Dubbing,
}

impl AudioCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioCategory::Tts => "tts",
            AudioCategory::Music => "music",
            AudioCategory::Sfx => "sfx",
            AudioCategory::Cleanup => "cleanup",
            AudioCategory::Dubbing => "dubbing",
        }
    }

    /// Swift `Category.label`.
    pub fn label(&self) -> &'static str {
        match self {
            AudioCategory::Tts => "Speech",
            AudioCategory::Music => "Music",
            AudioCategory::Sfx => "Sound Effects",
            AudioCategory::Cleanup => "Voice Cleanup",
            AudioCategory::Dubbing => "Dubbing",
        }
    }
}

/// Swift `AudioModelConfig.Input` raw values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioInput {
    Text,
    Audio,
    Video,
}

impl AudioInput {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioInput::Text => "text",
            AudioInput::Audio => "audio",
            AudioInput::Video => "video",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AudioPricing {
    /// USD per 1000 characters of prompt text (TTS).
    PerThousandChars(f64),
    /// USD per output second (music with duration param).
    PerSecond(f64),
    /// USD per generation, duration-agnostic.
    Flat(f64),
    /// Price unknown — estimator returns None.
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioCaps {
    pub category: AudioCategory,
    pub voices: Option<Vec<&'static str>>,
    pub default_voice: Option<&'static str>,
    pub supports_lyrics: bool,
    pub supports_instrumental: bool,
    pub supports_style_instructions: bool,
    pub durations: Option<Vec<i64>>,
    pub min_prompt_length: i64,
    pub pricing: AudioPricing,
    /// Video-to-audio span bounds (upstream #288). None = the defaults
    /// (1s .. 600s) apply; the Fal-era catalog carries no per-model values.
    pub min_seconds: Option<f64>,
    pub max_seconds: Option<f64>,
    /// Swift `caps.inputs` (#294 added "audio"). None = the "text" default.
    pub inputs: Option<Vec<AudioInput>>,
    /// Swift `caps.targetLanguages` / `caps.defaultTargetLanguage` (#294,
    /// dubbing). Server-catalog data; the static catalog carries none.
    pub target_languages: Option<Vec<&'static str>>,
    pub default_target_language: Option<&'static str>,
}

impl Default for AudioCaps {
    fn default() -> Self {
        Self {
            category: AudioCategory::Tts,
            voices: None,
            default_voice: None,
            supports_lyrics: false,
            supports_instrumental: false,
            supports_style_instructions: false,
            durations: None,
            min_prompt_length: 1,
            pricing: AudioPricing::Unknown,
            min_seconds: None,
            max_seconds: None,
            inputs: None,
            target_languages: None,
            default_target_language: None,
        }
    }
}

impl AudioCaps {
    /// Swift `AudioModelConfig.inputs`: `caps.inputs ?? ["text"]`.
    pub fn effective_inputs(&self) -> &[AudioInput] {
        const DEFAULT: &[AudioInput] = &[AudioInput::Text];
        self.inputs.as_deref().unwrap_or(DEFAULT)
    }

    /// Swift `acceptsSourceMedia`.
    pub fn accepts_source_media(&self) -> bool {
        let inputs = self.effective_inputs();
        inputs.contains(&AudioInput::Audio) || inputs.contains(&AudioInput::Video)
    }

    /// Swift `usesSourceURL`: cleanup/dubbing transform an uploaded source.
    pub fn uses_source_url(&self) -> bool {
        matches!(
            self.category,
            AudioCategory::Cleanup | AudioCategory::Dubbing
        )
    }

    /// Swift `acceptsSource(_:)`. Non-A/V clip types (image, text, lottie,
    /// sequence — plus Rust's shape) are never source media.
    pub fn accepts_source(&self, clip_type: core_model::ClipType) -> bool {
        match clip_type {
            core_model::ClipType::Audio => self.effective_inputs().contains(&AudioInput::Audio),
            core_model::ClipType::Video => self.effective_inputs().contains(&AudioInput::Video),
            _ => false,
        }
    }
}

/// Swift global `unsupportedValue(model:field:value:allowed:)`.
pub fn unsupported_value(display_name: &str, field: &str, value: &str, allowed: &[String]) -> String {
    format!(
        "{display_name} does not support {field} '{value}'. Valid: {}.",
        allowed.join(", ")
    )
}

/// Swift `AudioModelConfig.validate(spanSeconds:)` (#294 wording: "source
/// media"). Bounds default to Swift's `minSeconds ?? 1` / `maxSeconds ?? 600`.
pub fn audio_validate_span_seconds(
    display_name: &str,
    caps: &AudioCaps,
    span_seconds: f64,
) -> Option<String> {
    let s = span_seconds.round() as i64;
    let min = caps.min_seconds.unwrap_or(1.0).round() as i64;
    let max = caps.max_seconds.unwrap_or(600.0).round() as i64;
    if s < min {
        return Some(format!(
            "{display_name} needs at least {min}s of source media (selection is {s}s)."
        ));
    }
    if s > max {
        return Some(format!(
            "{display_name} accepts at most {max}s of source media (selection is {s}s)."
        ));
    }
    None
}

/// Swift `AudioModelConfig.validate(params:)` (#294: the prompt-length check
/// applies only to text-input models; targetLanguages gates dubbing).
pub fn audio_validate_params(
    display_name: &str,
    caps: &AudioCaps,
    params: &crate::generation_payload::AudioGenerationPayload,
) -> Option<String> {
    let prompt_len = params.prompt.trim().chars().count() as i64;
    if caps.effective_inputs().contains(&AudioInput::Text) && prompt_len < caps.min_prompt_length {
        return Some(format!(
            "{display_name} requires prompt ≥ {} characters (got {prompt_len}).",
            caps.min_prompt_length
        ));
    }
    if let (Some(allowed), Some(v)) = (&caps.voices, &params.voice) {
        if !v.is_empty() && !allowed.iter().any(|a| a == v) {
            let mut shown: Vec<String> = allowed.iter().take(6).map(|s| s.to_string()).collect();
            if allowed.len() > 6 {
                shown.push("…".to_string());
            }
            return Some(unsupported_value(display_name, "voice", v, &shown));
        }
    }
    if let (Some(allowed), Some(d)) = (&caps.durations, params.duration_seconds) {
        if !allowed.iter().any(|&a| a as f64 == d) {
            let shown: Vec<String> = allowed.iter().map(|a| format!("{a}s")).collect();
            return Some(unsupported_value(
                display_name,
                "duration",
                &format!("{}s", d as i64),
                &shown,
            ));
        }
    }
    if let Some(allowed) = &caps.target_languages {
        let lang = params.target_language.as_deref().unwrap_or("");
        if lang.is_empty() {
            return Some("Choose a target language.".to_string());
        }
        if !allowed.contains(&lang) {
            let shown: Vec<String> = allowed.iter().map(|s| s.to_string()).collect();
            return Some(unsupported_value(
                display_name,
                "target language",
                lang,
                &shown,
            ));
        }
    }
    None
}

// ── Gating (upstream #249) ──────────────────────────────────────

/// A model is available when the account is paid or the model is not paid-only.
pub fn model_available(is_paid: bool, paid_only: bool) -> bool {
    is_paid || !paid_only
}

/// Swift `requirePlan` error text.
pub fn require_plan_message(model_id: &str) -> String {
    format!(
        "Model '{model_id}' requires a paid plan. Pick a free model from list_models, \
         or tell the user to subscribe."
    )
}

/// Swift `defaultModelId` no-available-model error text.
pub fn no_available_model_message(kind: &str) -> String {
    format!("No {kind} model is available on the current plan. Tell the user to subscribe.")
}

pub fn model_by_id(id: &str) -> Option<&'static ModelConfig> {
    catalog().iter().find(|m| m.id == id)
}

/// First plan-available model of a kind (Swift `defaultModelId`).
pub fn default_model(kind: ModelKind, is_paid: bool) -> Result<&'static ModelConfig, String> {
    let kind_str = match kind {
        ModelKind::Video => "video",
        ModelKind::Image => "image",
        ModelKind::Audio => "audio",
        ModelKind::Upscale => "upscale",
    };
    catalog()
        .iter()
        .filter(|m| m.kind() == kind)
        .find(|m| model_available(is_paid, m.paid_only))
        .ok_or_else(|| no_available_model_message(kind_str))
}

// ── The catalog ─────────────────────────────────────────────────

const ELEVENLABS_VOICES: &[&str] = &[
    "Rachel",
    "Aria",
    "Roger",
    "Sarah",
    "Laura",
    "Charlie",
    "George",
    "Callum",
    "River",
    "Liam",
    "Charlotte",
    "Alice",
    "Matilda",
    "Will",
    "Jessica",
    "Eric",
    "Chris",
    "Brian",
    "Daniel",
    "Lily",
    "Bill",
];

const GEMINI_VOICES: &[&str] = &[
    "Kore",
    "Achernar",
    "Achird",
    "Algenib",
    "Algieba",
    "Alnilam",
    "Aoede",
    "Autonoe",
    "Callirrhoe",
    "Charon",
    "Despina",
    "Enceladus",
    "Erinome",
    "Fenrir",
    "Gacrux",
    "Iapetus",
    "Laomedeia",
    "Leda",
    "Orus",
    "Pulcherrima",
    "Puck",
    "Rasalgethi",
    "Sadachbia",
    "Sadaltager",
    "Schedar",
    "Sulafat",
    "Umbriel",
    "Vindemiatrix",
    "Zephyr",
    "Zubenelgenubi",
];

/// Kling V3/O3 share a duration range and pro/4k tiering (Swift `klingProOr4k`).
fn kling(
    id: &'static str,
    display_name: &'static str,
    max_reference_images: i64,
    price_per_second: Vec<(&'static str, f64)>,
    audio_discount_rate: Vec<(&'static str, f64)>,
) -> ModelConfig {
    ModelConfig {
        id,
        display_name,
        paid_only: false,
        caps: ModelCaps::Video(VideoCaps {
            durations: (3..=15).collect(),
            resolutions: Some(vec!["1080p", "4k"]),
            aspect_ratios: vec!["16:9", "9:16", "1:1"],
            supports_last_frame: true,
            max_reference_images,
            reference_tag_noun: "Element",
            price_per_second,
            audio_discount_rate: Some(audio_discount_rate),
            ..Default::default()
        }),
    }
}

fn veo(
    id: &'static str,
    display_name: &'static str,
    resolutions: Vec<&'static str>,
    price_per_second: Vec<(&'static str, f64)>,
) -> ModelConfig {
    ModelConfig {
        id,
        display_name,
        paid_only: false,
        caps: ModelCaps::Video(VideoCaps {
            durations: vec![4, 6, 8],
            resolutions: Some(resolutions),
            aspect_ratios: vec!["16:9", "9:16"],
            supports_last_frame: true,
            price_per_second,
            audio_discount_rate: Some(vec![("", 2.0 / 3.0)]),
            ..Default::default()
        }),
    }
}

fn seedance(
    id: &'static str,
    display_name: &'static str,
    resolutions: Vec<&'static str>,
    price_per_second: Vec<(&'static str, f64)>,
) -> ModelConfig {
    ModelConfig {
        id,
        display_name,
        paid_only: false,
        caps: ModelCaps::Video(VideoCaps {
            durations: (4..=15).collect(),
            resolutions: Some(resolutions),
            aspect_ratios: vec!["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"],
            supports_last_frame: true,
            max_reference_images: 9,
            max_reference_videos: 3,
            max_reference_audios: 3,
            max_total_references: Some(12),
            max_combined_video_ref_seconds: Some(15.0),
            max_combined_audio_ref_seconds: Some(15.0),
            frames_and_references_exclusive: true,
            price_per_second,
            ..Default::default()
        }),
    }
}

fn build_catalog() -> Vec<ModelConfig> {
    vec![
        // ── Video (Swift VideoModelConfig.allModels order) ──
        seedance(
            "seedance-2",
            "Seedance 2",
            vec!["480p", "720p", "1080p"],
            vec![("480p", 0.1345), ("720p", 0.3024), ("1080p", 0.68)],
        ),
        seedance(
            "seedance-2-fast",
            "Seedance 2 Fast",
            vec!["480p", "720p"],
            vec![("480p", 0.0843), ("720p", 0.2427)],
        ),
        kling(
            "kling-o3",
            "Kling O3",
            7,
            vec![("1080p", 0.14), ("4k", 0.42)],
            vec![("1080p", 0.8)],
        ),
        kling(
            "kling-v3",
            "Kling V3",
            3,
            vec![("1080p", 0.168), ("4k", 0.42)],
            vec![("1080p", 2.0 / 3.0)],
        ),
        veo(
            "veo3.1-fast",
            "Veo 3.1 Fast",
            vec!["720p", "1080p", "4k"],
            vec![("720p", 0.15), ("1080p", 0.15), ("4k", 0.35)],
        ),
        veo(
            "veo3.1",
            "Veo 3.1",
            vec!["720p", "1080p", "4k"],
            vec![("720p", 0.40), ("1080p", 0.40), ("4k", 0.60)],
        ),
        veo(
            "veo3.1-lite",
            "Veo 3.1 Lite",
            vec!["720p", "1080p"],
            vec![("720p", 0.05), ("1080p", 0.08)],
        ),
        ModelConfig {
            id: "grok-imagine-video",
            display_name: "Grok Imagine Video",
            paid_only: false,
            caps: ModelCaps::Video(VideoCaps {
                durations: (6..=15).collect(),
                resolutions: Some(vec!["480p", "720p"]),
                aspect_ratios: vec!["16:9", "9:16"],
                max_reference_images: 7,
                frames_and_references_exclusive: true,
                price_per_second: vec![("480p", 0.05), ("720p", 0.07)],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "kling-o3-edit",
            display_name: "Kling O3 Edit",
            paid_only: false,
            caps: ModelCaps::Video(VideoCaps {
                supports_first_frame: false,
                requires_source_video: true,
                price_per_second: vec![("", 0.168)],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "kling-v3-motion-control",
            display_name: "Kling V3 Motion Control",
            paid_only: false,
            caps: ModelCaps::Video(VideoCaps {
                supports_first_frame: false,
                max_reference_images: 1,
                requires_source_video: true,
                price_per_second: vec![("", 0.168)],
                ..Default::default()
            }),
        },
        // ── Image (Swift ImageModelConfig.allModels order) ──
        ModelConfig {
            id: "nano-banana-pro",
            display_name: "Nano Banana Pro",
            paid_only: false,
            caps: ModelCaps::Image(ImageCaps {
                resolutions: Some(vec!["2K", "4K"]),
                aspect_ratios: vec![
                    "auto", "21:9", "16:9", "3:2", "4:3", "5:4", "1:1", "4:5", "3:4", "2:3", "9:16",
                ],
                supports_image_reference: true,
                max_images: 4,
                price_per_image: vec![("2K", 0.15), ("4K", 0.30)],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "nano-banana-2",
            display_name: "Nano Banana 2",
            paid_only: false,
            caps: ModelCaps::Image(ImageCaps {
                resolutions: Some(vec!["2K", "4K"]),
                aspect_ratios: vec![
                    "auto", "21:9", "16:9", "3:2", "4:3", "5:4", "1:1", "4:5", "3:4", "2:3",
                    "9:16", "4:1", "1:4", "8:1", "1:8",
                ],
                supports_image_reference: true,
                max_images: 4,
                price_per_image: vec![("2K", 0.12), ("4K", 0.16)],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "gpt-image-2",
            display_name: "GPT Image 2",
            paid_only: false,
            caps: ModelCaps::Image(ImageCaps {
                resolutions: Some(vec![
                    "1024x768",
                    "1024x1024",
                    "1024x1536",
                    "1920x1080",
                    "2560x1440",
                    "3840x2160",
                ]),
                aspect_ratios: Vec::new(),
                qualities: Some(vec!["low", "medium", "high"]),
                supports_image_reference: true,
                max_images: 1,
                price_per_image: vec![
                    ("1024x768|low", 0.01),
                    ("1024x768|medium", 0.04),
                    ("1024x768|high", 0.15),
                    ("1024x1024|low", 0.01),
                    ("1024x1024|medium", 0.06),
                    ("1024x1024|high", 0.22),
                    ("1024x1536|low", 0.01),
                    ("1024x1536|medium", 0.05),
                    ("1024x1536|high", 0.17),
                    ("1920x1080|low", 0.01),
                    ("1920x1080|medium", 0.04),
                    ("1920x1080|high", 0.16),
                    ("2560x1440|low", 0.01),
                    ("2560x1440|medium", 0.06),
                    ("2560x1440|high", 0.23),
                    ("3840x2160|low", 0.02),
                    ("3840x2160|medium", 0.11),
                    ("3840x2160|high", 0.41),
                ],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "grok-imagine",
            display_name: "Grok Imagine",
            paid_only: false,
            caps: ModelCaps::Image(ImageCaps {
                aspect_ratios: vec![
                    "2:1", "20:9", "19.5:9", "16:9", "4:3", "3:2", "1:1", "2:3", "3:4", "9:16",
                    "9:19.5", "9:20", "1:2",
                ],
                supports_image_reference: true,
                max_images: 4,
                price_per_image: vec![("", 0.02)],
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "recraft-v4",
            display_name: "Recraft V4",
            paid_only: false,
            caps: ModelCaps::Image(ImageCaps {
                aspect_ratios: vec![
                    "square_hd",
                    "square",
                    "portrait_4_3",
                    "portrait_16_9",
                    "landscape_4_3",
                    "landscape_16_9",
                ],
                supports_image_reference: false,
                max_images: 4,
                price_per_image: vec![("", 0.25)],
                ..Default::default()
            }),
        },
        // ── Audio (Swift AudioModelConfig.allModels order) ──
        ModelConfig {
            id: "elevenlabs-tts-v3",
            display_name: "ElevenLabs v3 TTS",
            paid_only: false,
            caps: ModelCaps::Audio(AudioCaps {
                category: AudioCategory::Tts,
                voices: Some(ELEVENLABS_VOICES.to_vec()),
                default_voice: Some("Rachel"),
                pricing: AudioPricing::PerThousandChars(0.10),
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "gemini-3.1-flash-tts",
            display_name: "Gemini 3.1 Flash TTS",
            paid_only: false,
            caps: ModelCaps::Audio(AudioCaps {
                category: AudioCategory::Tts,
                voices: Some(GEMINI_VOICES.to_vec()),
                default_voice: Some("Kore"),
                supports_style_instructions: true,
                pricing: AudioPricing::PerThousandChars(0.03),
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "minimax-music-v2.6",
            display_name: "MiniMax Music 2.6",
            paid_only: false,
            caps: ModelCaps::Audio(AudioCaps {
                category: AudioCategory::Music,
                supports_lyrics: true,
                supports_instrumental: true,
                min_prompt_length: 10,
                pricing: AudioPricing::Flat(0.03),
                ..Default::default()
            }),
        },
        ModelConfig {
            id: "elevenlabs-music",
            display_name: "ElevenLabs Music",
            paid_only: false,
            caps: ModelCaps::Audio(AudioCaps {
                category: AudioCategory::Music,
                supports_instrumental: true,
                durations: Some(vec![15, 30, 60, 90, 120, 180]),
                pricing: AudioPricing::PerSecond(0.002),
                ..Default::default()
            }),
        },
    ]
}

/// The full model catalog, in upstream display order (video, image, audio).
pub fn catalog() -> &'static [ModelConfig] {
    static CATALOG: std::sync::OnceLock<Vec<ModelConfig>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

// ── Upscale catalog (Swift `UpscaleModelConfig` at 9dfde8d^) ────────
//
// Swift keeps upscalers as their own type, not a Video/Image/Audio caps
// variant — mirrored here (also keeps `ModelCaps` matches exhaustive
// elsewhere). `buildFalInput` payload closures are backend plumbing and
// are not carried, same rule as the other configs.

#[derive(Debug, Clone, PartialEq)]
pub struct UpscaleModelConfig {
    pub id: &'static str,
    pub display_name: &'static str,
    pub speed: &'static str,
    pub endpoint: &'static str,
    /// USD per source second.
    pub price_per_second: f64,
    pub p75_duration_seconds: i64,
    pub supported_types: &'static [core_model::ClipType],
}

impl UpscaleModelConfig {
    pub fn kind(&self) -> ModelKind {
        ModelKind::Upscale
    }

    /// Swift `supportedTypes.contains`.
    pub fn supports(&self, clip_type: core_model::ClipType) -> bool {
        self.supported_types.contains(&clip_type)
    }
}

/// All upscale models, in Swift `allModels` order (video, then image).
pub fn upscale_catalog() -> &'static [UpscaleModelConfig] {
    use core_model::ClipType;
    const UPSCALE_CATALOG: &[UpscaleModelConfig] = &[
        UpscaleModelConfig {
            id: "bytedance-upscaler",
            display_name: "Bytedance Upscaler",
            speed: "Fast",
            endpoint: "fal-ai/bytedance-upscaler/upscale/video",
            price_per_second: 0.0288,
            p75_duration_seconds: 130,
            supported_types: &[ClipType::Video],
        },
        UpscaleModelConfig {
            id: "seedvr-upscaler",
            display_name: "SeedVR2",
            speed: "Medium",
            endpoint: "fal-ai/seedvr/upscale/video",
            price_per_second: 0.062,
            p75_duration_seconds: 691,
            supported_types: &[ClipType::Video],
        },
        UpscaleModelConfig {
            id: "topaz-upscaler",
            display_name: "Topaz Upscale",
            speed: "Slow",
            endpoint: "fal-ai/topaz/upscale/video",
            price_per_second: 0.08,
            p75_duration_seconds: 65,
            supported_types: &[ClipType::Video],
        },
        UpscaleModelConfig {
            id: "seedvr-image-upscaler",
            display_name: "SeedVR2",
            speed: "Fast",
            endpoint: "fal-ai/seedvr/upscale/image",
            price_per_second: 0.04,
            p75_duration_seconds: 19,
            supported_types: &[ClipType::Image],
        },
        UpscaleModelConfig {
            id: "topaz-image-upscaler",
            display_name: "Topaz Upscale",
            speed: "Medium",
            endpoint: "fal-ai/topaz/upscale/image",
            price_per_second: 0.08,
            p75_duration_seconds: 24,
            supported_types: &[ClipType::Image],
        },
    ];
    UPSCALE_CATALOG
}

/// Swift `UpscaleModelConfig.models(for:)`.
pub fn upscale_models_for(clip_type: core_model::ClipType) -> Vec<&'static UpscaleModelConfig> {
    upscale_catalog()
        .iter()
        .filter(|m| m.supports(clip_type))
        .collect()
}

pub fn upscale_model_by_id(id: &str) -> Option<&'static UpscaleModelConfig> {
    upscale_catalog().iter().find(|m| m.id == id)
}

/// Swift `UpscaleModelConfig.allIds.contains` (the Rerun branch test).
pub fn is_upscale_model_id(id: &str) -> bool {
    upscale_model_by_id(id).is_some()
}

/// Swift `CostEstimator.upscaleCost`: pricePerSecond × max(1, duration).
pub fn upscale_cost(model: &UpscaleModelConfig, duration_seconds: i64) -> Option<f64> {
    Some(model.price_per_second * duration_seconds.max(1) as f64)
}

/// Human-readable label for an image aspect / image-size API enum id
/// (upstream #284, verbatim port of Swift
/// `ImageModelConfig.aspectRatioDisplayLabel`). Colon-form ids
/// ("16:9", "2.35:1") pass through unchanged; underscore enum ids are
/// tokenized ("landscape_16_9" → "Landscape 16:9", "square_hd" → "Square HD").
pub fn aspect_ratio_display_label(id: &str) -> String {
    if id.contains(':') {
        return id.to_string();
    }
    let mut parts: Vec<String> = id.split('_').map(str::to_string).collect();
    if parts.is_empty() {
        return id.to_string();
    }
    if parts.len() >= 2 {
        let n = parts.len();
        if let (Ok(width), Ok(height)) = (parts[n - 2].parse::<i64>(), parts[n - 1].parse::<i64>())
        {
            parts.truncate(n - 2);
            parts.push(format!("{width}:{height}"));
        }
    }
    parts
        .iter()
        .map(|t| aspect_ratio_display_token(t))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Per-token casing for [`aspect_ratio_display_label`] (Swift
/// `aspectRatioDisplayToken`): resolution acronyms upcase, everything else
/// capitalizes.
fn aspect_ratio_display_token(token: &str) -> String {
    let lower = token.to_lowercase();
    match lower.as_str() {
        "hd" | "uhd" | "1k" | "2k" | "4k" | "8k" => lower.to_uppercase(),
        _ => {
            let mut chars = lower.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => lower,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_snapshot_ids_names_kinds_paid_only() {
        // Pins the full transcribed list (upstream Fal-era catalog + #249 paid_only).
        let got: Vec<(&str, &str, &str, bool)> = catalog()
            .iter()
            .map(|m| (m.kind_str(), m.id, m.display_name, m.paid_only))
            .collect();
        let expected = vec![
            ("video", "seedance-2", "Seedance 2", false),
            ("video", "seedance-2-fast", "Seedance 2 Fast", false),
            ("video", "kling-o3", "Kling O3", false),
            ("video", "kling-v3", "Kling V3", false),
            ("video", "veo3.1-fast", "Veo 3.1 Fast", false),
            ("video", "veo3.1", "Veo 3.1", false),
            ("video", "veo3.1-lite", "Veo 3.1 Lite", false),
            ("video", "grok-imagine-video", "Grok Imagine Video", false),
            ("video", "kling-o3-edit", "Kling O3 Edit", false),
            (
                "video",
                "kling-v3-motion-control",
                "Kling V3 Motion Control",
                false,
            ),
            ("image", "nano-banana-pro", "Nano Banana Pro", false),
            ("image", "nano-banana-2", "Nano Banana 2", false),
            ("image", "gpt-image-2", "GPT Image 2", false),
            ("image", "grok-imagine", "Grok Imagine", false),
            ("image", "recraft-v4", "Recraft V4", false),
            ("audio", "elevenlabs-tts-v3", "ElevenLabs v3 TTS", false),
            (
                "audio",
                "gemini-3.1-flash-tts",
                "Gemini 3.1 Flash TTS",
                false,
            ),
            ("audio", "minimax-music-v2.6", "MiniMax Music 2.6", false),
            ("audio", "elevenlabs-music", "ElevenLabs Music", false),
        ];
        assert_eq!(got, expected);
    }

    fn video(id: &str) -> &'static VideoCaps {
        match &model_by_id(id).unwrap().caps {
            ModelCaps::Video(c) => c,
            _ => panic!("{id} is not video"),
        }
    }

    fn image(id: &str) -> &'static ImageCaps {
        match &model_by_id(id).unwrap().caps {
            ModelCaps::Image(c) => c,
            _ => panic!("{id} is not image"),
        }
    }

    fn audio(id: &str) -> &'static AudioCaps {
        match &model_by_id(id).unwrap().caps {
            ModelCaps::Audio(c) => c,
            _ => panic!("{id} is not audio"),
        }
    }

    #[test]
    fn seedance_caps_transcribed() {
        let c = video("seedance-2");
        assert_eq!(c.durations, (4..=15).collect::<Vec<_>>());
        assert_eq!(
            c.resolutions.as_deref(),
            Some(&["480p", "720p", "1080p"][..])
        );
        assert_eq!(
            c.aspect_ratios,
            vec!["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]
        );
        assert!(c.supports_first_frame && c.supports_last_frame);
        assert_eq!(
            (
                c.max_reference_images,
                c.max_reference_videos,
                c.max_reference_audios
            ),
            (9, 3, 3)
        );
        assert_eq!(c.max_total_references, Some(12));
        assert_eq!(c.max_combined_video_ref_seconds, Some(15.0));
        assert_eq!(c.max_combined_audio_ref_seconds, Some(15.0));
        assert!(c.frames_and_references_exclusive);
        assert_eq!(c.reference_tag_noun, "Image");
        assert_eq!(c.max_references(), 12);
        assert_eq!(
            c.price_per_second,
            vec![("480p", 0.1345), ("720p", 0.3024), ("1080p", 0.68)]
        );
        let fast = video("seedance-2-fast");
        assert_eq!(fast.resolutions.as_deref(), Some(&["480p", "720p"][..]));
        assert_eq!(
            fast.price_per_second,
            vec![("480p", 0.0843), ("720p", 0.2427)]
        );
    }

    #[test]
    fn kling_caps_transcribed() {
        let o3 = video("kling-o3");
        assert_eq!(o3.durations, (3..=15).collect::<Vec<_>>());
        assert_eq!(o3.max_reference_images, 7);
        assert_eq!(o3.reference_tag_noun, "Element");
        assert_eq!(o3.audio_discount_rate, Some(vec![("1080p", 0.8)]));
        assert_eq!(o3.price_per_second, vec![("1080p", 0.14), ("4k", 0.42)]);
        let v3 = video("kling-v3");
        assert_eq!(v3.max_reference_images, 3);
        assert_eq!(v3.audio_discount_rate, Some(vec![("1080p", 2.0 / 3.0)]));
        assert_eq!(v3.price_per_second, vec![("1080p", 0.168), ("4k", 0.42)]);
    }

    #[test]
    fn veo_caps_transcribed() {
        for id in ["veo3.1", "veo3.1-fast", "veo3.1-lite"] {
            let c = video(id);
            assert_eq!(c.durations, vec![4, 6, 8], "{id}");
            assert_eq!(c.aspect_ratios, vec!["16:9", "9:16"], "{id}");
            assert!(c.supports_last_frame, "{id}");
            assert_eq!(c.audio_discount_rate, Some(vec![("", 2.0 / 3.0)]), "{id}");
        }
        assert_eq!(
            video("veo3.1").price_per_second,
            vec![("720p", 0.40), ("1080p", 0.40), ("4k", 0.60)]
        );
        assert_eq!(
            video("veo3.1-lite").resolutions.as_deref(),
            Some(&["720p", "1080p"][..])
        );
    }

    #[test]
    fn edit_models_transcribed() {
        for id in ["kling-o3-edit", "kling-v3-motion-control"] {
            let c = video(id);
            assert!(c.durations.is_empty(), "{id}");
            assert!(c.resolutions.is_none(), "{id}");
            assert!(c.aspect_ratios.is_empty(), "{id}");
            assert!(!c.supports_first_frame && !c.supports_last_frame, "{id}");
            assert!(c.requires_source_video, "{id}");
            assert_eq!(c.price_per_second, vec![("", 0.168)], "{id}");
        }
        assert_eq!(video("kling-v3-motion-control").max_reference_images, 1);
        assert!(!video("kling-o3-edit").supports_references());
    }

    #[test]
    fn grok_video_caps_transcribed() {
        let c = video("grok-imagine-video");
        assert_eq!(c.durations, (6..=15).collect::<Vec<_>>());
        assert!(c.supports_first_frame && !c.supports_last_frame);
        assert_eq!(c.max_reference_images, 7);
        assert!(c.frames_and_references_exclusive);
        assert_eq!(c.price_per_second, vec![("480p", 0.05), ("720p", 0.07)]);
    }

    #[test]
    fn image_caps_transcribed() {
        let gpt = image("gpt-image-2");
        assert_eq!(
            gpt.qualities.as_deref(),
            Some(&["low", "medium", "high"][..])
        );
        assert_eq!(gpt.resolutions.as_ref().unwrap().len(), 6);
        assert!(gpt.aspect_ratios.is_empty());
        assert_eq!(gpt.max_images, 1);
        assert_eq!(gpt.price_per_image.len(), 18);
        assert_eq!(gpt.price_per_image[17], ("3840x2160|high", 0.41));

        assert_eq!(image("nano-banana-pro").aspect_ratios.len(), 11);
        assert_eq!(image("nano-banana-2").aspect_ratios.len(), 15);
        assert!(!image("recraft-v4").supports_image_reference);
        assert_eq!(image("grok-imagine").aspect_ratios.len(), 13);
        assert_eq!(image("grok-imagine").price_per_image, vec![("", 0.02)]);
    }

    #[test]
    fn audio_caps_transcribed() {
        let el = audio("elevenlabs-tts-v3");
        assert_eq!(el.category, AudioCategory::Tts);
        assert_eq!(el.voices.as_ref().unwrap().len(), 21);
        assert_eq!(el.default_voice, Some("Rachel"));
        assert_eq!(el.pricing, AudioPricing::PerThousandChars(0.10));

        let gem = audio("gemini-3.1-flash-tts");
        assert_eq!(gem.voices.as_ref().unwrap().len(), 30);
        assert_eq!(gem.default_voice, Some("Kore"));
        assert!(gem.supports_style_instructions);

        let mm = audio("minimax-music-v2.6");
        assert_eq!(mm.category, AudioCategory::Music);
        assert!(mm.supports_lyrics && mm.supports_instrumental);
        assert_eq!(mm.min_prompt_length, 10);
        assert_eq!(mm.pricing, AudioPricing::Flat(0.03));

        let em = audio("elevenlabs-music");
        assert_eq!(em.durations, Some(vec![15, 30, 60, 90, 120, 180]));
        assert_eq!(em.pricing, AudioPricing::PerSecond(0.002));
        assert!(!em.supports_lyrics && em.supports_instrumental);
    }

    #[test]
    fn gating_quadrant() {
        assert!(model_available(false, false), "free account, free model");
        assert!(!model_available(false, true), "free account, paid model");
        assert!(model_available(true, false), "paid account, free model");
        assert!(model_available(true, true), "paid account, paid model");
    }

    #[test]
    fn require_plan_message_matches_swift() {
        let msg = require_plan_message("kling-v3");
        assert_eq!(
            msg,
            "Model 'kling-v3' requires a paid plan. Pick a free model from list_models, \
             or tell the user to subscribe."
        );
    }

    #[test]
    fn default_model_first_available_per_kind() {
        assert_eq!(
            default_model(ModelKind::Video, false).unwrap().id,
            "seedance-2"
        );
        assert_eq!(
            default_model(ModelKind::Image, false).unwrap().id,
            "nano-banana-pro"
        );
        assert_eq!(
            default_model(ModelKind::Audio, false).unwrap().id,
            "elevenlabs-tts-v3"
        );
        let err = default_model(ModelKind::Upscale, false).unwrap_err();
        assert!(err.contains("No upscale model is available"));
    }

    #[test]
    fn model_by_id_lookup() {
        assert_eq!(model_by_id("veo3.1").unwrap().display_name, "Veo 3.1");
        assert!(model_by_id("gen-3").is_none(), "placeholder ids are gone");
    }

    // ── Cost estimation (pinned to Swift CostEstimator @ 9dfde8d^) ──

    fn approx(a: Option<f64>, b: f64) {
        let a = a.expect("cost expected");
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn video_cost_by_resolution() {
        approx(
            video_cost(video("seedance-2"), 5, Some("720p"), true),
            1.512,
        );
        approx(
            video_cost(video("seedance-2"), 5, Some("1080p"), true),
            3.40,
        );
        approx(
            video_cost(video("veo3.1-lite"), 8, Some("720p"), true),
            0.40,
        );
    }

    #[test]
    fn video_cost_audio_discount() {
        // kling-o3: 0.14 * 0.8 * 10
        approx(
            video_cost(video("kling-o3"), 10, Some("1080p"), false),
            1.12,
        );
        // no discount entry for 4k and no "" default → full rate
        approx(video_cost(video("kling-o3"), 10, Some("4k"), false), 4.20);
        // veo: "" default discount 2/3 applies at any resolution
        approx(
            video_cost(video("veo3.1"), 8, Some("4k"), false),
            0.60 * (2.0 / 3.0) * 8.0,
        );
    }

    #[test]
    fn video_cost_flat_rate_and_guards() {
        // Edit models price on the "" key regardless of resolution.
        approx(video_cost(video("kling-o3-edit"), 10, None, true), 1.68);
        assert_eq!(
            video_cost(video("kling-o3-edit"), 0, None, true),
            None,
            "no duration (no source video) → no estimate"
        );
        assert_eq!(
            video_cost(video("seedance-2"), 5, Some("8k"), true),
            None,
            "unknown resolution with no default rate"
        );
        let unpriced = VideoCaps::default();
        assert_eq!(video_cost(&unpriced, 5, None, true), None);
    }

    #[test]
    fn image_cost_matrix_and_flat() {
        // 2D matrix (GPT Image 2).
        approx(
            image_cost(image("gpt-image-2"), Some("1024x1024"), Some("high"), 1),
            0.22,
        );
        approx(
            image_cost(image("gpt-image-2"), Some("3840x2160"), Some("medium"), 1),
            0.11,
        );
        assert_eq!(
            image_cost(image("gpt-image-2"), Some("1024x1024"), None, 1),
            None,
            "quality-priced model without a quality → no estimate"
        );
        // Resolution-keyed (Nano Banana Pro), multiplied by count.
        approx(
            image_cost(image("nano-banana-pro"), Some("4K"), None, 3),
            0.90,
        );
        // Flat "" key (Grok), count clamped to ≥1.
        approx(image_cost(image("grok-imagine"), None, None, 4), 0.08);
        approx(image_cost(image("grok-imagine"), None, None, 0), 0.02);
        approx(image_cost(image("recraft-v4"), None, None, 2), 0.50);
    }

    #[test]
    fn audio_cost_modes() {
        let prompt_100: String = "x".repeat(100);
        approx(
            audio_cost(audio("elevenlabs-tts-v3"), &prompt_100, None),
            0.01,
        );
        approx(
            audio_cost(audio("gemini-3.1-flash-tts"), &"x".repeat(1000), None),
            0.03,
        );
        assert_eq!(
            audio_cost(audio("elevenlabs-tts-v3"), "", None),
            None,
            "empty prompt → no per-char estimate"
        );
        approx(
            audio_cost(audio("elevenlabs-music"), "beat", Some(60)),
            0.12,
        );
        assert_eq!(
            audio_cost(audio("elevenlabs-music"), "beat", None),
            None,
            "per-second pricing needs a duration"
        );
        approx(
            audio_cost(audio("minimax-music-v2.6"), "lofi hip hop", None),
            0.03,
        );
        let unknown = AudioCaps::default();
        assert_eq!(audio_cost(&unknown, "prompt", Some(30)), None);
    }

    #[test]
    fn format_usd_matches_swift() {
        assert_eq!(format_usd(None), "—");
        assert_eq!(format_usd(Some(0.0)), "$0.00");
        assert_eq!(format_usd(Some(-1.0)), "$0.00");
        assert_eq!(format_usd(Some(0.005)), "<$0.01");
        assert_eq!(format_usd(Some(1.512)), "$1.51");
        assert_eq!(format_usd(Some(3.4)), "$3.40");
    }

    #[test]
    fn upscale_catalog_snapshot_field_for_field() {
        // Pins the Swift `UpscaleModelConfig.allModels` list at 9dfde8d^.
        use core_model::ClipType;
        let got: Vec<(&str, &str, &str, &str, f64, i64, &[ClipType])> = upscale_catalog()
            .iter()
            .map(|m| {
                (
                    m.id,
                    m.display_name,
                    m.speed,
                    m.endpoint,
                    m.price_per_second,
                    m.p75_duration_seconds,
                    m.supported_types,
                )
            })
            .collect();
        let expected: Vec<(&str, &str, &str, &str, f64, i64, &[ClipType])> = vec![
            (
                "bytedance-upscaler",
                "Bytedance Upscaler",
                "Fast",
                "fal-ai/bytedance-upscaler/upscale/video",
                0.0288,
                130,
                &[ClipType::Video],
            ),
            (
                "seedvr-upscaler",
                "SeedVR2",
                "Medium",
                "fal-ai/seedvr/upscale/video",
                0.062,
                691,
                &[ClipType::Video],
            ),
            (
                "topaz-upscaler",
                "Topaz Upscale",
                "Slow",
                "fal-ai/topaz/upscale/video",
                0.08,
                65,
                &[ClipType::Video],
            ),
            (
                "seedvr-image-upscaler",
                "SeedVR2",
                "Fast",
                "fal-ai/seedvr/upscale/image",
                0.04,
                19,
                &[ClipType::Image],
            ),
            (
                "topaz-image-upscaler",
                "Topaz Upscale",
                "Medium",
                "fal-ai/topaz/upscale/image",
                0.08,
                24,
                &[ClipType::Image],
            ),
        ];
        assert_eq!(got, expected);
        assert!(upscale_catalog()
            .iter()
            .all(|m| m.kind() == ModelKind::Upscale));
    }

    #[test]
    fn upscale_models_for_filters_by_type() {
        use core_model::ClipType;
        let video: Vec<&str> = upscale_models_for(ClipType::Video)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(
            video,
            vec!["bytedance-upscaler", "seedvr-upscaler", "topaz-upscaler"]
        );
        let image: Vec<&str> = upscale_models_for(ClipType::Image)
            .iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(image, vec!["seedvr-image-upscaler", "topaz-image-upscaler"]);
        assert!(upscale_models_for(ClipType::Audio).is_empty());
    }

    #[test]
    fn upscale_id_lookup_and_cost() {
        assert!(is_upscale_model_id("topaz-upscaler"));
        assert!(!is_upscale_model_id("seedance-2"));
        let m = upscale_model_by_id("bytedance-upscaler").unwrap();
        // Swift upscaleCost clamps duration to ≥ 1.
        assert_eq!(upscale_cost(m, 10), Some(0.288));
        assert_eq!(upscale_cost(m, 0), Some(0.0288));
        assert_eq!(format_usd(upscale_cost(m, 10)), "$0.29");
    }

    #[test]
    fn audio_discount_lookup_precedence() {
        assert_eq!(video("kling-o3").audio_discount(Some("1080p")), Some(0.8));
        assert_eq!(video("kling-o3").audio_discount(Some("4k")), None);
        assert_eq!(video("kling-o3").audio_discount(None), None);
        assert_eq!(
            video("veo3.1").audio_discount(Some("anything")),
            Some(2.0 / 3.0),
            "\"\" key is the default"
        );
        assert_eq!(video("seedance-2").audio_discount(Some("720p")), None);
    }

    // ── #294 source-based audio categories ──────────────────────────

    #[test]
    fn audio_category_raw_values_and_labels_match_swift() {
        // Swift Category rawValues (String-backed since #294) + labels.
        let cases = [
            (AudioCategory::Tts, "tts", "Speech"),
            (AudioCategory::Music, "music", "Music"),
            (AudioCategory::Sfx, "sfx", "Sound Effects"),
            (AudioCategory::Cleanup, "cleanup", "Voice Cleanup"),
            (AudioCategory::Dubbing, "dubbing", "Dubbing"),
        ];
        for (cat, raw, label) in cases {
            assert_eq!(cat.as_str(), raw);
            assert_eq!(cat.label(), label);
        }
        assert_eq!(AudioInput::Text.as_str(), "text");
        assert_eq!(AudioInput::Audio.as_str(), "audio");
        assert_eq!(AudioInput::Video.as_str(), "video");
    }

    #[test]
    fn audio_inputs_default_and_source_acceptance() {
        use core_model::ClipType;
        let default_caps = AudioCaps::default();
        assert_eq!(default_caps.effective_inputs(), &[AudioInput::Text]);
        assert!(!default_caps.accepts_source_media());
        assert!(!default_caps.uses_source_url());
        assert!(!default_caps.accepts_source(ClipType::Video));

        let dubbing = AudioCaps {
            category: AudioCategory::Dubbing,
            inputs: Some(vec![AudioInput::Audio, AudioInput::Video]),
            ..Default::default()
        };
        assert!(dubbing.accepts_source_media());
        assert!(dubbing.uses_source_url());
        assert!(dubbing.accepts_source(ClipType::Audio));
        assert!(dubbing.accepts_source(ClipType::Video));
        for t in [
            ClipType::Image,
            ClipType::Text,
            ClipType::Lottie,
            ClipType::Shape,
            ClipType::Sequence,
        ] {
            assert!(!dubbing.accepts_source(t), "{t:?} is never source media");
        }

        let cleanup = AudioCaps {
            category: AudioCategory::Cleanup,
            ..Default::default()
        };
        assert!(cleanup.uses_source_url());
        let sfx = AudioCaps {
            category: AudioCategory::Sfx,
            inputs: Some(vec![AudioInput::Text, AudioInput::Video]),
            ..Default::default()
        };
        assert!(!sfx.uses_source_url());
        assert!(sfx.accepts_source_media());
    }

    #[test]
    fn audio_validate_span_seconds_messages_match_swift() {
        let caps = AudioCaps {
            min_seconds: Some(5.0),
            max_seconds: Some(30.0),
            ..Default::default()
        };
        assert_eq!(
            audio_validate_span_seconds("Test SFX", &caps, 3.2),
            Some("Test SFX needs at least 5s of source media (selection is 3s).".to_string())
        );
        assert_eq!(
            audio_validate_span_seconds("Test SFX", &caps, 700.0),
            Some("Test SFX accepts at most 30s of source media (selection is 700s).".to_string())
        );
        assert_eq!(audio_validate_span_seconds("Test SFX", &caps, 10.0), None);
        // Swift defaults: minSeconds ?? 1, maxSeconds ?? 600.
        let defaults = AudioCaps::default();
        assert_eq!(audio_validate_span_seconds("M", &defaults, 0.2), Some(
            "M needs at least 1s of source media (selection is 0s).".to_string()
        ));
        assert_eq!(audio_validate_span_seconds("M", &defaults, 600.4), None);
    }

    #[test]
    fn audio_validate_params_swift_parity() {
        use crate::generation_payload::AudioGenerationPayload;
        let params = |prompt: &str,
                      voice: Option<&str>,
                      duration: Option<f64>,
                      lang: Option<&str>| AudioGenerationPayload {
            prompt: prompt.into(),
            voice: voice.map(String::from),
            lyrics: None,
            style_instructions: None,
            instrumental: false,
            duration_seconds: duration,
            video_url: None,
            source_url: None,
            target_language: lang.map(String::from),
        };

        // Prompt-length check fires for text-input models only (#294).
        let text_caps = AudioCaps {
            min_prompt_length: 10,
            ..Default::default()
        };
        assert_eq!(
            audio_validate_params("MiniMax", &text_caps, &params("hey", None, None, None)),
            Some("MiniMax requires prompt ≥ 10 characters (got 3).".to_string())
        );
        let cleanup_caps = AudioCaps {
            category: AudioCategory::Cleanup,
            inputs: Some(vec![AudioInput::Audio, AudioInput::Video]),
            min_prompt_length: 10,
            ..Default::default()
        };
        assert_eq!(
            audio_validate_params("Isolator", &cleanup_caps, &params("", None, None, None)),
            None,
            "no prompt check for non-text models"
        );

        // Voice gate: first 6 shown, ellipsis beyond.
        let voiced = AudioCaps {
            voices: Some(vec!["A", "B", "C", "D", "E", "F", "G"]),
            min_prompt_length: 0,
            ..Default::default()
        };
        assert_eq!(
            audio_validate_params("TTS", &voiced, &params("hi", Some("Zed"), None, None)),
            Some("TTS does not support voice 'Zed'. Valid: A, B, C, D, E, F, ….".to_string())
        );

        // Duration gate.
        let timed = AudioCaps {
            durations: Some(vec![15, 30]),
            min_prompt_length: 0,
            ..Default::default()
        };
        assert_eq!(
            audio_validate_params("Music", &timed, &params("x", None, Some(45.0), None)),
            Some("Music does not support duration '45s'. Valid: 15s, 30s.".to_string())
        );

        // Target-language gate (dubbing).
        let dubbing = AudioCaps {
            category: AudioCategory::Dubbing,
            inputs: Some(vec![AudioInput::Audio, AudioInput::Video]),
            target_languages: Some(vec!["es", "fr"]),
            default_target_language: Some("es"),
            ..Default::default()
        };
        assert_eq!(
            audio_validate_params("Dub", &dubbing, &params("", None, None, None)),
            Some("Choose a target language.".to_string())
        );
        assert_eq!(
            audio_validate_params("Dub", &dubbing, &params("", None, None, Some(""))),
            Some("Choose a target language.".to_string())
        );
        assert_eq!(
            audio_validate_params("Dub", &dubbing, &params("", None, None, Some("xx"))),
            Some("Dub does not support target language 'xx'. Valid: es, fr.".to_string())
        );
        assert_eq!(
            audio_validate_params("Dub", &dubbing, &params("", None, None, Some("es"))),
            None
        );
    }

    #[test]
    fn aspect_ratio_display_labels_match_swift() {
        // Golden vectors transplanted verbatim from Swift ImageModelConfigTests
        // (deleted in upstream PR #284; the labels are the canonical contract).
        let cases = [
            ("16:9", "16:9"),
            ("2.35:1", "2.35:1"),
            ("auto", "Auto"),
            ("auto_2K", "Auto 2K"),
            ("square_hd", "Square HD"),
            ("16_9", "16:9"),
            ("portrait_4_3", "Portrait 4:3"),
            ("portrait_16_9", "Portrait 16:9"),
            ("landscape_4_3", "Landscape 4:3"),
            ("landscape_16_9", "Landscape 16:9"),
        ];
        for (id, expected) in cases {
            assert_eq!(
                aspect_ratio_display_label(id),
                expected,
                "aspect_ratio_display_label({id})"
            );
        }
    }
}
