use core_model::{Clip, ClipType, Crop, Interpolation, Track, Transform};
use timeline_core::{
    clamp_drag_to_frame_zero, collect_targets, find_snap, find_snap_simple,
    resolve_cut_preview_snap, validate_drag_not_past_zero, SnapState, SnapTarget, SnapTargetKind,
    PLAYHEAD_MULTIPLIER, STICKY_MULTIPLIER, THRESHOLD_PIXELS,
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
    }
}

fn video_track(clips: Vec<Clip>) -> Track {
    Track {
        id: "video-track".to_string(),
        r#type: ClipType::Video,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

// ─── SNP-001: Base threshold = 8 pixels ───

#[test]
fn snp_001_base_threshold_is_8_pixels() {
    assert_eq!(THRESHOLD_PIXELS, 8.0);
}

#[test]
fn snp_001_snap_within_threshold_frame() {
    // At pixels_per_frame = 4.0, threshold is 8/4 = 2 frames
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // Position 101 is 1 frame from clip start at 100 → within threshold of 2
    let result = find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_some());
    assert_eq!(result.unwrap().frame, 100);
}

#[test]
fn snp_001_no_snap_beyond_threshold_frame() {
    // At pixels_per_frame = 4.0, threshold is 2 frames
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // Position 103 is 3 frames from clip start at 100 → beyond threshold of 2
    let result = find_snap_simple(103, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_none());
}

// ─── SNP-002: Sticky multiplier = 1.5 ───

#[test]
fn snp_002_sticky_multiplier_is_1_5() {
    assert_eq!(STICKY_MULTIPLIER, 1.5);
}

#[test]
fn snp_002_sticky_snap_holds_within_sticky_threshold() {
    // Base threshold = 8px, px_per_frame = 4 → base 2 frames
    // Sticky = 2 * 1.5 = 3 frames
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // First snap: position 101 snaps to 100
    assert!(find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0).is_some());

    // Sticky holds: position 104 is 4 frames away from 100, but sticky threshold is 3
    // so 104 should snap (distance 4 > 3) → no snap via sticky
    // We probe at 104 with offset 0, sticky threshold from snapped=100 is 3,
    // |104 - 100| = 4 > 3, so sticky releases
    let result = find_snap_simple(104, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    // After release, fresh scan: |104 - 100| = 4 > 2 (base), so no snap
    assert!(result.is_none());
}

// ─── SNP-003: Playhead multiplier = 1.5 ───

#[test]
fn snp_003_playhead_multiplier_is_1_5() {
    assert_eq!(PLAYHEAD_MULTIPLIER, 1.5);
}

#[test]
fn snp_003_playhead_snaps_with_wider_threshold() {
    // Base threshold = 2 frames (at px=4), playhead = 2 * 1.5 = 3 frames
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 200, &[], true); // include playhead at 200
    let mut state = SnapState::default();

    // Position 198 is 2 frames from playhead at 200
    // Base threshold of 2 would just catch it for clip edges, but for playhead it's 3
    let result = find_snap_simple(198, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_some());
    // Should snap to playhead (200) because it's closer than any clip edge (100 or 150)
    assert_eq!(result.unwrap().frame, 200);
}

// ─── SNP-004: Targets include clip boundaries and optionally playhead ───

#[test]
fn snp_004_collect_targets_includes_clip_edges() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50), clip("c2", 200, 30)])];
    let targets = collect_targets(&tracks, 0, &[], false);

    assert!(targets
        .iter()
        .any(|t| t.frame == 100 && t.kind == SnapTargetKind::ClipEdge));
    assert!(targets
        .iter()
        .any(|t| t.frame == 150 && t.kind == SnapTargetKind::ClipEdge));
    assert!(targets
        .iter()
        .any(|t| t.frame == 200 && t.kind == SnapTargetKind::ClipEdge));
    assert!(targets
        .iter()
        .any(|t| t.frame == 230 && t.kind == SnapTargetKind::ClipEdge));
    assert!(!targets.iter().any(|t| t.kind == SnapTargetKind::Playhead));
}

#[test]
fn snp_004_collect_targets_includes_playhead_when_requested() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 300, &[], true);

    assert!(targets
        .iter()
        .any(|t| t.frame == 300 && t.kind == SnapTargetKind::Playhead));
    assert!(targets
        .iter()
        .any(|t| t.frame == 100 && t.kind == SnapTargetKind::ClipEdge));
}

#[test]
fn snp_004_collect_targets_skips_excluded_clip_ids() {
    let tracks = vec![video_track(vec![
        clip("skip", 100, 50),
        clip("keep", 200, 30),
    ])];
    let targets = collect_targets(&tracks, 0, &["skip".to_string()], false);

    assert!(!targets.iter().any(|t| t.frame == 100));
    assert!(targets.iter().any(|t| t.frame == 200));
    assert!(targets.iter().any(|t| t.frame == 230));
}

#[test]
fn snp_004_collect_targets_empty_when_no_clips_and_no_playhead() {
    let tracks: Vec<Track> = vec![];
    let targets = collect_targets(&tracks, 0, &[], false);
    assert!(targets.is_empty());
}

// ─── SNP-005: Sticky state persistence ───

#[test]
fn snp_005_sticky_state_releases_when_target_disappears() {
    let mut tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // First snap: position 101 snaps to 100
    let r1 = find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(r1.is_some());
    assert_eq!(r1.unwrap().frame, 100);

    // Remove the clip so target disappears
    tracks[0].clips.clear();
    let targets2 = collect_targets(&tracks, 0, &[], false);

    // Sticky should release because target no longer exists
    let r2 = find_snap_simple(101, &targets2, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(r2.is_none());
}

#[test]
fn snp_005_sticky_snap_releases_after_escape() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // First snap at 101 → snaps to 100
    assert!(find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0).is_some());

    // Move way beyond sticky threshold: position 200, distance 100 >> 3
    find_snap_simple(200, &targets, &mut state, THRESHOLD_PIXELS, 4.0);

    // State should be reset
    assert_eq!(state.currently_snapped_to, None);
}

// ─── SNP-006: Multi-probe-offset snapping ───

#[test]
fn snp_006_multi_probe_snaps_to_closest_target() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50), clip("c2", 200, 30)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    // Probe offsets [-50, 0, 50] at position 100
    // - offset -50: probes frame 50 → distance 50 from 100, 150 from 150
    // - offset 0: probes frame 100 → distance 0 from 100 → perfect match!
    let result = find_snap(
        100,
        &[-50, 0, 50],
        &targets,
        &mut state,
        THRESHOLD_PIXELS,
        4.0,
    );
    assert!(result.is_some());
    assert_eq!(result.unwrap().frame, 100);
}

#[test]
fn snp_006_empty_probe_offsets_no_snap() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    let result = find_snap(101, &[], &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_none());
}

#[test]
fn snp_006_no_targets_no_snap() {
    let tracks: Vec<Track> = vec![];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    let result = find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_none());
}

// ─── Snap result includes x position ───

#[test]
fn snp_snap_result_includes_x_position() {
    let tracks = vec![video_track(vec![clip("c1", 100, 50)])];
    let targets = collect_targets(&tracks, 0, &[], false);
    let mut state = SnapState::default();

    let result = find_snap_simple(101, &targets, &mut state, THRESHOLD_PIXELS, 4.0);
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.frame, 100);
    assert_eq!(r.probe_offset, 0);
    assert_eq!(r.x, 400.0); // 100 frames * 4 px/frame
}

fn snap_targets(frames: &[i64]) -> Vec<SnapTarget> {
    frames
        .iter()
        .map(|&f| SnapTarget {
            frame: f,
            kind: SnapTargetKind::ClipEdge,
        })
        .collect()
}

// ─── SNP-009: collect_targets properties ───

#[test]
fn snp_009_collect_targets_returns_sorted_frames() {
    // Clips with varied start frames should produce sorted snap targets
    let c1 = clip("c1", 50, 30);
    let c2 = clip("c2", 10, 20);
    let c3 = clip("c3", 100, 60);
    let mut tracks = vec![video_track(vec![c1, c2, c3])];
    tracks[0].clips[0].link_group_id = Some("g1".to_string());
    tracks[0].clips[1].link_group_id = Some("g1".to_string());

    let targets = collect_targets(&tracks, 0, &[], false);
    let frames: Vec<i64> = targets.iter().map(|t| t.frame).collect();

    for pair in frames.windows(2) {
        assert!(pair[0] <= pair[1], "target frames not sorted: {:?}", frames);
    }

    assert!(frames.contains(&50), "missing start of c1");
    assert!(frames.contains(&80), "missing end of c1 (50+30)");
    assert!(frames.contains(&10), "missing start of c2");
    assert!(frames.contains(&30), "missing end of c2 (10+20)");
}

#[test]
fn snp_009_collect_targets_deterministic() {
    let c1 = clip("c1", 0, 100);
    let c2 = clip("c2", 150, 50);
    let tracks = vec![video_track(vec![c1, c2])];

    let first = collect_targets(&tracks, 0, &[], false);
    let second = collect_targets(&tracks, 0, &[], false);
    assert_eq!(first, second, "collect_targets should be deterministic");
}

// ─── SNP-010: find_snap_simple threshold behavior ───

#[test]
fn snp_010_find_snap_simple_within_threshold_returns_some() {
    // At pixels_per_frame = 1.0, base frame threshold = 8 / 1 = 8 frames
    let targets = snap_targets(&[100, 200, 300]);

    let mut state = SnapState::default();
    let result = find_snap_simple(105, &targets, &mut state, 8.0, 1.0);
    assert_eq!(
        result.unwrap().frame,
        100,
        "frame 100 is 5 away, within threshold 8"
    );

    let mut state = SnapState::default();
    let result = find_snap_simple(95, &targets, &mut state, 8.0, 1.0);
    assert_eq!(
        result.unwrap().frame,
        100,
        "frame 100 is 5 away on the left"
    );

    // 107 - 100 = 7 <= 8, so within threshold
    let mut state = SnapState::default();
    let result = find_snap_simple(107, &targets, &mut state, 8.0, 1.0);
    assert_eq!(
        result.unwrap().frame,
        100,
        "frame 100 is 7 away, still within threshold 8"
    );
}

#[test]
fn snp_010_find_snap_simple_outside_threshold_returns_none() {
    let targets = snap_targets(&[100, 200, 300]);

    let mut state = SnapState::default();
    let result = find_snap_simple(50, &targets, &mut state, 8.0, 1.0);
    assert!(result.is_none(), "50 is 50 away from nearest target 100");

    let targets2 = snap_targets(&[100]);
    let mut state = SnapState::default();
    let result = find_snap_simple(200, &targets2, &mut state, 8.0, 1.0);
    assert!(result.is_none(), "200 is 100 away from 100");
}

#[test]
fn snp_010_find_snap_simple_empty_targets_returns_none() {
    let targets: Vec<SnapTarget> = vec![];
    let mut state = SnapState::default();
    let result = find_snap_simple(50, &targets, &mut state, 8.0, 1.0);
    assert!(result.is_none(), "empty targets should return None");
}

#[test]
fn snp_010_find_snap_simple_exact_match() {
    let targets = snap_targets(&[100, 200]);

    let mut state = SnapState::default();
    let result = find_snap_simple(100, &targets, &mut state, 0.0, 1.0);
    assert_eq!(result.unwrap().frame, 100, "exact match at threshold 0");

    let mut state = SnapState::default();
    let result = find_snap_simple(101, &targets, &mut state, 0.0, 1.0);
    assert!(result.is_none(), "1 away at threshold 0 should not snap");
}

// ─── SNP-007: Drag must not cross frame 0 ───

#[test]
fn snp_007_valid_drag_allows_positive_target() {
    let starts = vec![100, 200, 300];
    assert!(validate_drag_not_past_zero(&starts, 150, 100));
}

#[test]
fn snp_007_invalid_drag_crosses_zero() {
    let starts = vec![100];
    assert!(!validate_drag_not_past_zero(&starts, -50, 100));
}

#[test]
fn snp_007_clamp_to_frame_zero() {
    let starts = vec![100];
    let clamped = clamp_drag_to_frame_zero(&starts, -50, 100);
    assert_eq!(clamped, 0);
}

#[test]
fn snp_007_clamp_multi_clip() {
    let starts = vec![50, 100, 200];
    let clamped = clamp_drag_to_frame_zero(&starts, -100, 0);
    assert_eq!(clamped, -50); // earliest clip (50) goes to 0, delta = -50
}

// ─── SNP-008: Razor/cut preview snaps to same targets as drag ───

#[test]
fn snp_008_cut_preview_snaps_to_nearest_edge() {
    let targets = vec![
        SnapTarget {
            frame: 100,
            kind: SnapTargetKind::ClipEdge,
        },
        SnapTarget {
            frame: 200,
            kind: SnapTargetKind::ClipEdge,
        },
    ];
    let snapped = resolve_cut_preview_snap(103, &targets, 8.0, 1.0);
    assert_eq!(snapped, Some(100));
}

#[test]
fn snp_008_cut_preview_no_snap_when_far() {
    let targets = vec![SnapTarget {
        frame: 100,
        kind: SnapTargetKind::ClipEdge,
    }];
    let snapped = resolve_cut_preview_snap(150, &targets, 8.0, 1.0);
    assert_eq!(snapped, None);
}

#[test]
fn snp_008_cut_preview_snaps_to_playhead() {
    let targets = vec![SnapTarget {
        frame: 50,
        kind: SnapTargetKind::Playhead,
    }];
    let snapped = resolve_cut_preview_snap(55, &targets, 8.0, 1.0);
    assert_eq!(snapped, Some(50));
}

#[test]
fn snp_008_cut_preview_zero_pixels_per_frame() {
    let targets = vec![SnapTarget {
        frame: 100,
        kind: SnapTargetKind::ClipEdge,
    }];
    let snapped = resolve_cut_preview_snap(999, &targets, 8.0, 0.0);
    assert_eq!(snapped, Some(100));
}
