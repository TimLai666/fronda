use core_model::{Clip, ClipType, Crop, Interpolation, Track, Transform};
use std::collections::BTreeSet;
use timeline_core::{
    compute_ripple_push, compute_ripple_shifts, compute_ripple_shifts_for_ranges, merge_ranges,
    validate_track_shifts, ClipShift, FrameRange, RippleValidationError,
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

fn track(clips: Vec<Clip>) -> Track {
    Track {
        id: "track-1".to_string(),
        r#type: ClipType::Video,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

#[test]
fn rpl_001_compute_ripple_shifts_removing_middle_clip_shifts_trailing_clip_left() {
    let removed = clip("r", 50, 50);
    let trailing = clip("t", 200, 50);
    let head = clip("h", 0, 50);
    let removed_ids = BTreeSet::from(["r".to_string()]);

    let shifts = compute_ripple_shifts(&[head, removed, trailing], &removed_ids);
    assert_eq!(
        shifts,
        vec![ClipShift {
            clip_id: "t".to_string(),
            new_start_frame: 150,
        }]
    );
}

#[test]
fn rpl_001_compute_ripple_shifts_ignores_clips_before_removed_range() {
    let head = clip("h", 0, 50);
    let removed = clip("r", 100, 50);
    let removed_ids = BTreeSet::from(["r".to_string()]);

    assert!(compute_ripple_shifts(&[head, removed], &removed_ids).is_empty());
}

#[test]
fn rpl_001_compute_ripple_shifts_accumulate_merged_removed_lengths() {
    let r1 = clip("r1", 0, 50);
    let r2 = clip("r2", 100, 50);
    let tail = clip("t", 200, 50);
    let removed_ids = BTreeSet::from(["r1".to_string(), "r2".to_string()]);

    let shifts = compute_ripple_shifts(&[r1, r2, tail], &removed_ids);
    assert_eq!(
        shifts,
        vec![ClipShift {
            clip_id: "t".to_string(),
            new_start_frame: 100,
        }]
    );
}

#[test]
fn rpl_003_merge_ranges_merges_overlapping_and_touching_ranges() {
    let merged = merge_ranges(&[
        FrameRange { start: 0, end: 100 },
        FrameRange {
            start: 50,
            end: 200,
        },
        FrameRange {
            start: 200,
            end: 250,
        },
    ]);

    assert_eq!(merged, vec![FrameRange { start: 0, end: 250 }]);
}

#[test]
fn rpl_003_compute_ripple_shifts_for_ranges_merges_before_shifting() {
    let clip = clip("c", 300, 100);
    let shifts = compute_ripple_shifts_for_ranges(
        &[clip],
        &[
            FrameRange { start: 0, end: 100 },
            FrameRange {
                start: 50,
                end: 200,
            },
        ],
    );

    assert_eq!(
        shifts,
        vec![ClipShift {
            clip_id: "c".to_string(),
            new_start_frame: 100,
        }]
    );
}

#[test]
fn rpl_003_ranges_after_clip_do_not_shift_it() {
    let a = clip("a", 100, 50);
    let b = clip("b", 200, 50);
    let shifts = compute_ripple_shifts_for_ranges(
        &[a, b],
        &[
            FrameRange { start: 0, end: 50 },
            FrameRange {
                start: 400,
                end: 500,
            },
        ],
    );

    assert_eq!(
        shifts,
        vec![
            ClipShift {
                clip_id: "a".to_string(),
                new_start_frame: 50,
            },
            ClipShift {
                clip_id: "b".to_string(),
                new_start_frame: 150,
            },
        ]
    );
}

#[test]
fn rpl_003_range_must_end_at_or_before_clip_start_to_shift_it() {
    let clip = clip("c", 100, 50);

    let exactly_at_start = compute_ripple_shifts_for_ranges(
        std::slice::from_ref(&clip),
        &[FrameRange { start: 0, end: 100 }],
    );
    assert_eq!(
        exactly_at_start,
        vec![ClipShift {
            clip_id: "c".to_string(),
            new_start_frame: 0,
        }]
    );

    let overlapping =
        compute_ripple_shifts_for_ranges(&[clip], &[FrameRange { start: 0, end: 101 }]);
    assert!(overlapping.is_empty());
}

#[test]
fn rpl_009_compute_ripple_push_moves_clips_at_or_after_insert_frame() {
    let a = clip("a", 0, 50);
    let b = clip("b", 100, 50);
    let c = clip("c", 200, 50);
    let shifts = compute_ripple_push(&[a, b, c], 100, 30, &BTreeSet::new());

    assert_eq!(
        shifts,
        vec![
            ClipShift {
                clip_id: "b".to_string(),
                new_start_frame: 130,
            },
            ClipShift {
                clip_id: "c".to_string(),
                new_start_frame: 230,
            },
        ]
    );
}

#[test]
fn rpl_009_compute_ripple_push_skips_excluded_ids() {
    let a = clip("a", 100, 50);
    let b = clip("b", 200, 50);
    let exclude = BTreeSet::from(["a".to_string()]);
    let shifts = compute_ripple_push(&[a, b], 0, 25, &exclude);

    assert_eq!(
        shifts,
        vec![ClipShift {
            clip_id: "b".to_string(),
            new_start_frame: 225,
        }]
    );
}

#[test]
fn rpl_007_validate_track_shifts_refuses_negative_starts() {
    let track = track(vec![clip("c1", 10, 20)]);
    let shifts = vec![ClipShift {
        clip_id: "c1".to_string(),
        new_start_frame: -5,
    }];

    assert_eq!(
        validate_track_shifts(&track, &shifts),
        Err(RippleValidationError::NegativeStart {
            clip_id: "c1".to_string(),
        })
    );
}

#[test]
fn rpl_008_validate_track_shifts_refuses_collisions() {
    let track = track(vec![clip("a1", 0, 55), clip("a2", 100, 50)]);
    let shifts = vec![ClipShift {
        clip_id: "a2".to_string(),
        new_start_frame: 50,
    }];

    assert_eq!(
        validate_track_shifts(&track, &shifts),
        Err(RippleValidationError::Collision {
            leading_clip_id: "a1".to_string(),
            trailing_clip_id: "a2".to_string(),
        })
    );
}

#[test]
fn rpl_008_validate_track_shifts_accepts_non_overlapping_layouts() {
    let track = track(vec![clip("a1", 0, 50), clip("a2", 100, 50)]);
    let shifts = vec![ClipShift {
        clip_id: "a2".to_string(),
        new_start_frame: 70,
    }];

    assert_eq!(validate_track_shifts(&track, &shifts), Ok(()));
}
