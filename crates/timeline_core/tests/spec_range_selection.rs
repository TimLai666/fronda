use core_model::{Clip, ClipType, Track};
use timeline_core::{
    drag_range_edge, find_all_gaps, find_gap_at_frame, shift_drag_range, RangeEdge, TimelineRange,
};

// ─── RNG-001: normalized() swaps if reversed ───

#[test]
fn rng_001_normalized_returns_self_when_already_ordered() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    let norm = range.normalized();
    assert_eq!(norm.start_frame, 10);
    assert_eq!(norm.end_frame, 20);
}

#[test]
fn rng_001_normalized_swaps_reversed_range() {
    let range = TimelineRange {
        start_frame: 50,
        end_frame: 30,
    };
    let norm = range.normalized();
    assert_eq!(norm.start_frame, 30);
    assert_eq!(norm.end_frame, 50);
}

#[test]
fn rng_001_normalized_equal_frames_stay_equal() {
    let range = TimelineRange {
        start_frame: 25,
        end_frame: 25,
    };
    let norm = range.normalized();
    assert_eq!(norm.start_frame, 25);
    assert_eq!(norm.end_frame, 25);
}

// ─── RNG-002: is_valid requires end > start (half-open) ───

#[test]
fn rng_002_is_valid_positive_range() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    assert!(range.is_valid());
}

#[test]
fn rng_002_is_valid_zero_width_is_invalid() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 10,
    };
    assert!(!range.is_valid());
}

#[test]
fn rng_002_is_valid_negative_width_is_invalid() {
    let range = TimelineRange {
        start_frame: 20,
        end_frame: 10,
    };
    assert!(!range.is_valid());
}

#[test]
fn rng_002_is_valid_single_frame_range() {
    let range = TimelineRange {
        start_frame: 15,
        end_frame: 16,
    };
    assert!(range.is_valid());
}

// ─── RNG-003: contains(frame) half-open [start, end) ───

#[test]
fn rng_003_contains_start_frame() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    assert!(range.contains(10));
}

#[test]
fn rng_003_contains_last_frame() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    assert!(range.contains(19));
}

#[test]
fn rng_003_does_not_contain_end_frame() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    assert!(!range.contains(20));
}

#[test]
fn rng_003_does_not_contain_before_start() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 20,
    };
    assert!(!range.contains(9));
}

#[test]
fn rng_003_zero_width_range_contains_nothing() {
    let range = TimelineRange {
        start_frame: 10,
        end_frame: 10,
    };
    assert!(!range.contains(10));
}

// ─── RNG-004 (shift-drag): Create or edit a timeline range via shift-drag ───

#[test]
fn rng_004_shift_drag_creates_new_range() {
    let range = shift_drag_range(50, 100, None);
    assert_eq!(range.start_frame, 50);
    assert_eq!(range.end_frame, 100);
    assert!(range.is_valid());
}

#[test]
fn rng_004_shift_drag_reversed_order() {
    let range = shift_drag_range(100, 50, None);
    assert_eq!(range.start_frame, 50);
    assert_eq!(range.end_frame, 100);
}

#[test]
fn rng_004_shift_drag_extends_existing_start() {
    let existing = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let range = shift_drag_range(50, 30, Some(existing));
    assert_eq!(range.start_frame, 30);
    assert_eq!(range.end_frame, 100);
}

#[test]
fn rng_004_shift_drag_extends_existing_end() {
    let existing = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let range = shift_drag_range(100, 150, Some(existing));
    assert_eq!(range.start_frame, 50);
    assert_eq!(range.end_frame, 150);
}

// ─── RNG-005: Drag existing range edges ───

#[test]
fn rng_005_drag_start_edge() {
    let range = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let result = drag_range_edge(range, RangeEdge::Start, 30);
    assert_eq!(result.start_frame, 30);
    assert_eq!(result.end_frame, 100);
}

#[test]
fn rng_005_drag_end_edge() {
    let range = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let result = drag_range_edge(range, RangeEdge::End, 150);
    assert_eq!(result.start_frame, 50);
    assert_eq!(result.end_frame, 150);
}

#[test]
fn rng_005_drag_start_past_end_clamps() {
    let range = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let result = drag_range_edge(range, RangeEdge::Start, 200);
    assert!(result.start_frame < result.end_frame);
    assert_eq!(result.end_frame, 100);
    assert_eq!(result.start_frame, 99);
}

#[test]
fn rng_005_drag_end_before_start_clamps() {
    let range = TimelineRange {
        start_frame: 50,
        end_frame: 100,
    };
    let result = drag_range_edge(range, RangeEdge::End, 10);
    assert!(result.is_valid());
    assert_eq!(result.start_frame, 50);
    assert_eq!(result.end_frame, 51);
}

// ─── RNG-006: Gap selection ───

fn single_track(clips: Vec<Clip>) -> Track {
    Track {
        id: "t1".to_string(),
        r#type: ClipType::Video,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

fn gap_clip(id: &str, start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: id.to_string(),
        media_ref: String::new(),
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
        fade_in_interpolation: core_model::Interpolation::Linear,
        fade_out_interpolation: core_model::Interpolation::Linear,
        opacity: 1.0,
        transform: core_model::Transform::default(),
        crop: core_model::Crop::default(),
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

#[test]
fn rng_006_find_gap_between_clips() {
    let track = single_track(vec![gap_clip("c1", 0, 50), gap_clip("c2", 100, 50)]);
    let gap = find_gap_at_frame(&track, 75);
    assert!(gap.is_some());
    assert_eq!(gap.unwrap().start_frame, 50);
    assert_eq!(gap.unwrap().end_frame, 100);
}

#[test]
fn rng_006_find_gap_on_clip_returns_none() {
    let track = single_track(vec![gap_clip("c1", 0, 100)]);
    assert!(find_gap_at_frame(&track, 50).is_none());
}

#[test]
fn rng_006_find_all_gaps() {
    let track = single_track(vec![
        gap_clip("c1", 0, 30),
        gap_clip("c2", 50, 20),
        gap_clip("c3", 100, 40),
    ]);
    let gaps = find_all_gaps(&track);
    assert_eq!(gaps.len(), 2);
    assert_eq!(gaps[0].start_frame, 30);
    assert_eq!(gaps[0].end_frame, 50);
    assert_eq!(gaps[1].start_frame, 70);
    assert_eq!(gaps[1].end_frame, 100);
}

#[test]
fn rng_006_no_gaps_when_clips_adjacent() {
    let track = single_track(vec![gap_clip("c1", 0, 50), gap_clip("c2", 50, 50)]);
    let gaps = find_all_gaps(&track);
    assert!(gaps.is_empty());
}

#[test]
fn rng_006_empty_track_returns_no_gaps() {
    let track = single_track(vec![]);
    assert!(find_gap_at_frame(&track, 0).is_none());
    assert!(find_all_gaps(&track).is_empty());
}
