use core_model::{Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Transform};
use timeline_core::{clamp_clip_fades_to_duration, sample_keyframe_track, set_clip_duration};

fn clip(id: &str, start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: id.to_string(),
        media_ref: format!("asset-{id}"),
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

// INS-012: Keyframes remain clip-relative in storage.
// Verify keyframe frames are always relative to clip.start_frame (0-based).
#[test]
fn ins_012_keyframes_are_clip_relative_in_storage() {
    // Keyframe frames on a clip are stored as 0-based offsets.
    // A clip at start_frame=100 with keyframe at frame=50 means
    // the keyframe fires at timeline frame 150.
    let mut clip = clip("c1", 100, 200);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 200,
                value: 1.0,
                interpolation_out: Interpolation::Linear,
            },
        ],
    });
    // The clip positioning is independent of keyframe storage:
    // start_frame is not stored inside keyframes
    assert_eq!(clip.opacity_track.as_ref().unwrap().keyframes[0].frame, 0);
    assert_eq!(clip.opacity_track.as_ref().unwrap().keyframes[1].frame, 200);
    // Sampling at various clip-relative positions
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 0, 0.0) - 0.0).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 100, 0.0) - 0.5).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 200, 0.0) - 1.0).abs() < 0.001
    );
}

// INS-013: Keyframe interpolation modes: linear, hold, smooth.
#[test]
fn ins_013_linear_interpolation() {
    let mut clip = clip("c1", 0, 100);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 100,
                value: 1.0,
                interpolation_out: Interpolation::Linear,
            },
        ],
    });
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 0, 0.0) - 0.0).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 50, 0.0) - 0.5).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 100, 0.0) - 1.0).abs() < 0.001
    );
}

#[test]
fn ins_013_hold_interpolation() {
    let mut clip = clip("c1", 0, 100);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Hold,
            },
            Keyframe {
                frame: 100,
                value: 1.0,
                interpolation_out: Interpolation::Hold,
            },
        ],
    });
    // Hold clamps to the value of the preceding keyframe
    let v_at_0 = sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 0, 999.0);
    assert!(
        (v_at_0 - 0.0).abs() < 0.001,
        "hold at frame 0 should be first value"
    );
    let v_at_50 = sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 50, 999.0);
    assert!(
        (v_at_50 - 0.0).abs() < 0.001,
        "hold at frame 50 should still be 0.0"
    );
    let v_at_100 = sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 100, 999.0);
    assert!(
        (v_at_100 - 1.0).abs() < 0.001,
        "hold at last keyframe should be its value"
    );
}

#[test]
fn ins_013_smooth_interpolation() {
    let mut clip = clip("c1", 0, 100);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Smooth,
            },
            Keyframe {
                frame: 100,
                value: 1.0,
                interpolation_out: Interpolation::Smooth,
            },
        ],
    });
    let v_at_50 = sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 50, 999.0);
    // Smooth = smoothstep(0.5) = 0.5^2 * (3 - 2*0.5) = 0.25 * 2.0 = 0.5
    assert!(
        (v_at_50 - 0.5).abs() < 0.001,
        "smooth at midpoint should be 0.5, got {}",
        v_at_50
    );
    // At 25%: t=0.25, smoothstep(0.25)=0.15625, value=0.15625
    let v_at_25 = sample_keyframe_track(clip.opacity_track.as_ref().unwrap(), 25, 999.0);
    assert!(
        (v_at_25 - 0.15625).abs() < 0.001,
        "smooth at 25 should be 0.15625, got {}",
        v_at_25
    );
}

// INS-014: Duplicate keyframes at the same frame collapse with last-value-wins.
#[test]
fn ins_014_duplicate_keyframes_collapse_last_value_wins() {
    let mut clip = clip("c1", 0, 60);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 30,
                value: 0.5,
                interpolation_out: Interpolation::Smooth,
            },
            Keyframe {
                frame: 60,
                value: 1.0,
                interpolation_out: Interpolation::Linear,
            },
        ],
    });
    // Set the same duration — clamping should not change anything
    set_clip_duration(&mut clip, 60);
    assert_eq!(clip.opacity_track.as_ref().unwrap().keyframes.len(), 3);
    // Now simulate upsert by adding keyframes with a duplicate frame
    let kf1 = Keyframe {
        frame: 0,
        value: 1.0,
        interpolation_out: Interpolation::Linear,
    };
    let kf2 = Keyframe {
        frame: 10,
        value: 2.0,
        interpolation_out: Interpolation::Smooth,
    };
    let kf3 = Keyframe {
        frame: 10,
        value: 3.0,
        interpolation_out: Interpolation::Hold,
    }; // duplicate frame
    let kf4 = Keyframe {
        frame: 20,
        value: 4.0,
        interpolation_out: Interpolation::Linear,
    };
    // set_clip_duration -> clamp_clip_keyframes_to_duration -> clamp_keyframe_track
    // which calls upsert_keyframe for each keyframe (last-value-wins for duplicates)
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![kf1, kf2, kf3, kf4],
    });
    set_clip_duration(&mut clip, 60);
    let track_ref = clip.opacity_track.as_ref().unwrap();
    assert_eq!(
        track_ref.keyframes.len(),
        3,
        "should have 3 unique frames after clamp/upsert"
    );
    let at_10 = track_ref
        .keyframes
        .iter()
        .find(|kf| kf.frame == 10)
        .unwrap();
    assert!(
        (at_10.value - 3.0).abs() < 0.001,
        "last-value-wins: expected 3.0, got {}",
        at_10.value
    );
    assert_eq!(
        at_10.interpolation_out,
        Interpolation::Hold,
        "last-value-wins should also update interpolation_out"
    );
}

// INS-015: Fade lengths clamped so fadeInFrames + fadeOutFrames <= durationFrames.
#[test]
fn ins_015_fade_clamping_sums_to_duration() {
    let mut clip = clip("c1", 0, 60);
    clip.fade_in_frames = 40;
    clip.fade_out_frames = 40;
    // fadeIn + fadeOut = 80 > 60, should clamp fadeOut to 20
    clamp_clip_fades_to_duration(&mut clip);
    assert_eq!(clip.fade_in_frames, 40);
    assert_eq!(clip.fade_out_frames, 20);
    assert_eq!(clip.fade_in_frames + clip.fade_out_frames, 60);
}

#[test]
fn ins_015_fade_clamping_respects_zero() {
    let mut clip = clip("c1", 0, 60);
    clip.fade_in_frames = -5;
    clip.fade_out_frames = -5;
    clamp_clip_fades_to_duration(&mut clip);
    assert_eq!(clip.fade_in_frames, 0);
    assert_eq!(clip.fade_out_frames, 0);
}

#[test]
fn ins_015_fade_clamping_keeps_valid_values() {
    let mut clip = clip("c1", 0, 100);
    clip.fade_in_frames = 20;
    clip.fade_out_frames = 30;
    clamp_clip_fades_to_duration(&mut clip);
    assert_eq!(clip.fade_in_frames, 20);
    assert_eq!(clip.fade_out_frames, 30);
}

#[test]
fn ins_015_set_duration_clamps_fades() {
    let mut clip = clip("c1", 0, 100);
    clip.fade_in_frames = 40;
    clip.fade_out_frames = 80;
    // fadeIn + fadeOut = 120 > 100
    set_clip_duration(&mut clip, 50);
    assert_eq!(clip.fade_in_frames, 40);
    assert_eq!(clip.fade_out_frames, 10);
}
