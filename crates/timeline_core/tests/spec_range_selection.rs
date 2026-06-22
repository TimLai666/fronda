use timeline_core::TimelineRange;

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
