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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCategory {
    Tts,
    Music,
}

impl AudioCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioCategory::Tts => "tts",
            AudioCategory::Music => "music",
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
        }
    }
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
    "Rachel", "Aria", "Roger", "Sarah", "Laura", "Charlie", "George", "Callum", "River", "Liam",
    "Charlotte", "Alice", "Matilda", "Will", "Jessica", "Eric", "Chris", "Brian", "Daniel", "Lily",
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
            ("audio", "gemini-3.1-flash-tts", "Gemini 3.1 Flash TTS", false),
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
        assert_eq!(c.resolutions.as_deref(), Some(&["480p", "720p", "1080p"][..]));
        assert_eq!(
            c.aspect_ratios,
            vec!["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]
        );
        assert!(c.supports_first_frame && c.supports_last_frame);
        assert_eq!(
            (c.max_reference_images, c.max_reference_videos, c.max_reference_audios),
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
        assert_eq!(fast.price_per_second, vec![("480p", 0.0843), ("720p", 0.2427)]);
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
        assert_eq!(gpt.qualities.as_deref(), Some(&["low", "medium", "high"][..]));
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
        assert_eq!(default_model(ModelKind::Video, false).unwrap().id, "seedance-2");
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
}
