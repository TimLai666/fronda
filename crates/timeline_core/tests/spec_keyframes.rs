use core_model::{
    Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Timeline, Track, Transform,
};
use timeline_core::{
    apply_fps, clamp_clip_fades_to_duration, sample_keyframe_track, set_clip_duration,
    split_all_clip_keyframe_tracks,
};

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
        effects: None,
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        text_animation: None,
        word_timings: None,
    }
}

#[test]
fn set_duration_rescales_word_timings_on_text_clips() {
    use core_model::WordTiming;
    let mut c = clip("t", 0, 30);
    c.media_type = ClipType::Text;
    c.word_timings = Some(vec![
        WordTiming { text: "Hello".into(), start_frame: 0, end_frame: 15 },
        WordTiming { text: "World".into(), start_frame: 15, end_frame: 30 },
    ]);
    // Doubling the duration doubles the word timings (Swift rescaleWordTimings).
    set_clip_duration(&mut c, 60);
    let t = c.word_timings.as_ref().unwrap();
    assert_eq!((t[0].start_frame, t[0].end_frame), (0, 30));
    assert_eq!((t[1].start_frame, t[1].end_frame), (30, 60));
}

#[test]
fn set_duration_leaves_non_text_word_timings_untouched() {
    use core_model::WordTiming;
    // word_timings only apply to text clips; a non-text clip is never rescaled.
    let mut c = clip("v", 0, 30); // Video
    c.word_timings = Some(vec![WordTiming {
        text: "x".into(),
        start_frame: 10,
        end_frame: 20,
    }]);
    set_clip_duration(&mut c, 60);
    let t = c.word_timings.as_ref().unwrap();
    assert_eq!((t[0].start_frame, t[0].end_frame), (10, 20));
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

fn test_clip(id: &str, start_frame: i64, duration_frames: i64) -> Clip {
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
        effects: None,
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        text_animation: None,
        word_timings: None,
    }
}

// INS-016: Audio volume keyframes support direct editing in time and dB/value space
// while respecting neighboring keyframe ordering.
#[test]
fn ins_016_volume_keyframes_sample_linear() {
    let mut clip = test_clip("c1", 0, 100);
    clip.volume_track = Some(KeyframeTrack {
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
        (sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 0, 999.0) - 0.0).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 50, 999.0) - 0.5).abs() < 0.001
    );
    assert!(
        (sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 100, 999.0) - 1.0).abs()
            < 0.001
    );
}

#[test]
fn ins_016_volume_keyframes_clamp_on_duration_change() {
    let mut clip = test_clip("c1", 0, 100);
    clip.volume_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 100,
                value: 1.0,
                interpolation_out: Interpolation::Smooth,
            },
        ],
    });
    // Shrink duration — keyframe at 100 should be removed
    set_clip_duration(&mut clip, 50);
    assert_eq!(clip.volume_track.as_ref().unwrap().keyframes.len(), 1);
    assert_eq!(clip.volume_track.as_ref().unwrap().keyframes[0].frame, 0);
}

#[test]
fn ins_016_volume_keyframes_upsert_last_value_wins() {
    let track = KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 10,
                value: 0.5,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 10,
                value: 0.8,
                interpolation_out: Interpolation::Hold,
            },
        ],
    };
    let mut clip = test_clip("c1", 0, 100);
    clip.volume_track = Some(track);
    set_clip_duration(&mut clip, 100);
    let kf = clip
        .volume_track
        .as_ref()
        .unwrap()
        .keyframes
        .iter()
        .find(|k| k.frame == 10)
        .unwrap();
    assert!(
        (kf.value - 0.8).abs() < 0.001,
        "last-value-wins: expected 0.8, got {}",
        kf.value
    );
    assert_eq!(kf.interpolation_out, Interpolation::Hold);
}

#[test]
fn ins_016_volume_keyframes_interpolation_modes() {
    let mut clip = test_clip("c1", 0, 100);
    clip.volume_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 50,
                value: 0.5,
                interpolation_out: Interpolation::Hold,
            },
            Keyframe {
                frame: 100,
                value: 1.0,
                interpolation_out: Interpolation::Linear,
            },
        ],
    });
    // Before hold keyframe: linear from 0 to 0.5
    let v25 = sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 25, 999.0);
    assert!(
        (v25 - 0.25).abs() < 0.001,
        "linear before hold: expected 0.25, got {}",
        v25
    );
    // At hold keyframe frame: value is 0.5
    let v50 = sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 50, 999.0);
    assert!(
        (v50 - 0.5).abs() < 0.001,
        "hold at frame: expected 0.5, got {}",
        v50
    );
    // After hold keyframe with hold interpolation: stays at 0.5
    let v75 = sample_keyframe_track(clip.volume_track.as_ref().unwrap(), 75, 999.0);
    assert!(
        (v75 - 0.5).abs() < 0.001,
        "hold after frame: expected 0.5, got {}",
        v75
    );
}

// PCFG-004 regression: the stroke_progress_track (7th keyframe track, Rust-only
// from #46) participates in fps rescale alongside its six siblings.
#[test]
fn stroke_progress_track_rescales_on_fps_change() {
    let mut c = clip("s", 0, 100);
    c.stroke_progress_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe { frame: 0, value: 0.0, interpolation_out: Interpolation::Linear },
            Keyframe { frame: 50, value: 1.0, interpolation_out: Interpolation::Linear },
        ],
    });
    let mut timeline = Timeline {
        fps: 30,
        tracks: vec![Track {
            id: "t".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips: vec![c],
        }],
        ..Timeline::default()
    };
    apply_fps(&mut timeline, 60); // scale 2.0
    let track = timeline.tracks[0].clips[0]
        .stroke_progress_track
        .as_ref()
        .expect("stroke track preserved");
    assert_eq!(track.keyframes[0].frame, 0);
    assert_eq!(track.keyframes[1].frame, 100, "50 → 100 at 2x fps");
}

// Splitting a clip partitions the stroke_progress_track like the other tracks.
#[test]
fn stroke_progress_track_splits_with_clip() {
    let mut c = clip("s", 0, 100);
    c.stroke_progress_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe { frame: 0, value: 0.0, interpolation_out: Interpolation::Linear },
            Keyframe { frame: 100, value: 1.0, interpolation_out: Interpolation::Linear },
        ],
    });
    let (left, right) = split_all_clip_keyframe_tracks(&c, 40);
    let l = left.stroke_progress_track.as_ref().expect("left stroke");
    let r = right.stroke_progress_track.as_ref().expect("right stroke");
    // Left ends with a boundary keyframe at the split point.
    assert_eq!(l.keyframes.last().unwrap().frame, 40);
    // Right is reindexed to 0 and keeps the tail keyframe at 100-40=60.
    assert_eq!(r.keyframes.first().unwrap().frame, 0);
    assert_eq!(r.keyframes.last().unwrap().frame, 60);
}
