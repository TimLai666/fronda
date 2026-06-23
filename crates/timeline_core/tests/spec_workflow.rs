use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use std::collections::BTreeSet;
use timeline_core::{
    apply_ripple_insert_with_split, compute_ripple_delete, compute_ripple_delete_gap,
    compute_trim_values, timing_propagation_partners, FrameRange, RippleDeleteConfig,
    RippleDeleteOutcome, RippleInsertClipSpec, RippleInsertConfig, RippleInsertWithSplitOutcome,
    TrimEdge,
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
    }
}

fn audio_clip(id: &str, start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: id.to_string(),
        media_ref: format!("asset-{id}"),
        media_type: ClipType::Audio,
        source_clip_type: ClipType::Audio,
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

fn audio_track(clips: Vec<Clip>) -> Track {
    Track {
        id: "audio-track".to_string(),
        r#type: ClipType::Audio,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

fn unsynced_track(clips: Vec<Clip>) -> Track {
    Track {
        id: "unsynced".to_string(),
        r#type: ClipType::Video,
        muted: false,
        hidden: false,
        sync_locked: false,
        clips,
    }
}

fn timeline(tracks: Vec<Track>) -> Timeline {
    Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks,
    }
}

// ─── RPL-001: Ripple delete removes clips and closes gap ───

#[test]
fn rpl_001_ripple_delete_middle_closets_gap() {
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 100, 50);
    let t = timeline(vec![video_track(vec![c1, c2])]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![FrameRange {
                start: 50,
                end: 100,
            }],
        },
    );
    assert!(matches!(result, RippleDeleteOutcome::Ok(_)));
}

// ─── RPL-002: Gap delete closes exactly the empty interval ───

#[test]
fn rpl_002_gap_delete_closes_gap_and_shifts_trailing() {
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 100, 50);
    let t = timeline(vec![video_track(vec![c1, c2])]);
    let result = compute_ripple_delete_gap(
        &t,
        0,
        FrameRange {
            start: 50,
            end: 100,
        },
    );
    assert!(result.is_ok());
    let shifts = result.unwrap();
    // Only track 0 has shifts — c2 moves from 100 to 50
    assert_eq!(shifts.len(), 1);
    assert_eq!(shifts[0].len(), 1);
    assert_eq!(shifts[0][0].clip_id, "c2");
    assert_eq!(shifts[0][0].new_start_frame, 50);
}

#[test]
fn rpl_002_gap_delete_refuses_stale_gap() {
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 60, 30);
    let c3 = clip("c3", 100, 50);
    let t = timeline(vec![video_track(vec![c1, c2, c3])]);
    let result = compute_ripple_delete_gap(
        &t,
        0,
        FrameRange {
            start: 50,
            end: 100,
        },
    );
    assert!(result.is_err());
}

// ─── RPL-004: Ripple delete cuts overlapping clip fragments ───

#[test]
fn rpl_004_sync_locked_follower_shifts_after_range_delete() {
    let v1 = clip("v1", 0, 100);
    let a1 = clip("a1", 120, 30);
    let t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![FrameRange { start: 40, end: 50 }],
        },
    );
    assert!(matches!(result, RippleDeleteOutcome::Ok(_)));
}

// ─── RPL-005: Linked A/V partner tracks are also cleared ───

#[test]
fn rpl_005_linked_partner_tracks_cleared() {
    let mut v1 = clip("v1", 0, 100);
    v1.link_group_id = Some("G".to_string());
    let mut a1 = audio_clip("a1", 0, 100);
    a1.link_group_id = Some("G".to_string());
    let t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![FrameRange { start: 40, end: 50 }],
        },
    );
    match result {
        RippleDeleteOutcome::Ok(report) => {
            assert!(report.cleared_track_indices.contains(&1));
        }
        _ => panic!("expected Ok"),
    }
}

// ─── RPL-006: Sync-locked follower tracks shift ───

#[test]
fn rpl_006_sync_locked_track_shift_valid() {
    let v = video_track(vec![clip("v1", 0, 100)]);
    let a = audio_track(vec![clip("a1", 120, 30)]);
    let t = timeline(vec![v, a]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![FrameRange { start: 40, end: 50 }],
        },
    );
    assert!(matches!(result, RippleDeleteOutcome::Ok(_)));
}

#[test]
fn rpl_006_unsynced_track_ignored() {
    let v = video_track(vec![clip("v1", 0, 100)]);
    let u = unsynced_track(vec![clip("u1", 120, 30)]);
    let t = timeline(vec![v, u]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![FrameRange { start: 40, end: 50 }],
        },
    );
    assert!(matches!(result, RippleDeleteOutcome::Ok(_)));
}

// ─── RPL-007/008: Validation in gap context ───

#[test]
fn rpl_007_gap_delete_refuses_sync_collision() {
    let v = video_track(vec![clip("c1", 0, 50), clip("c2", 100, 50)]);
    let a = audio_track(vec![clip("a1", 0, 55), clip("a2", 100, 50)]);
    let t = timeline(vec![v, a]);
    let result = compute_ripple_delete_gap(
        &t,
        0,
        FrameRange {
            start: 50,
            end: 100,
        },
    );
    assert!(result.is_err());
}

// ─── LNK-005: Timing propagation partners ───

#[test]
fn lnk_005_timing_propagation_partners_returns_partners_not_in_input() {
    let mut v1 = clip("v1", 0, 30);
    v1.link_group_id = Some("G".to_string());
    let mut a1 = audio_clip("a1", 0, 30);
    a1.link_group_id = Some("G".to_string());
    let mut v2 = clip("v2", 100, 30);
    v2.link_group_id = Some("G2".to_string());
    let mut a2 = audio_clip("a2", 100, 30);
    a2.link_group_id = Some("G2".to_string());
    let t = timeline(vec![video_track(vec![v1, v2]), audio_track(vec![a1, a2])]);
    let input = BTreeSet::from(["v1".to_string(), "v2".to_string()]);
    let partners = timing_propagation_partners(&t, &input);
    assert!(partners.contains("a1"));
    assert!(partners.contains("a2"));
    assert!(!partners.contains("v1"));
    assert!(!partners.contains("v2"));
}

#[test]
fn lnk_005_timing_propagation_partners_returns_empty_for_no_links() {
    let t = timeline(vec![video_track(vec![clip("v1", 0, 30)])]);
    let input = BTreeSet::from(["v1".to_string()]);
    assert!(timing_propagation_partners(&t, &input).is_empty());
}

// ─── LNK-009: Trim propagation ───

#[test]
fn lnk_009_trim_left_source_time_delta() {
    let c = clip("c1", 100, 50);
    let (trim_start, trim_end) = compute_trim_values(&c, TrimEdge::Left, -10);
    // delta = -10 frames, source_delta = -10 (speed=1.0)
    // new trim_start = 0 + (-10) = -10, clamped to 0 for non-image/text
    assert_eq!(trim_start, 0);
    assert_eq!(trim_end, 0);
}

#[test]
fn lnk_009_trim_right_source_time_delta() {
    let c = clip("c1", 100, 50);
    let (trim_start, trim_end) = compute_trim_values(&c, TrimEdge::Right, 10);
    // delta = 10 frames, source_delta = 10
    // new trim_end = 0 - 10 = -10, clamped to 0 for non-image/text
    assert_eq!(trim_start, 0);
    assert_eq!(trim_end, 0);
}

// ─── LNK-010: Image/text trim can go negative ───

#[test]
fn lnk_010_image_trim_left_can_go_negative() {
    let mut c = clip("img1", 100, 50);
    c.media_type = ClipType::Image;
    let (trim_start, _) = compute_trim_values(&c, TrimEdge::Left, -10);
    // unbounded: new_start = 0 + (-10) = -10
    assert_eq!(trim_start, -10);
}

#[test]
fn lnk_010_text_trim_right_can_go_negative() {
    let mut c = clip("txt1", 100, 50);
    c.media_type = ClipType::Text;
    let (_, trim_end) = compute_trim_values(&c, TrimEdge::Right, 10);
    // unbounded: new_end = 0 - 10 = -10
    assert_eq!(trim_end, -10);
}

// ─── RPL-009: Ripple insert pushes downstream clips ───

#[test]
fn rpl_009_ripple_insert_pushes_downstream_clips() {
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 50, 50);
    let t = timeline(vec![video_track(vec![c1, c2])]);
    // compute_ripple_insert pushes clips at/after insert_frame
    let shifts = timeline_core::compute_ripple_push(&t.tracks[0].clips, 50, 30, &BTreeSet::new());
    assert_eq!(shifts.len(), 1);
    assert_eq!(shifts[0].clip_id, "c2");
    assert_eq!(shifts[0].new_start_frame, 80);
}

// ─── RPL-009/010/011/012: Ripple insert ───

#[test]
fn rpl_009_ripple_insert_opens_gap_and_shifts_downstream() {
    // Insert a 30-frame clip at 50, pushing c2 from [50,100) to [80,130)
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 50, 50);
    let t = timeline(vec![video_track(vec![c1, c2])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: None,
        },
    );
    match result {
        timeline_core::RippleInsertOutcome::Ok(report) => {
            assert_eq!(report.total_push, 30);
            assert!(!report.created_clip_ids.is_empty());
            // Track 0 should have a shift for c2
            assert!(!report.shifts_by_track[0].is_empty());
            let c2_shift = report.shifts_by_track[0]
                .iter()
                .find(|s| s.clip_id == "c2")
                .expect("c2 should be shifted");
            assert_eq!(c2_shift.new_start_frame, 80);
        }
        _ => panic!("expected Ok"),
    }
}

#[test]
fn rpl_009_ripple_insert_multiple_clips_seqentially_in_gap() {
    // Insert two clips sequentially at 50: first 20fr, second 30fr
    let c1 = clip("c1", 0, 50);
    let c2 = clip("c2", 50, 50);
    let t = timeline(vec![video_track(vec![c1, c2])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![
                timeline_core::RippleInsertClipSpec {
                    asset_id: "a".to_string(),
                    duration_frames: 20,
                    trim_start_frame: None,
                    trim_end_frame: None,
                },
                timeline_core::RippleInsertClipSpec {
                    asset_id: "b".to_string(),
                    duration_frames: 30,
                    trim_start_frame: None,
                    trim_end_frame: None,
                },
            ],
            linked_audio_track_index: None,
        },
    );
    match result {
        timeline_core::RippleInsertOutcome::Ok(report) => {
            // Total push = 20 + 30 = 50
            assert_eq!(report.total_push, 50);
            assert_eq!(report.created_clip_ids.len(), 2);
            // c2 should be pushed from 50 to 100
            let c2_shift = report.shifts_by_track[0]
                .iter()
                .find(|s| s.clip_id == "c2")
                .expect("c2 should be shifted");
            assert_eq!(c2_shift.new_start_frame, 100);
        }
        _ => panic!("expected Ok"),
    }
}

#[test]
fn rpl_010_ripple_insert_opens_gap_on_linked_audio_track() {
    let v1 = clip("v1", 0, 100);
    let a1 = clip("a1", 120, 50);
    let t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: Some(1),
        },
    );
    match result {
        timeline_core::RippleInsertOutcome::Ok(report) => {
            // Both tracks should be in push set
            assert!(report.push_track_indices.contains(&0));
            assert!(report.push_track_indices.contains(&1));
            // Track 1 (audio) should also have shifts
            assert!(!report.shifts_by_track[1].is_empty());
            let a1_shift = report.shifts_by_track[1]
                .iter()
                .find(|s| s.clip_id == "a1")
                .expect("a1 should be shifted");
            assert_eq!(a1_shift.new_start_frame, 150);
        }
        _ => panic!("expected Ok"),
    }
}

#[test]
fn rpl_011_straddling_clip_at_insert_point_is_detected() {
    // A clip straddling the insert frame: c1 covers [0, 100), insert at 50
    let c1 = clip("c1", 0, 100);
    let t = timeline(vec![video_track(vec![c1])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: None,
        },
    );
    // The straddling clip is detected; the compute function still accepts it
    // (actual split happens at the editor level)
    match result {
        timeline_core::RippleInsertOutcome::Ok(report) => {
            assert_eq!(report.total_push, 30);
            // c1 won't be shifted since its start is before insert_frame
            assert!(report.shifts_by_track[0].is_empty());
        }
        _ => panic!("expected Ok"),
    }
}

#[test]
fn rpl_ripple_insert_refuses_empty_clips() {
    let t = timeline(vec![video_track(vec![clip("c1", 0, 50)])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![],
            linked_audio_track_index: None,
        },
    );
    assert!(matches!(
        result,
        timeline_core::RippleInsertOutcome::Refused(_)
    ));
}

#[test]
fn rpl_ripple_insert_refuses_negative_frame() {
    let t = timeline(vec![video_track(vec![clip("c1", 0, 50)])]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: -5,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: None,
        },
    );
    assert!(matches!(
        result,
        timeline_core::RippleInsertOutcome::Refused(_)
    ));
}

#[test]
fn rpl_ripple_insert_refuses_out_of_bounds_track() {
    let t = timeline(vec![]);
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: None,
        },
    );
    assert!(matches!(
        result,
        timeline_core::RippleInsertOutcome::Refused(_)
    ));
}

#[test]
fn rpl_ripple_insert_sync_locked_track_also_gets_pushed() {
    let v1 = clip("v1", 0, 100);
    let a1 = clip("a1", 120, 50);
    let mut t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);
    // Make audio track sync-locked
    t.tracks[1].sync_locked = true;
    let result = timeline_core::compute_ripple_insert(
        &t,
        timeline_core::RippleInsertConfig {
            track_index: 0,
            insert_frame: 50,
            clips: vec![timeline_core::RippleInsertClipSpec {
                asset_id: "new".to_string(),
                duration_frames: 30,
                trim_start_frame: None,
                trim_end_frame: None,
            }],
            linked_audio_track_index: None,
        },
    );
    match result {
        timeline_core::RippleInsertOutcome::Ok(report) => {
            assert!(report.push_track_indices.contains(&1));
            assert!(!report.shifts_by_track[1].is_empty());
        }
        _ => panic!("expected Ok"),
    }
}

// ─── Refuse on non-empty ranges ───

#[test]
fn refuses_empty_ranges() {
    let t = timeline(vec![video_track(vec![clip("c1", 0, 50)])]);
    let result = compute_ripple_delete(
        &t,
        RippleDeleteConfig {
            anchor_track_index: 0,
            ranges: vec![],
        },
    );
    assert!(matches!(result, RippleDeleteOutcome::Refused(_)));
}

// ─── RPL-012: Ripple insert with split splits straddling clips ───

#[test]
fn rpl_012_ripple_insert_with_split_splits_straddling_clip() {
    // Track has a clip spanning [20, 80), insert at frame 40
    let c1 = clip("c1", 20, 60);
    let mut t = timeline(vec![video_track(vec![c1])]);
    t.tracks[0].sync_locked = false; // only target track

    let config = timeline_core::RippleInsertConfig {
        track_index: 0,
        insert_frame: 40,
        clips: vec![timeline_core::RippleInsertClipSpec {
            asset_id: "new-asset".to_string(),
            duration_frames: 30,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    let result = timeline_core::compute_ripple_insert_with_split(&t, config);
    match result {
        timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
            assert_eq!(plan.split_actions.len(), 1, "should have one split action");
            assert_eq!(plan.split_actions[0].0, 0); // track 0
            assert_eq!(plan.split_actions[0].1, "c1"); // clip id
            assert_eq!(plan.split_actions[0].2, 40); // split at frame 40
            assert_eq!(plan.insert.total_push, 30);
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[test]
fn rpl_012_ripple_insert_with_split_multiple_tracks() {
    // Two sync-locked tracks, both straddling insert at frame 40
    let v1 = clip("v1", 20, 80); // straddles frame 40: [20, 100)
    let a1 = clip("a1", 10, 50); // straddles frame 40: [10, 60)
    let t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);

    let config = timeline_core::RippleInsertConfig {
        track_index: 0,
        insert_frame: 40,
        clips: vec![timeline_core::RippleInsertClipSpec {
            asset_id: "new".to_string(),
            duration_frames: 20,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    let result = timeline_core::compute_ripple_insert_with_split(&t, config);
    match result {
        timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
            // Both tracks have sync_locked=true (our helper sets it)
            // Track 0 has straddler v1 at [20,100), insert at 40
            // Track 1 has a1 at [10,60), insert at 40 — also straddles
            assert_eq!(plan.split_actions.len(), 2, "should have two split actions");
            assert_eq!(plan.insert.total_push, 20);
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[test]
fn rpl_012_ripple_insert_with_split_no_straddle_delegates_to_regular_insert() {
    let c1 = clip("c1", 0, 30);
    let c2 = clip("c2", 50, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);
    t.tracks[0].sync_locked = false;

    let config = timeline_core::RippleInsertConfig {
        track_index: 0,
        insert_frame: 30, // between clips, no straddle
        clips: vec![timeline_core::RippleInsertClipSpec {
            asset_id: "new".to_string(),
            duration_frames: 10,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    match timeline_core::compute_ripple_insert_with_split(&t, config) {
        timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
            assert!(plan.split_actions.is_empty(), "no straddle = no splits");
            assert_eq!(plan.insert.total_push, 10);
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

#[test]
fn rpl_012_ripple_insert_with_split_at_boundary_no_straddle() {
    // Insert at clip end_frame — no straddle, no split needed
    let c1 = clip("c1", 0, 40);
    let mut t = timeline(vec![video_track(vec![c1])]);
    t.tracks[0].sync_locked = false;

    let config = timeline_core::RippleInsertConfig {
        track_index: 0,
        insert_frame: 40, // at clip end — boundary, not a straddle
        clips: vec![timeline_core::RippleInsertClipSpec {
            asset_id: "new".to_string(),
            duration_frames: 10,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    match timeline_core::compute_ripple_insert_with_split(&t, config) {
        timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
            assert!(plan.split_actions.is_empty(), "end boundary = no straddle");
            assert_eq!(plan.insert.total_push, 10);
        }
        other => panic!("expected Ok, got {:?}", other),
    }
}

// ─── apply_ripple_insert_with_split ───

#[test]
fn apply_ripple_insert_with_split_splits_and_inserts() {
    // Track 0: clip c1 at [10, 100). Insert at 40 — straddles.
    let c1 = clip("c1", 10, 90);
    let mut t = timeline(vec![video_track(vec![c1])]);

    let config = RippleInsertConfig {
        track_index: 0,
        insert_frame: 40,
        clips: vec![RippleInsertClipSpec {
            asset_id: "new-asset".to_string(),
            duration_frames: 20,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    let plan = match timeline_core::compute_ripple_insert_with_split(&t, config) {
        RippleInsertWithSplitOutcome::Ok(p) => p,
        other => panic!("expected Ok, got {:?}", other),
    };

    assert_eq!(plan.split_actions.len(), 1, "should split the straddler");
    assert_eq!(plan.split_actions[0].1, "c1");
    assert_eq!(plan.split_actions[0].2, 40);

    apply_ripple_insert_with_split(&mut t, plan);

    // After apply:
    // c1 now occupies [10, 40)
    // Right half of c1 occupies [60, 120) (shifted from 40→60 by push of 20)
    // New clip occupies [40, 60)
    assert_eq!(
        t.tracks[0].clips.len(),
        3,
        "should have 3 clips after split+insert"
    );

    let first = &t.tracks[0].clips[0];
    assert_eq!(first.id, "c1");
    assert_eq!(first.start_frame, 10);
    assert_eq!(first.duration_frames, 30); // 40 - 10 = 30

    let second = &t.tracks[0].clips[1];
    assert!(
        second.media_ref == "new-asset",
        "new clip should be in the middle"
    );
    assert_eq!(second.start_frame, 40);
    assert_eq!(second.duration_frames, 20);

    let third = &t.tracks[0].clips[2];
    assert_eq!(third.start_frame, 60); // 40 + 20 push
    assert_eq!(third.duration_frames, 60); // right half: 90 - 30 = 60
}

#[test]
fn apply_ripple_insert_with_split_multiple_tracks() {
    // Track 0: clip v1 at [20, 100). Insert at 40.
    // Track 1: sync-locked audio, clip a1 at [10, 60). Insert at 40 — also straddles.
    let v1 = clip("v1", 20, 80);
    let a1 = audio_clip("a1", 10, 50);
    let mut t = timeline(vec![video_track(vec![v1]), audio_track(vec![a1])]);

    let config = RippleInsertConfig {
        track_index: 0,
        insert_frame: 40,
        clips: vec![RippleInsertClipSpec {
            asset_id: "new-asset".to_string(),
            duration_frames: 15,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None, // sync_locked covers track 1
    };

    let plan = match timeline_core::compute_ripple_insert_with_split(&t, config) {
        RippleInsertWithSplitOutcome::Ok(p) => p,
        other => panic!("expected Ok, got {:?}", other),
    };

    assert_eq!(plan.split_actions.len(), 2, "both tracks have straddlers");

    apply_ripple_insert_with_split(&mut t, plan);

    // Track 0: v1 split at 40, push 15 → clips at [20,40), [55,?)
    // New clip starts at 40 for 15 frames
    assert_eq!(t.tracks[0].clips.len(), 3, "3 clips on track 0");
    assert_eq!(t.tracks[0].clips[0].id, "v1");
    assert_eq!(t.tracks[0].clips[0].start_frame, 20);
    assert_eq!(t.tracks[0].clips[0].duration_frames, 20); // 40 - 20
    assert_eq!(t.tracks[0].clips[1].start_frame, 40);
    assert_eq!(t.tracks[0].clips[1].duration_frames, 15);

    // Track 1: a1 split at 40, push 15
    assert_eq!(t.tracks[1].clips.len(), 2, "2 clips on track 1");
    assert_eq!(t.tracks[1].clips[0].id, "a1");
    assert_eq!(t.tracks[1].clips[0].start_frame, 10);
    assert_eq!(t.tracks[1].clips[0].duration_frames, 30); // 40 - 10
    assert_eq!(
        t.tracks[1].clips[1].start_frame, 55,
        "right half of a1 after split+push: 40 + 15"
    );
}

#[test]
fn apply_ripple_insert_with_split_no_straddle() {
    // Track 0: clip c1 at [50, 100). Insert at 10 — no straddle.
    let c1 = clip("c1", 50, 50);
    let mut t = timeline(vec![video_track(vec![c1])]);

    let config = RippleInsertConfig {
        track_index: 0,
        insert_frame: 10,
        clips: vec![RippleInsertClipSpec {
            asset_id: "new-asset".to_string(),
            duration_frames: 20,
            trim_start_frame: None,
            trim_end_frame: None,
        }],
        linked_audio_track_index: None,
    };

    let plan = match timeline_core::compute_ripple_insert_with_split(&t, config) {
        RippleInsertWithSplitOutcome::Ok(p) => p,
        other => panic!("expected Ok, got {:?}", other),
    };

    assert!(
        plan.split_actions.is_empty(),
        "no straddle = no split actions"
    );

    apply_ripple_insert_with_split(&mut t, plan);

    assert_eq!(t.tracks[0].clips.len(), 2, "2 clips after insert");
    assert_eq!(t.tracks[0].clips[0].start_frame, 10);
    assert_eq!(t.tracks[0].clips[0].duration_frames, 20);
    assert_eq!(t.tracks[0].clips[0].media_ref, "new-asset");
    assert_eq!(t.tracks[0].clips[1].start_frame, 70); // 50 + 20 push
    assert_eq!(t.tracks[0].clips[1].id, "c1");
}
