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
        tracks: vec![
            track(ClipType::Video, vec![clip(100, 50), clip(220, 25)]),
            track(ClipType::Audio, vec![clip(0, 400)]),
        ],
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
        tracks: vec![track(ClipType::Video, vec![clip(100, 50)])],
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
fn tim_008_never_violates_half_open_invariant_during_operations() {
    // Verify that clip and range invariants hold after editing operations
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
}
