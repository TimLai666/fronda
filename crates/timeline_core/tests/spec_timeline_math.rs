use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use proptest::prelude::*;
use timeline_core::{is_valid_half_open_range, ClipMathExt, TimelineMathExt};

fn clip(start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: "clip-1".to_string(),
        media_ref: "asset-1".to_string(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame,
        duration_frames,
        trim_start_frame: 0,
        trim_end_frame: 0,
        speed: 1.0,
        volume: 1.0,
        fade_in_frames: 0,
        fade_out_frames: 0,
        fade_in_interpolation: Interpolation::Linear,
        fade_out_interpolation: Interpolation::Linear,
        opacity: 1.0,
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
    }
}

fn track(kind: ClipType, clips: Vec<Clip>) -> Track {
    Track {
        id: format!("{:?}-track", kind),
        r#type: kind,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

#[test]
fn tim_001_total_frames_is_max_clip_end_across_tracks() {
    let timeline = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![
            track(ClipType::Video, vec![clip(100, 50), clip(220, 25)]),
            track(ClipType::Audio, vec![clip(0, 400)]),
        ],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    assert_eq!(timeline.total_frames(), 400);
}

#[test]
fn tim_002_clip_contains_uses_half_open_interval() {
    let clip = clip(50, 30);

    assert!(clip.contains_frame(50));
    assert!(clip.contains_frame(79));
    assert!(!clip.contains_frame(80));
    assert!(!clip.contains_frame(49));
}

#[test]
fn tim_003_end_frame_is_start_plus_duration() {
    let clip = clip(100, 50);

    assert_eq!(clip.end_frame(), 150);
}

#[test]
fn tim_004_source_frames_consumed_rounds_duration_times_speed() {
    let mut clip = clip(0, 33);
    clip.speed = 0.75;

    assert_eq!(clip.source_frames_consumed(), 25);
}

#[test]
fn tim_005_source_duration_includes_both_trims() {
    let mut clip = clip(0, 100);
    clip.trim_start_frame = 10;
    clip.trim_end_frame = 5;

    assert_eq!(clip.source_duration_frames(), 115);
}

#[test]
fn tim_006_seek_clamps_into_zero_to_total_frames() {
    let timeline = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: false,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![track(ClipType::Video, vec![clip(100, 50)])],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    assert_eq!(timeline.clamp_seek_frame(-20), 0);
    assert_eq!(timeline.clamp_seek_frame(120), 120);
    assert_eq!(timeline.clamp_seek_frame(1000), 150);
}

#[test]
fn tim_007_half_open_range_requires_end_after_start() {
    assert!(is_valid_half_open_range(10, 11));
    assert!(!is_valid_half_open_range(10, 10));
    assert!(!is_valid_half_open_range(10, 9));
}

#[test]
fn tim_008_half_open_invariant_after_speed_change_and_trim() {
    // Chain operations that should preserve the half-open invariant
    let mut clip = clip(100, 50);
    assert_eq!(clip.end_frame(), 150);
    assert!(clip.contains_frame(100));
    assert!(clip.contains_frame(149));
    assert!(!clip.contains_frame(150));

    // After speed change
    clip.speed = 2.0;
    clip.duration_frames = ((clip.duration_frames as f64 * 1.0) / 2.0).round() as i64;
    assert_eq!(clip.end_frame(), 100 + clip.duration_frames);
    assert!(!clip.contains_frame(clip.end_frame()));

    // After trim
    clip.start_frame = 120;
    clip.duration_frames = 30;
    assert_eq!(clip.end_frame(), 150);
    assert!(clip.contains_frame(120));
    assert!(!clip.contains_frame(150));
}

#[test]
fn tim_008_half_open_invariant_after_split() {
    use timeline_core::split_clip;
    let c1 = clip(0, 100);
    let mut timeline = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![track(ClipType::Video, vec![c1])],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    let _right_ids = split_clip(&mut timeline, "clip-1", 40);
    assert_half_open_invariants(&timeline);
}

#[test]
fn tim_008_half_open_invariant_after_clear_region() {
    use timeline_core::clear_region;
    let c1 = clip(20, 60);
    let c2 = clip(100, 40);
    let mut timeline = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![track(ClipType::Video, vec![c1, c2])],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    // Clear region covering c1's end and c2's start
    clear_region(&mut timeline, 0, 50, 120, false);
    assert_half_open_invariants(&timeline);
}

#[test]
fn tim_008_half_open_invariant_after_split_then_speed() {
    use timeline_core::{apply_clip_speed, split_clip};
    let c1 = clip(0, 100);
    let mut timeline = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![track(ClipType::Video, vec![c1])],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    let right_ids = split_clip(&mut timeline, "clip-1", 50);
    assert_eq!(right_ids.len(), 1);
    assert_half_open_invariants(&timeline);

    // Speed change the right half
    assert!(apply_clip_speed(&mut timeline, &right_ids[0], 2.0));
    assert_half_open_invariants(&timeline);
}

fn assert_half_open_invariants(timeline: &Timeline) {
    for (ti, track) in timeline.tracks.iter().enumerate() {
        for clip in &track.clips {
            assert_eq!(
                clip.end_frame(),
                clip.start_frame + clip.duration_frames,
                "track {ti} clip {} end_frame mismatch: start={}, duration={}",
                clip.id,
                clip.start_frame,
                clip.duration_frames
            );
            assert!(
                !clip.contains_frame(clip.end_frame()),
                "track {ti} clip {} contains its own end_frame {}",
                clip.id,
                clip.end_frame()
            );
        }
    }
}

proptest! {
    #[test]
    fn tim_002_and_tim_003_hold_for_generated_clips(
        start_frame in 0_i64..1_000_000,
        duration_frames in 0_i64..10_000,
    ) {
        let clip = clip(start_frame, duration_frames);

        prop_assert_eq!(clip.end_frame(), start_frame + duration_frames);

        if duration_frames == 0 {
            prop_assert!(!clip.contains_frame(start_frame));
        } else {
            prop_assert!(clip.contains_frame(start_frame));
            prop_assert!(clip.contains_frame(start_frame + duration_frames - 1));
            prop_assert!(!clip.contains_frame(start_frame + duration_frames));
        }
    }

    #[test]
    fn tim_004_and_tim_005_hold_for_generated_speed_and_trims(
        duration_frames in 0_i64..100_000,
        trim_start_frame in 0_i64..10_000,
        trim_end_frame in 0_i64..10_000,
        speed_millis in 0_u32..5_000,
    ) {
        let mut clip = clip(0, duration_frames);
        clip.trim_start_frame = trim_start_frame;
        clip.trim_end_frame = trim_end_frame;
        clip.speed = speed_millis as f64 / 1_000.0;

        let expected_consumed = ((duration_frames as f64) * clip.speed).round() as i64;

        prop_assert_eq!(clip.source_frames_consumed(), expected_consumed);
        prop_assert_eq!(
            clip.source_duration_frames(),
            expected_consumed + trim_start_frame + trim_end_frame,
        );
    }

    #[test]
    fn tim_008_half_open_invariant_after_speed_and_trim(
        start_frame in 0_i64..500_000,
        duration_frames in 1_i64..10_000,
        speed_millis in 100_u32..5_000,
        trim_start in 0_i64..500,
        trim_end in 0_i64..500,
        new_start_delta in (-2000_i64..2000),
    ) {
        let mut clip = clip(start_frame, duration_frames);
        clip.trim_start_frame = trim_start;
        clip.trim_end_frame = trim_end;

        // Set speed (must be > 0)
        let speed = speed_millis as f64 / 1_000.0;
        clip.speed = speed;

        // Speed change recomputes duration
        let source_frames = (duration_frames as f64) * speed;
        clip.duration_frames = (source_frames / speed).round() as i64;
        prop_assert_eq!(clip.end_frame(), start_frame + clip.duration_frames);
        prop_assert!(!clip.contains_frame(clip.end_frame()));

        // Apply start_frame change (simulating trim/move)
        let new_start = (start_frame as i64 + new_start_delta).max(0);
        clip.start_frame = new_start;
        prop_assert_eq!(clip.end_frame(), new_start + clip.duration_frames);
        prop_assert!(!clip.contains_frame(clip.end_frame()));
    }
}
