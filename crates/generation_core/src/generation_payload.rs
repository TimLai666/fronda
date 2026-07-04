//! Generation request payload and validation.
//!
//! Covers GPAY-001 through GPAY-016.

// ── Video generation ────────────────────────────────────────────

/// GPAY-001: Video generation payload.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoGenerationPayload {
    pub prompt: String,
    pub duration: f64,
    pub aspect_ratio: String,
    pub resolution: Option<String>,
    pub source_video_url: Option<String>,
    pub start_frame_url: Option<String>,
    pub end_frame_url: Option<String>,
    pub reference_image_urls: Vec<String>,
    pub reference_video_urls: Vec<String>,
    pub reference_audio_urls: Vec<String>,
    pub generate_audio: bool,
}

impl VideoGenerationPayload {
    /// Validate supported values against model capabilities.
    pub fn validate(
        &self,
        supported_durations: &[f64],
        supported_aspect_ratios: &[&str],
        supported_resolutions: &[&str],
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // GPAY-002
        if !supported_durations.is_empty() && !supported_durations.contains(&self.duration) {
            errors.push(format!(
                "unsupportedValue: duration {} not in {:?}",
                self.duration, supported_durations
            ));
        }
        if !supported_aspect_ratios.is_empty()
            && !supported_aspect_ratios.contains(&self.aspect_ratio.as_str())
        {
            errors.push(format!(
                "unsupportedValue: aspect ratio {} not in {:?}",
                self.aspect_ratio, supported_aspect_ratios
            ));
        }
        if let Some(ref res) = self.resolution {
            if !supported_resolutions.is_empty() && !supported_resolutions.contains(&res.as_str()) {
                errors.push(format!(
                    "unsupportedValue: resolution {} not in {:?}",
                    res, supported_resolutions
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// GPAY-003: Whether the model supports references.
    pub fn supports_references(max_images: usize, max_videos: usize, max_audio: usize) -> bool {
        max_images > 0 || max_videos > 0 || max_audio > 0
    }

    /// GPAY-004: Maximum references using maxTotalReferences or sum of individual caps.
    pub fn max_references(
        max_total: Option<usize>,
        max_images: usize,
        max_videos: usize,
        max_audio: usize,
    ) -> usize {
        max_total.unwrap_or(max_images + max_videos + max_audio)
    }

    /// GPAY-005: Audio discount lookup by resolution key.
    pub fn audio_discount(
        rates: &std::collections::HashMap<String, f64>,
        resolution: Option<&str>,
    ) -> Option<f64> {
        if let Some(res) = resolution {
            if let Some(rate) = rates.get(res) {
                return Some(*rate);
            }
        }
        rates.get("").copied()
    }
}

// ═══════════════════════════════════════════════════════════════════
// Image generation
// ═══════════════════════════════════════════════════════════════════

/// GPAY-006: Image generation payload.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageGenerationPayload {
    pub prompt: String,
    pub aspect_ratio: String,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub image_urls: Vec<String>,
    pub num_images: usize,
}

impl ImageGenerationPayload {
    /// GPAY-007: Clamp max images to [1, 4].
    pub fn clamp_max_images(catalog_max: usize) -> usize {
        catalog_max.clamp(1, 4)
    }

    /// GPAY-008: Validate image generation parameters.
    pub fn validate(
        &self,
        supported_aspect_ratios: &[&str],
        supported_resolutions: &[&str],
        supported_qualities: &[&str],
        supports_reference_images: bool,
        max_images: usize,
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if !supported_aspect_ratios.is_empty()
            && !supported_aspect_ratios.contains(&self.aspect_ratio.as_str())
        {
            errors.push(format!(
                "unsupportedValue: aspect ratio {} not in {:?}",
                self.aspect_ratio, supported_aspect_ratios
            ));
        }
        if let Some(ref res) = self.resolution {
            if !supported_resolutions.is_empty() && !supported_resolutions.contains(&res.as_str()) {
                errors.push(format!(
                    "unsupportedValue: resolution {} not in {:?}",
                    res, supported_resolutions
                ));
            }
        }
        if let Some(ref qual) = self.quality {
            if !supported_qualities.is_empty() && !supported_qualities.contains(&qual.as_str()) {
                errors.push(format!(
                    "unsupportedValue: quality {} not in {:?}",
                    qual, supported_qualities
                ));
            }
        }
        if !self.image_urls.is_empty() && !supports_reference_images {
            errors.push("unsupportedValue: reference images not supported".into());
        }
        if self.num_images < 1 || self.num_images > max_images {
            errors.push(format!(
                "numImages {} outside 1..{max_images}",
                self.num_images
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// GPAY-009: Parse resolution label as "WxH" pair.
    pub fn parse_resolution_label(label: &str) -> Option<(u32, u32)> {
        let label = label.to_lowercase();
        let parts: Vec<&str> = label.split('x').collect();
        if parts.len() != 2 {
            return None;
        }
        let w = parts[0].parse::<u32>().ok()?;
        let h = parts[1].parse::<u32>().ok()?;
        Some((w, h))
    }

    /// GPAY-010: Resolution display label.
    pub fn resolution_display_label(w: u32, h: u32) -> String {
        if w == h {
            return "Square".into();
        }
        let (landscape, long_edge) = if w >= h { (true, w) } else { (false, h) };
        let orientation = if landscape { "Landscape" } else { "Portrait" };
        let tier = match long_edge {
            3840 => Some("4K"),
            2560 => Some("2K"),
            1920 => Some("1080p"),
            1024 | 1536 => None,
            _ => return String::new(),
        };
        match tier {
            Some(t) => format!("{orientation} {t}"),
            None => orientation.to_string(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Audio generation
// ═══════════════════════════════════════════════════════════════════

/// GPAY-011: Audio generation payload.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioGenerationPayload {
    pub prompt: String,
    pub voice: Option<String>,
    pub lyrics: Option<String>,
    pub style_instructions: Option<String>,
    pub instrumental: bool,
    pub duration_seconds: Option<f64>,
    pub video_url: Option<String>,
}

/// GPAY-012: Audio category labels.
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCategory {
    Speech,
    Music,
    SoundEffects,
}

impl AudioCategory {
    /// Parse from catalog string; defaults to Speech (GPAY-012).
    pub fn from_catalog(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "music" => AudioCategory::Music,
            "sound effects" | "sfx" => AudioCategory::SoundEffects,
            _ => AudioCategory::Speech,
        }
    }
}

/// GPAY-013: Audio defaults.
pub const AUDIO_DEFAULT_INPUTS: &str = "text";
pub const AUDIO_DEFAULT_PROMPT_LABEL: &str = "Describe the sound";
pub const AUDIO_MIN_SECONDS: f64 = 1.0;
pub const AUDIO_MAX_SECONDS: f64 = 900.0;

impl AudioGenerationPayload {
    /// GPAY-014: Validate audio generation.
    pub fn validate(
        &self,
        min_prompt_length: usize,
        supported_voices: &[&str],
        supported_durations: &[f64],
    ) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Swift compares the whitespace-trimmed character count; prompt.len() (UTF-8
        // byte length, untrimmed) over-counts non-ASCII and padded prompts (e.g.
        // "café" is 5 bytes but 4 characters).
        let prompt_len = self.prompt.trim().chars().count();
        if prompt_len < min_prompt_length {
            errors.push(format!("Prompt too short ({prompt_len} < {min_prompt_length})"));
        }
        if let Some(ref voice) = self.voice {
            if !supported_voices.is_empty() && !supported_voices.contains(&voice.as_str()) {
                errors.push(format!("Unsupported voice: {voice}"));
            }
        }
        if let Some(dur) = self.duration_seconds {
            // A duration not in the model's supported list is unsupported, matching
            // the sibling video check and Swift AudioModelConfig.validate — do NOT
            // also accept it just because it falls in the global [1,900] span range.
            if !supported_durations.is_empty() && !supported_durations.contains(&dur) {
                errors.push(format!(
                    "unsupportedValue: duration {dur}s not in {supported_durations:?}"
                ));
            }
        }
        if let Some(ref _url) = self.video_url {
            // video span validation if needed
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Upscale generation
// ═══════════════════════════════════════════════════════════════════

/// GPAY-015: Upscale generation payload.
#[derive(Debug, Clone, PartialEq)]
pub struct UpscaleGenerationPayload {
    pub source_url: String,
    pub duration_seconds: f64,
}

/// GPAY-016: Parse supported clip types from catalog strings.
pub fn parse_supported_clip_types(catalog_strings: &[String]) -> Vec<String> {
    catalog_strings
        .iter()
        .filter_map(|s| {
            let trimmed = s.trim().to_lowercase();
            match trimmed.as_str() {
                "video" | "image" | "audio" | "text" | "lottie" => Some(trimmed),
                _ => None,
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // GPAY-001
    #[test]
    fn video_payload_fields() {
        let p = VideoGenerationPayload {
            prompt: "A cat video".into(),
            duration: 5.0,
            aspect_ratio: "16:9".into(),
            resolution: Some("1920x1080".into()),
            source_video_url: None,
            start_frame_url: None,
            end_frame_url: None,
            reference_image_urls: vec![],
            reference_video_urls: vec![],
            reference_audio_urls: vec![],
            generate_audio: true,
        };
        assert_eq!(p.prompt, "A cat video");
        assert!(p.generate_audio);
    }

    // GPAY-002
    #[test]
    fn video_validation_rejects_unsupported_duration() {
        let p = VideoGenerationPayload {
            prompt: "test".into(),
            duration: 99.0,
            aspect_ratio: "16:9".into(),
            resolution: None,
            source_video_url: None,
            start_frame_url: None,
            end_frame_url: None,
            reference_image_urls: vec![],
            reference_video_urls: vec![],
            reference_audio_urls: vec![],
            generate_audio: true,
        };
        let result = p.validate(&[5.0, 10.0], &["16:9"], &[]);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs[0].contains("99"));
    }

    #[test]
    fn video_validation_passes_valid() {
        let p = VideoGenerationPayload {
            prompt: "test".into(),
            duration: 10.0,
            aspect_ratio: "16:9".into(),
            resolution: Some("1920x1080".into()),
            source_video_url: None,
            start_frame_url: None,
            end_frame_url: None,
            reference_image_urls: vec![],
            reference_video_urls: vec![],
            reference_audio_urls: vec![],
            generate_audio: true,
        };
        assert!(p.validate(&[5.0, 10.0], &["16:9"], &["1920x1080"]).is_ok());
    }

    // GPAY-003
    #[test]
    fn supports_references_positive() {
        assert!(VideoGenerationPayload::supports_references(1, 0, 0));
        assert!(VideoGenerationPayload::supports_references(0, 0, 1));
    }

    #[test]
    fn supports_references_negative() {
        assert!(!VideoGenerationPayload::supports_references(0, 0, 0));
    }

    // GPAY-004
    #[test]
    fn max_references_uses_total_when_present() {
        assert_eq!(
            VideoGenerationPayload::max_references(Some(5), 10, 10, 10),
            5
        );
    }

    #[test]
    fn max_references_sums_when_no_total() {
        assert_eq!(VideoGenerationPayload::max_references(None, 3, 2, 1), 6);
    }

    // GPAY-005
    #[test]
    fn audio_discount_by_resolution() {
        let mut rates = std::collections::HashMap::new();
        rates.insert("".to_string(), 1.0);
        rates.insert("1920x1080".to_string(), 0.8);
        assert!(
            (VideoGenerationPayload::audio_discount(&rates, Some("1920x1080")).unwrap() - 0.8)
                .abs()
                < 1e-10
        );
    }

    #[test]
    fn audio_discount_falls_back_to_default() {
        let mut rates = std::collections::HashMap::new();
        rates.insert("".to_string(), 1.0);
        assert!(
            (VideoGenerationPayload::audio_discount(&rates, Some("unknown")).unwrap() - 1.0).abs()
                < 1e-10
        );
    }

    // GPAY-006
    #[test]
    fn image_payload_fields() {
        let p = ImageGenerationPayload {
            prompt: "A scenic view".into(),
            aspect_ratio: "16:9".into(),
            resolution: Some("1920x1080".into()),
            quality: Some("standard".into()),
            image_urls: vec![],
            num_images: 1,
        };
        assert_eq!(p.num_images, 1);
    }

    // GPAY-007
    #[test]
    fn clamp_max_images() {
        assert_eq!(ImageGenerationPayload::clamp_max_images(10), 4);
        assert_eq!(ImageGenerationPayload::clamp_max_images(0), 1);
        assert_eq!(ImageGenerationPayload::clamp_max_images(2), 2);
    }

    // GPAY-008
    #[test]
    fn image_validation_passes() {
        let p = ImageGenerationPayload {
            prompt: "test".into(),
            aspect_ratio: "16:9".into(),
            resolution: None,
            quality: None,
            image_urls: vec![],
            num_images: 1,
        };
        assert!(p.validate(&["16:9"], &[], &[], false, 4).is_ok());
    }

    #[test]
    fn image_validation_rejects_unsupported_aspect() {
        let p = ImageGenerationPayload {
            prompt: "test".into(),
            aspect_ratio: "4:3".into(),
            resolution: None,
            quality: None,
            image_urls: vec![],
            num_images: 1,
        };
        let result = p.validate(&["16:9"], &[], &[], false, 4);
        assert!(result.is_err());
    }

    #[test]
    fn image_validation_rejects_ref_images_unsupported() {
        let p = ImageGenerationPayload {
            prompt: "test".into(),
            aspect_ratio: "1:1".into(),
            resolution: None,
            quality: None,
            image_urls: vec!["http://example.com/img.jpg".into()],
            num_images: 1,
        };
        let result = p.validate(&["1:1"], &[], &[], false, 4);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("reference images"));
    }

    // GPAY-009
    #[test]
    fn parse_resolution_label_valid() {
        assert_eq!(
            ImageGenerationPayload::parse_resolution_label("1920x1080"),
            Some((1920, 1080))
        );
        assert_eq!(
            ImageGenerationPayload::parse_resolution_label("1024x768"),
            Some((1024, 768))
        );
    }

    #[test]
    fn parse_resolution_label_invalid() {
        assert_eq!(
            ImageGenerationPayload::parse_resolution_label("invalid"),
            None
        );
        assert_eq!(
            ImageGenerationPayload::parse_resolution_label("1920x"),
            None
        );
    }

    #[test]
    fn parse_resolution_label_case_insensitive() {
        assert_eq!(
            ImageGenerationPayload::parse_resolution_label("1920X1080"),
            Some((1920, 1080))
        );
    }

    // GPAY-010
    #[test]
    fn resolution_display_square() {
        assert_eq!(
            ImageGenerationPayload::resolution_display_label(1024, 1024),
            "Square"
        );
    }

    #[test]
    fn resolution_display_landscape_4k() {
        assert_eq!(
            ImageGenerationPayload::resolution_display_label(3840, 2160),
            "Landscape 4K"
        );
    }

    #[test]
    fn resolution_display_portrait_hd() {
        assert_eq!(
            ImageGenerationPayload::resolution_display_label(1080, 1920),
            "Portrait 1080p"
        );
    }

    #[test]
    fn resolution_display_unknown_returns_empty() {
        assert_eq!(
            ImageGenerationPayload::resolution_display_label(800, 600),
            ""
        );
    }

    #[test]
    fn resolution_display_landscape_1024() {
        assert_eq!(
            ImageGenerationPayload::resolution_display_label(1024, 768),
            "Landscape"
        );
    }

    // GPAY-012
    #[test]
    fn audio_category_parsing() {
        assert_eq!(AudioCategory::from_catalog("speech"), AudioCategory::Speech);
        assert_eq!(AudioCategory::from_catalog("Music"), AudioCategory::Music);
        assert_eq!(
            AudioCategory::from_catalog("Sound Effects"),
            AudioCategory::SoundEffects
        );
        assert_eq!(
            AudioCategory::from_catalog("unknown"),
            AudioCategory::Speech
        );
    }

    // GPAY-013
    #[test]
    fn audio_defaults() {
        assert_eq!(AUDIO_DEFAULT_INPUTS, "text");
        assert!((AUDIO_MIN_SECONDS - 1.0).abs() < 1e-10);
        assert!((AUDIO_MAX_SECONDS - 900.0).abs() < 1e-10);
    }

    // GPAY-014
    #[test]
    fn audio_validation_rejects_short_prompt() {
        let p = AudioGenerationPayload {
            prompt: "hi".into(),
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: false,
            duration_seconds: None,
            video_url: None,
        };
        let result = p.validate(5, &[], &[]);
        assert!(result.is_err());
    }

    #[test]
    fn audio_validation_rejects_duration_not_in_supported_list() {
        // 45s is inside the global [1,900] span but NOT in the model's supported list;
        // it must still be rejected (matching the video check and Swift).
        let make = |dur: f64| AudioGenerationPayload {
            prompt: "a valid prompt".into(),
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: false,
            duration_seconds: Some(dur),
            video_url: None,
        };
        assert!(make(45.0).validate(1, &[], &[30.0, 60.0]).is_err());
        assert!(make(60.0).validate(1, &[], &[30.0, 60.0]).is_ok());
    }

    #[test]
    fn audio_validation_prompt_length_is_trimmed_char_count() {
        let make = |prompt: &str| AudioGenerationPayload {
            prompt: prompt.into(),
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: false,
            duration_seconds: None,
            video_url: None,
        };
        // "café" is 4 characters (5 UTF-8 bytes) → too short at min 5 (byte len passed).
        assert!(make("café").validate(5, &[], &[]).is_err());
        // Whitespace is trimmed before counting: "  abcd  " → "abcd" (4) < 5.
        assert!(make("  abcd  ").validate(5, &[], &[]).is_err());
        assert!(make("abcde").validate(5, &[], &[]).is_ok());
    }

    // GPAY-015
    #[test]
    fn upscale_payload() {
        let p = UpscaleGenerationPayload {
            source_url: "http://example.com/video.mp4".into(),
            duration_seconds: 10.0,
        };
        assert_eq!(p.source_url, "http://example.com/video.mp4");
    }

    // GPAY-016
    #[test]
    fn parse_supported_clip_types_valid() {
        let input = vec!["video".into(), "image".into(), "audio".into()];
        let result = parse_supported_clip_types(&input);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"video".to_string()));
    }

    #[test]
    fn parse_supported_clip_types_filters_unknown() {
        let input = vec!["video".into(), "unknown_type".into()];
        let result = parse_supported_clip_types(&input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "video");
    }
}
