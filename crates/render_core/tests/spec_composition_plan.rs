//! Integration tests for composition plan specification items.
//!
//! PRV-004, PRV-005, RND-001, RND-007, RND-010.

use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use render_core::{CompositionPlan, DetailedCompositionPlan, RenderResolution};

/// Build a simple timeline with a single video clip.
fn make_single_clip_timeline() -> Timeline {
    Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        tracks: vec![Track {
            id: "v1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
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
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        tracks: vec![Track {
            id: "v".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
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
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        tracks: vec![Track {
            id: "v".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
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
