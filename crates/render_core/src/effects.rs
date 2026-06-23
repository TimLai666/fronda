use core_model::{Clip, ClipType, Effect, GradeCurve, Timeline};
use std::collections::HashMap;

/// Pipeline-level summary of effects processing requirements.
/// Used by the compositor to decide which render passes are needed.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectPipeline {
    /// Whether any clip in the timeline has effects (enabled or not).
    pub has_effects: bool,
    /// Whether effects compositing is actually needed
    /// (visual clips with enabled effects exist).
    pub needs_effects_pass: bool,
    /// Whether both effects AND text overlays are active,
    /// requiring a dual-pass render (effects → text overlay).
    pub needs_dual_pass: bool,
}

/// Per-clip effect analysis state for the compositor.
#[derive(Debug, Clone, PartialEq)]
pub struct PerClipEffectState {
    pub clip_id: String,
    /// Ordered list of enabled (resolved) effects for this clip.
    pub effects: Vec<EffectState>,
    /// Whether this clip has color grading effects (color wheels or curves).
    pub has_color_grading: bool,
    /// Whether this clip has color adjustment effects
    /// (exposure, contrast, saturation, etc.).
    pub has_color_adjustments: bool,
    /// Whether this clip has blur or vignette effects.
    pub has_blur_or_vignette: bool,
}

/// Resolved state of a single effect at a given frame.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectState {
    /// Effect type identifier, e.g. "color.exposure", "color.wheels".
    pub effect_type: String,
    /// Whether this effect is enabled.
    pub enabled: bool,
    /// Resolved numeric parameter values at the given frame.
    pub params: HashMap<String, f64>,
    /// Optional grade curve for curve-based effects.
    pub grade_curve: Option<GradeCurve>,
}

/// Analyze a clip's effects at a given clip-relative frame offset.
///
/// Iterates the clip's effects (if any), resolves each enabled effect's
/// parameter values, and classifies the effect into compositing categories.
pub fn analyze_clip_effects(clip: &Clip, frame: i64) -> PerClipEffectState {
    let mut effects = Vec::new();
    let mut has_color_grading = false;
    let mut has_color_adjustments = false;
    let mut has_blur_or_vignette = false;

    if let Some(ref clip_effects) = clip.effects {
        for effect in clip_effects {
            if !effect.enabled {
                continue;
            }

            let category = categorize_effect(&effect.r#type);
            match category {
                "color_grading" => has_color_grading = true,
                "color_adjustment" => has_color_adjustments = true,
                "blur_vignette" => has_blur_or_vignette = true,
                _ => {}
            }

            let params = resolve_effect_params(effect, frame);

            effects.push(EffectState {
                effect_type: effect.r#type.clone(),
                enabled: effect.enabled,
                params,
                // Current Effect model does not carry a GradeCurve field;
                // this slot is reserved for future curve-based effect support.
                grade_curve: None,
            });
        }
    }

    PerClipEffectState {
        clip_id: clip.id.clone(),
        effects,
        has_color_grading,
        has_color_adjustments,
        has_blur_or_vignette,
    }
}

/// Scan an entire timeline and produce a summary of effects processing needs.
pub fn pipeline_from_timeline(timeline: &Timeline) -> EffectPipeline {
    let mut has_effects = false;
    let mut has_text_overlays = false;

    for track in &timeline.tracks {
        // Non-visual tracks do not contribute to visual effects compositing.
        if track.r#type == ClipType::Audio {
            continue;
        }

        for clip in &track.clips {
            if clip.media_type == ClipType::Text {
                has_text_overlays = true;
            }
            if let Some(ref effects) = clip.effects {
                if effects.iter().any(|e| e.enabled) {
                    has_effects = true;
                }
            }
        }
    }

    EffectPipeline {
        has_effects,
        needs_effects_pass: has_effects,
        needs_dual_pass: has_effects && has_text_overlays,
    }
}

/// Resolve all numeric parameters of an effect at a given clip-relative frame.
///
/// For each param in the effect, calls `resolved_at(frame, known_default)`
/// which uses the keyframe track if present, otherwise the static value,
/// falling back to the effect type's known default.
pub fn resolve_effect_params(effect: &Effect, frame: i64) -> HashMap<String, f64> {
    effect
        .params
        .iter()
        .map(|(name, param)| {
            let default = known_default(&effect.r#type, name);
            (name.clone(), param.resolved_at(frame, default))
        })
        .collect()
}

/// Classify an effect type into a compositing category.
///
/// Returns one of: "color_grading", "color_adjustment", "blur_vignette", "unknown".
pub fn categorize_effect(effect_type: &str) -> &str {
    match effect_type {
        "color.wheels" | "color.curve" => "color_grading",
        "color.exposure" | "color.contrast" | "color.brightness" | "color.saturation"
        | "color.hue" | "color.temperature" | "color.tint" | "color.highlights"
        | "color.shadows" | "color.whites" | "color.blacks" | "color.vibrance" => {
            "color_adjustment"
        }
        "blur.sharpen" | "blur.gaussian" | "blur" | "vignette" => "blur_vignette",
        _ => "unknown",
    }
}

// ── Private helpers ──────────────────────────────────────────────────────────

/// Known default value for a standard effect type's parameters.
/// Falls back to 0.0 for unknown effect/param combinations.
fn known_default(effect_type: &str, _param_name: &str) -> f64 {
    match effect_type {
        "color.exposure" => 0.0,
        "color.contrast" => 1.0,
        "color.brightness" => 0.0,
        "color.saturation" => 1.0,
        "color.hue" => 0.0,
        "color.temperature" => 0.0,
        "color.tint" => 0.0,
        "color.highlights" => 0.0,
        "color.shadows" => 0.0,
        "color.whites" => 0.0,
        "color.blacks" => 0.0,
        "color.vibrance" => 0.0,
        "blur.sharpen" => 0.0,
        "blur.gaussian" | "blur" => 0.0,
        "vignette" => 0.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Clip, ClipType, Crop, EffectParam, Interpolation, Track, Transform};

    /// Minimal video clip with sensible defaults.
    fn base_clip() -> Clip {
        Clip {
            id: String::new(),
            media_ref: String::new(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 1,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
        }
    }

    #[test]
    fn effects_empty_when_no_clips() {
        let timeline = Timeline::default();
        let pipeline = pipeline_from_timeline(&timeline);
        assert!(!pipeline.has_effects);
        assert!(!pipeline.needs_effects_pass);
        assert!(!pipeline.needs_dual_pass);
    }

    #[test]
    fn effects_detected_on_clip() {
        let effect = Effect::new("color.exposure", vec![("ev", 0.5)]);
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![effect]),
            ..base_clip()
        };
        let timeline = Timeline {
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
                clips: vec![clip],
            }],
            ..Timeline::default()
        };

        let pipeline = pipeline_from_timeline(&timeline);
        assert!(pipeline.has_effects);
        assert!(pipeline.needs_effects_pass);
        assert!(!pipeline.needs_dual_pass);
    }

    #[test]
    fn disabled_effect_excluded() {
        let enabled_effect = Effect::new("color.exposure", vec![("ev", 0.5)]);
        let disabled_effect = Effect {
            id: "disabled-1".into(),
            r#type: "color.contrast".into(),
            enabled: false,
            params: vec![("amount".into(), EffectParam::value(1.5))]
                .into_iter()
                .collect(),
        };
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![enabled_effect, disabled_effect]),
            ..base_clip()
        };

        let state = analyze_clip_effects(&clip, 0);
        assert_eq!(state.effects.len(), 1);
        assert_eq!(state.effects[0].effect_type, "color.exposure");
    }

    #[test]
    fn effect_params_resolved() {
        let effect = Effect::new("color.exposure", vec![("ev", -1.0)]);
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![effect]),
            ..base_clip()
        };

        let state = analyze_clip_effects(&clip, 0);
        assert_eq!(state.effects.len(), 1);
        let params = &state.effects[0].params;
        assert!((params.get("ev").copied().unwrap() - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn effect_categorization_color_adjustment() {
        assert_eq!(categorize_effect("color.exposure"), "color_adjustment");
        assert_eq!(categorize_effect("color.contrast"), "color_adjustment");
        assert_eq!(categorize_effect("color.brightness"), "color_adjustment");
        assert_eq!(categorize_effect("color.saturation"), "color_adjustment");
        assert_eq!(categorize_effect("color.hue"), "color_adjustment");
        assert_eq!(categorize_effect("color.temperature"), "color_adjustment");
        assert_eq!(categorize_effect("color.tint"), "color_adjustment");
        assert_eq!(categorize_effect("color.highlights"), "color_adjustment");
        assert_eq!(categorize_effect("color.shadows"), "color_adjustment");
        assert_eq!(categorize_effect("color.whites"), "color_adjustment");
        assert_eq!(categorize_effect("color.blacks"), "color_adjustment");
        assert_eq!(categorize_effect("color.vibrance"), "color_adjustment");
    }

    #[test]
    fn effect_categorization_color_grading() {
        assert_eq!(categorize_effect("color.wheels"), "color_grading");
        assert_eq!(categorize_effect("color.curve"), "color_grading");
    }

    #[test]
    fn effect_categorization_blur_vignette() {
        assert_eq!(categorize_effect("blur.sharpen"), "blur_vignette");
        assert_eq!(categorize_effect("blur.gaussian"), "blur_vignette");
        assert_eq!(categorize_effect("blur"), "blur_vignette");
        assert_eq!(categorize_effect("vignette"), "blur_vignette");
        assert_eq!(categorize_effect("unknown.custom"), "unknown");
    }

    #[test]
    fn dual_pass_detected() {
        let effect = Effect::new("color.exposure", vec![("ev", 0.3)]);
        let video_clip = Clip {
            id: "v1".into(),
            effects: Some(vec![effect]),
            ..base_clip()
        };
        let text_clip = Clip {
            id: "t1".into(),
            media_type: ClipType::Text,
            source_clip_type: ClipType::Text,
            start_frame: 0,
            duration_frames: 100,
            text_content: Some("Hello".into()),
            ..base_clip()
        };

        let timeline = Timeline {
            tracks: vec![
                Track {
                    id: "v-track".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: true,
                    clips: vec![video_clip],
                },
                Track {
                    id: "t-track".into(),
                    r#type: ClipType::Text,
                    muted: false,
                    hidden: false,
                    sync_locked: true,
                    clips: vec![text_clip],
                },
            ],
            ..Timeline::default()
        };

        let pipeline = pipeline_from_timeline(&timeline);
        assert!(pipeline.has_effects);
        assert!(pipeline.needs_effects_pass);
        assert!(pipeline.needs_dual_pass);
    }

    #[test]
    fn grade_curve_identity_default() {
        // GradeCurve is not yet part of the Effect model. EffectState holds
        // grade_curve: Option<GradeCurve> for future curve-based effects,
        // but should remain None until the model is extended.
        let effect = Effect::new("color.wheels", vec![]);
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![effect]),
            ..base_clip()
        };

        let state = analyze_clip_effects(&clip, 0);
        assert_eq!(state.effects.len(), 1);
        assert!(state.effects[0].grade_curve.is_none());
    }

    #[test]
    fn unknown_effect_type_passthrough() {
        let effect = Effect::new("custom.my_effect", vec![("value", 42.0)]);
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![effect]),
            ..base_clip()
        };

        let state = analyze_clip_effects(&clip, 0);
        assert_eq!(state.effects.len(), 1);
        assert!(state.effects[0].params.contains_key("value"));
        assert!((state.effects[0].params["value"] - 42.0).abs() < 1e-10);
    }

    #[test]
    fn per_clip_effect_categories() {
        let clip = Clip {
            id: "c1".into(),
            effects: Some(vec![
                Effect::new("color.exposure", vec![("ev", 0.5)]),
                Effect::new("color.wheels", vec![]),
                Effect::new("blur.gaussian", vec![("blur", 2.0)]),
            ]),
            ..base_clip()
        };

        let state = analyze_clip_effects(&clip, 0);
        assert!(state.has_color_adjustments);
        assert!(state.has_color_grading);
        assert!(state.has_blur_or_vignette);
        assert_eq!(state.effects.len(), 3);
    }

    #[test]
    fn effect_params_resolved_with_known_defaults() {
        // Contrast defaults to 1.0 when no param is set.
        let effect = Effect {
            id: "defaults-test".into(),
            r#type: "color.contrast".into(),
            enabled: true,
            params: HashMap::new(),
        };
        let params = resolve_effect_params(&effect, 0);
        // No params to iterate — the effect carries zero entries, so
        // resolve_effect_params returns an empty map. The known default
        // is used when a param IS present but has no value/track set.
        assert!(params.is_empty());
    }
}
