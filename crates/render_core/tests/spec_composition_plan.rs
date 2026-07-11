//! Integration tests for composition plan specification items.
//!
//! PRV-004, PRV-005, RND-001, RND-007, RND-010,
//! SAV-004, SAV-005, SAV-009, RND-013, PRV-012, PRV-013.

use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use render_core::{CompositionPlan, DetailedCompositionPlan, RenderResolution};

/// Build a simple timeline with a single video clip.
fn make_single_clip_timeline() -> Timeline {
    Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![Track {
            id: "v1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![Clip {
                id: "clip1".into(),
                media_ref: "asset-video.mp4".into(),
                media_type: ClipType::Video,
                source_clip_type: ClipType::Video,
                start_frame: 0,
                duration_frames: 100,
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
                shape_style: None,
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                multicam_group_id: None,
                text_animation: None,
                word_timings: None,
            }],
        }],
    }
}

fn make_base_clip() -> Clip {
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
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        multicam_group_id: None,
        text_animation: None,
        word_timings: None,
    }
}

// ---------------------------------------------------------------------------
// PRV-004: Invalid timeline settings cause build failure
// ---------------------------------------------------------------------------
#[test]
fn prv_004_zero_fps_causes_validation_failure() {
    let mut timeline = make_single_clip_timeline();
    timeline.fps = 0;
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let validation = plan.validate();
    assert!(!validation.is_valid, "zero fps should be invalid");
    assert!(
        validation.errors.iter().any(|e| e.contains("fps")),
        "error should mention fps"
    );
}

#[test]
fn prv_004_negative_fps_causes_validation_failure() {
    let mut timeline = make_single_clip_timeline();
    timeline.fps = -1;
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let validation = plan.validate();
    assert!(!validation.is_valid, "negative fps should be invalid");
    assert!(
        validation.errors.iter().any(|e| e.contains("fps")),
        "error should mention fps"
    );
}

#[test]
fn prv_004_tiny_resolution_causes_validation_failure() {
    let timeline = make_single_clip_timeline();
    let tiny = RenderResolution {
        width: 1,
        height: 1,
    };
    let plan = CompositionPlan::from_timeline(&timeline, tiny);
    let validation = plan.validate();
    assert!(!validation.is_valid, "tiny resolution should be invalid");
}

// ---------------------------------------------------------------------------
// PRV-005: Offline media are skipped (plan remains valid with offline refs)
// ---------------------------------------------------------------------------
#[test]
fn prv_005_offline_media_does_not_invalidate_plan() {
    let timeline = make_single_clip_timeline();
    let mut plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    // Add offline media refs
    plan.offline_media_refs.push("missing-file.mov".into());
    plan.offline_media_refs.push("offline-audio.wav".into());
    let validation = plan.validate();
    assert!(
        validation.is_valid,
        "offline media should not invalidate plan"
    );
    assert_eq!(plan.offline_media_refs.len(), 2);
}

#[test]
fn prv_005_offline_and_unprocessable_are_distinct() {
    let timeline = make_single_clip_timeline();
    let mut plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    plan.offline_media_refs.push("missing.mov".into());
    plan.unprocessable_media_refs.push("corrupt.mp4".into());
    assert!(plan.offline_media_refs != plan.unprocessable_media_refs);
    assert_eq!(plan.offline_media_refs, vec!["missing.mov"]);
    assert_eq!(plan.unprocessable_media_refs, vec!["corrupt.mp4"]);
}

// ---------------------------------------------------------------------------
// RND-001: Non-positive fps / canvas size rejected
// ---------------------------------------------------------------------------
#[test]
fn rnd_001_validation_rejects_non_positive_fps() {
    let timeline = make_single_clip_timeline();
    // Zero fps
    let mut zero_fps = timeline.clone();
    zero_fps.fps = 0;
    let plan = CompositionPlan::from_timeline(&zero_fps, RenderResolution::native(&zero_fps));
    assert!(!plan.validate().is_valid, "zero fps should be rejected");

    // Negative fps
    let mut neg_fps = timeline;
    neg_fps.fps = -5;
    let plan2 = CompositionPlan::from_timeline(&neg_fps, RenderResolution::native(&neg_fps));
    assert!(
        !plan2.validate().is_valid,
        "negative fps should be rejected"
    );
}

#[test]
fn rnd_001_validation_rejects_tiny_canvas() {
    let timeline = make_single_clip_timeline();
    let tiny = RenderResolution {
        width: 0,
        height: 0,
    };
    let plan = CompositionPlan::from_timeline(&timeline, tiny);
    assert!(
        !plan.validate().is_valid,
        "zero resolution should be rejected"
    );

    let too_small = RenderResolution {
        width: 1,
        height: 1,
    };
    let plan2 = CompositionPlan::from_timeline(&timeline, too_small);
    assert!(
        !plan2.validate().is_valid,
        "sub-2-pixel resolution should be rejected"
    );
}

// ---------------------------------------------------------------------------
// RND-007: Full-duration black background inserted when no clip at frame 0
// ---------------------------------------------------------------------------
#[test]
fn rnd_007_black_background_when_no_clip_at_frame_zero() {
    let mut timeline = make_single_clip_timeline();
    // Move all clips to start at frame 50
    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            clip.start_frame = 50;
        }
    }
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert!(
        detailed.needs_black_background,
        "should need black bg when clip starts at 50"
    );
    assert!(
        detailed.black_background_duration > 0,
        "black bg duration should be > 0"
    );
    assert_eq!(
        detailed.black_background_duration, detailed.plan.total_frames,
        "black bg should span full timeline when first clip is not at frame 0"
    );
}

#[test]
fn rnd_007_no_black_background_when_clip_at_frame_zero() {
    let timeline = make_single_clip_timeline();
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert!(
        !detailed.needs_black_background,
        "no black bg needed when clip starts at frame 0"
    );
    assert_eq!(detailed.black_background_duration, 0);
}

#[test]
fn rnd_007_black_background_when_all_visual_tracks_hidden() {
    let mut timeline = make_single_clip_timeline();
    timeline.tracks[0].hidden = true;
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert!(
        detailed.needs_black_background,
        "black bg needed when all visual tracks are hidden"
    );
    assert!(detailed.black_background_duration > 0);
}

// ---------------------------------------------------------------------------
// RND-010: Same-track visual clips are sorted and non-overlapping
//   (valid timeline should have no overlap warnings)
// ---------------------------------------------------------------------------
#[test]
fn rnd_010_non_overlapping_clips_produce_no_warnings() {
    let v1 = Clip {
        id: "v1".into(),
        media_ref: "a.mp4".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 0,
        duration_frames: 50,
        ..make_base_clip()
    };
    let v2 = Clip {
        id: "v2".into(),
        media_ref: "b.mp4".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 50,
        duration_frames: 50,
        ..make_base_clip()
    };
    let timeline = Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![Track {
            id: "v".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![v1, v2],
        }],
    };
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let validation = plan.validate();
    assert!(validation.is_valid, "non-overlapping clips should be valid");
    assert!(
        validation.warnings.is_empty(),
        "non-overlapping clips should not produce warnings"
    );
}

#[test]
fn rnd_010_overlapping_visual_clips_produce_warnings() {
    let v1 = Clip {
        id: "v1".into(),
        media_ref: "a.mp4".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 0,
        duration_frames: 60,
        ..make_base_clip()
    };
    let v2 = Clip {
        id: "v2".into(),
        media_ref: "b.mp4".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 30,
        duration_frames: 60,
        ..v1.clone()
    };
    let timeline = Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![Track {
            id: "v".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![v1, v2],
        }],
    };
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let validation = plan.validate();
    assert!(
        validation.is_valid,
        "overlapping clips should still be valid (overlap is a warning)"
    );
    assert!(
        !validation.warnings.is_empty(),
        "overlapping clips should produce warnings"
    );
}

// ---------------------------------------------------------------------------
// SAV-004: Clip trim is propagated into CompositionClip
// ---------------------------------------------------------------------------
#[test]
fn sav_004_trim_propagated_to_composition_clip() {
    let mut timeline = make_single_clip_timeline();
    // Modify the clip to have non-zero trim values
    timeline.tracks[0].clips[0].trim_start_frame = 10;
    timeline.tracks[0].clips[0].trim_end_frame = 5;
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let clip = &plan.tracks[0].clips[0];
    assert_eq!(
        clip.source_trim_start, 10,
        "SAV-004: source_trim_start must reflect clip.trim_start_frame"
    );
    assert_eq!(
        clip.source_trim_end, 5,
        "SAV-004: source_trim_end must reflect clip.trim_end_frame"
    );
}

// ---------------------------------------------------------------------------
// SAV-005: Clip speed is propagated into CompositionClip
// ---------------------------------------------------------------------------
#[test]
fn sav_005_speed_propagated_to_composition_clip() {
    let mut timeline = make_single_clip_timeline();
    timeline.tracks[0].clips[0].speed = 2.0;
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let clip = &plan.tracks[0].clips[0];
    assert!(
        (clip.speed - 2.0).abs() < 1e-9,
        "SAV-005: speed must be 2.0, got {}",
        clip.speed
    );
}

// ---------------------------------------------------------------------------
// SAV-009: Trim and speed together flow into DetailedCompositionPlan
// ---------------------------------------------------------------------------
#[test]
fn sav_009_trim_and_speed_in_detailed_plan() {
    let mut timeline = make_single_clip_timeline();
    timeline.tracks[0].clips[0].trim_start_frame = 15;
    timeline.tracks[0].clips[0].trim_end_frame = 3;
    timeline.tracks[0].clips[0].speed = 0.5;
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    // The plan has one video clip in the main track; confirm trim+speed flow through
    assert!(!detailed.plan.tracks.is_empty(), "plan must have tracks");
    let vc = &detailed.plan.tracks[0].clips[0];
    assert_eq!(
        vc.source_trim_start, 15,
        "SAV-009: trim_start must flow through"
    );
    assert_eq!(vc.source_trim_end, 3, "SAV-009: trim_end must flow through");
    assert!(
        (vc.speed - 0.5).abs() < 1e-9,
        "SAV-009: speed must flow through"
    );
}

// ---------------------------------------------------------------------------
// RND-013: Opacity value is faithfully propagated into CompositionClip
// ---------------------------------------------------------------------------
#[test]
fn rnd_013_opacity_propagated() {
    let mut timeline = make_single_clip_timeline();
    timeline.tracks[0].clips[0].opacity = 0.5;
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let clip = &plan.tracks[0].clips[0];
    assert!(
        (clip.opacity - 0.5).abs() < 1e-9,
        "RND-013: opacity=0.5 must propagate; got {}",
        clip.opacity
    );
}

#[test]
fn rnd_013_full_opacity_stays_one() {
    let timeline = make_single_clip_timeline();
    let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    let clip = &plan.tracks[0].clips[0];
    assert!(
        (clip.opacity - 1.0).abs() < 1e-9,
        "RND-013: default opacity must be 1.0"
    );
}

// ---------------------------------------------------------------------------
// PRV-012: image_clips in DetailedCompositionPlan
// ---------------------------------------------------------------------------
fn make_image_timeline() -> Timeline {
    Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![Track {
            id: "v1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            display_height: 50.0,
            clips: vec![Clip {
                id: "img-1".into(),
                media_ref: "ref-img".into(),
                media_type: ClipType::Image,
                source_clip_type: ClipType::Image,
                ..make_base_clip()
            }],
        }],
    }
}

#[test]
fn prv_012_image_clips_in_detailed_plan() {
    let timeline = make_image_timeline();
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert_eq!(
        detailed.image_clips.len(),
        1,
        "PRV-012: DetailedCompositionPlan.image_clips must contain 1 image clip"
    );
    assert_eq!(detailed.image_clips[0].clip_id, "img-1");
}

#[test]
fn prv_012_video_clip_not_in_image_clips() {
    let timeline = make_single_clip_timeline();
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert!(
        detailed.image_clips.is_empty(),
        "PRV-012: video clip must not appear in image_clips"
    );
}

// ---------------------------------------------------------------------------
// PRV-013: lottie_clips in DetailedCompositionPlan
// ---------------------------------------------------------------------------
fn make_lottie_timeline() -> Timeline {
    Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![Track {
            id: "v1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            display_height: 50.0,
            clips: vec![Clip {
                id: "lottie-1".into(),
                media_ref: "ref-lottie".into(),
                media_type: ClipType::Lottie,
                source_clip_type: ClipType::Lottie,
                ..make_base_clip()
            }],
        }],
    }
}

#[test]
fn prv_013_lottie_clips_in_detailed_plan() {
    let timeline = make_lottie_timeline();
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert_eq!(
        detailed.lottie_clips.len(),
        1,
        "PRV-013: DetailedCompositionPlan.lottie_clips must contain 1 lottie clip"
    );
    assert_eq!(detailed.lottie_clips[0].clip_id, "lottie-1");
}

#[test]
fn prv_013_image_clip_not_in_lottie_clips() {
    let timeline = make_image_timeline();
    let detailed =
        DetailedCompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
    assert!(
        detailed.lottie_clips.is_empty(),
        "PRV-013: image clip must not appear in lottie_clips"
    );
}
