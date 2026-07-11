//! Multicam engine spec (upstream #283): direct transplantation of Swift
//! `MulticamEngineTests.swift` (@Suite "Multicam"), same inputs and
//! expectations. Editor-only cases (undo manager, toast plumbing, commitTrim
//! overwrite) are covered at their own layers or noted as deviations.

use core_model::{
    Clip, ClipType, Crop, MulticamMemberKind, MulticamSource, MulticamSyncMap, Timeline, Transform,
    VideoLayout,
};
use std::collections::{HashMap, HashSet};
use timeline_core::{
    create_group, live_groups, max_lag_hops, multicam_atomicity_violation, multicam_clip_locations,
    multicam_group_offsets, multicam_manual_ripple_violation, multicam_move_violation,
    multicam_program_rows, multicam_trim_bounds, referenced_group_ids, strip_group_stamps,
    switch_angles, switch_segment, AngleSwitchRequest, ClipMathExt, FrameRange, MulticamAsset,
    MulticamMemberSpec,
};

fn asset(name: &str, ty: ClipType, duration: f64, has_audio: bool) -> MulticamAsset {
    MulticamAsset {
        name: name.to_string(),
        clip_type: ty,
        duration,
        has_audio,
        source_width: None,
        source_height: None,
    }
}

fn spec(media_ref: &str, kind: MulticamMemberKind, label: &str) -> MulticamMemberSpec {
    MulticamMemberSpec {
        media_ref: media_ref.to_string(),
        kind,
        angle_label: Some(label.to_string()),
        pinned_offset_seconds: None,
    }
}

fn sync(offset: f64, confidence: f64) -> MulticamSyncMap {
    MulticamSyncMap {
        offset_seconds: offset,
        confidence,
        locked: false,
    }
}

/// Group clock at 30fps: camA covers [0, 3600), camB [150, 3450), mic1
/// (master) [60, 3960) — the Swift harness.
fn harness_assets() -> HashMap<String, MulticamAsset> {
    HashMap::from([
        (
            "camA".to_string(),
            asset("camA", ClipType::Video, 120.0, true),
        ),
        (
            "camB".to_string(),
            asset("camB", ClipType::Video, 110.0, true),
        ),
        (
            "mic1".to_string(),
            asset("mic1", ClipType::Audio, 130.0, false),
        ),
    ])
}

fn harness_specs() -> Vec<MulticamMemberSpec> {
    vec![
        spec("camA", MulticamMemberKind::Angle, "cam-a"),
        spec("camB", MulticamMemberKind::Angle, "cam-b"),
        spec("mic1", MulticamMemberKind::Mic, "mic-1"),
    ]
}

fn harness_maps() -> HashMap<String, MulticamSyncMap> {
    HashMap::from([
        ("camA".to_string(), sync(0.0, 1.0)),
        ("camB".to_string(), sync(5.0, 0.9)),
        ("mic1".to_string(), sync(2.0, 1.0)),
    ])
}

fn create_harness_group(
    timeline: &mut Timeline,
    assets: &HashMap<String, MulticamAsset>,
) -> (MulticamSource, Vec<String>) {
    create_group(
        timeline,
        &harness_specs(),
        &harness_maps(),
        "mic1",
        Some("MC"),
        &[],
        assets,
        Some(0),
    )
    .expect("group creation")
}

fn program_clips(timeline: &Timeline, group_id: &str) -> Vec<Clip> {
    let mut clips: Vec<Clip> = multicam_clip_locations(timeline, group_id)
        .into_iter()
        .filter(|(ti, ci)| {
            timeline.tracks[*ti].r#type == ClipType::Video
                && timeline.tracks[*ti].clips[*ci].media_type != ClipType::Audio
        })
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .collect();
    clips.sort_by_key(|c| c.start_frame);
    clips
}

fn mic_clip(timeline: &Timeline, group_id: &str) -> Clip {
    multicam_clip_locations(timeline, group_id)
        .into_iter()
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .find(|c| c.media_type == ClipType::Audio)
        .expect("mic clip")
}

fn find_clip(timeline: &Timeline, id: &str) -> Clip {
    let loc = timeline_core::find_clip(timeline, id).expect("clip exists");
    timeline.tracks[loc.track_index].clips[loc.clip_index].clone()
}

/// The agent-path range ripple with every track sync-locked (Swift
/// `rippleDeleteRangesOnTrack` semantics: clear the ranges on all tracks,
/// then close the gaps per track) — the whole group shifts atomically.
fn ripple_all_tracks(timeline: &mut Timeline, ranges: &[FrameRange]) {
    let merged = timeline_core::merge_ranges(ranges);
    for ti in 0..timeline.tracks.len() {
        for r in &merged {
            timeline_core::clear_region(timeline, ti, r.start, r.end, false);
        }
    }
    for track in &mut timeline.tracks {
        let shifts = timeline_core::compute_ripple_shifts_for_ranges(&track.clips, &merged);
        for s in shifts {
            if let Some(c) = track.clips.iter_mut().find(|c| c.id == s.clip_id) {
                c.start_frame = s.new_start_frame;
            }
        }
        track.clips.sort_by_key(|c| c.start_frame);
    }
}

// ── Model ────────────────────────────────────────────────────────────────

#[test]
fn lag_search_keeps_half_overlap() {
    // 3:35 files (~21500 hops) with a 240s window: without the clamp, ±220s
    // lags with seconds of overlap were legal — the false-peak that doubled a
    // group's length.
    assert_eq!(max_lag_hops(240.0, 0.01, 21500, 21500), 10750);
    assert_eq!(max_lag_hops(240.0, 0.01, 54000, 54000), 24000);
    assert_eq!(max_lag_hops(240.0, 0.01, 0, 100), 1);
}

// ── Creation ─────────────────────────────────────────────────────────────

#[test]
fn create_fills_program_holes_with_covering_angles() {
    // Seed camera stops early; a longer angle must fill the tail — no gap
    // where some camera has picture.
    let assets = HashMap::from([
        (
            "short".to_string(),
            asset("short", ClipType::Video, 20.0, true),
        ),
        (
            "wide".to_string(),
            asset("wide", ClipType::Video, 120.0, true),
        ),
        (
            "mic1".to_string(),
            asset("mic1", ClipType::Audio, 130.0, false),
        ),
    ]);
    let maps = HashMap::from([
        ("short".to_string(), sync(0.0, 1.0)),
        ("wide".to_string(), sync(0.0, 1.0)),
        ("mic1".to_string(), sync(0.0, 1.0)),
    ]);
    let mut timeline = Timeline::default();
    let (group, _) = create_group(
        &mut timeline,
        &[
            spec("short", MulticamMemberKind::Angle, "close"),
            spec("wide", MulticamMemberKind::Angle, "wide"),
            spec("mic1", MulticamMemberKind::Mic, "mic-1"),
        ],
        &maps,
        "mic1",
        Some("MC"),
        &[],
        &assets,
        Some(0),
    )
    .unwrap();
    let program = program_clips(&timeline, &group.id);
    assert_eq!(
        program
            .iter()
            .map(|c| c.media_ref.as_str())
            .collect::<Vec<_>>(),
        ["short", "wide"]
    );
    assert_eq!(program[0].end_frame(), 600);
    assert_eq!(program[1].start_frame, 600);
    assert_eq!(program[1].end_frame(), 3600);
    // The filler shows its own source at the right moment, not from zero.
    assert_eq!(program[1].trim_start_frame, 600);
}

#[test]
fn create_lays_stamped_clips() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, clip_ids) = create_harness_group(&mut timeline, &assets);
    assert_eq!(clip_ids.len(), 2);

    let program = program_clips(&timeline, &group.id);
    assert_eq!(program.len(), 1);
    // Video spans the union of camera coverage.
    assert_eq!(program[0].start_frame, 0);
    assert_eq!(program[0].end_frame(), 3600);
    assert_eq!(program[0].media_ref, "camA");
    assert_eq!(
        program[0].multicam_group_id.as_deref(),
        Some(group.id.as_str())
    );
    assert_eq!(
        group
            .member_by_media_ref(&program[0].media_ref)
            .map(|m| m.angle_label.as_str()),
        Some("cam-a")
    );

    // Mic is its own audio clip at its offset — stamped, not linked: clips
    // select and delete individually.
    let mic = mic_clip(&timeline, &group.id);
    assert_eq!(mic.start_frame, 60);
    assert_eq!(mic.duration_frames, 3900);
    assert_eq!(mic.trim_start_frame, 0);
    assert_eq!(mic.link_group_id, None);

    assert_eq!(group.name, "MC");
}

#[test]
fn switch_mic_rewrites_audio_clip_in_place() {
    let assets = HashMap::from([
        (
            "camA".to_string(),
            asset("camA", ClipType::Video, 120.0, true),
        ),
        (
            "lapel".to_string(),
            asset("lapel", ClipType::Audio, 130.0, false),
        ),
        (
            "room".to_string(),
            asset("room", ClipType::Audio, 125.0, false),
        ),
    ]);
    let maps = HashMap::from([
        ("camA".to_string(), sync(0.0, 1.0)),
        ("lapel".to_string(), sync(2.0, 1.0)),
        ("room".to_string(), sync(0.0, 1.0)),
    ]);
    let mut timeline = Timeline::default();
    let (group, _) = create_group(
        &mut timeline,
        &[
            spec("camA", MulticamMemberKind::Angle, "cam-a"),
            spec("lapel", MulticamMemberKind::Mic, "lapel"),
            spec("room", MulticamMemberKind::Mic, "room"),
        ],
        &maps,
        "lapel",
        Some("MC"),
        &[],
        &assets,
        Some(0),
    )
    .unwrap();

    // Chop the lapel lane, then switch just the middle piece to the room mic.
    let lapel = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .find(|c| c.media_ref == "lapel")
        .unwrap();
    timeline_core::split_clip(&mut timeline, &lapel.id, 600);
    let mid = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .find(|c| c.media_ref == "lapel" && c.start_frame == 600)
        .unwrap();
    timeline_core::split_clip(&mut timeline, &mid.id, 1200);
    let target = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .find(|c| c.media_ref == "lapel" && c.start_frame == 600)
        .unwrap();

    switch_segment(&mut timeline, &group, &target.id, "room", &assets).unwrap();
    let swapped = find_clip(&timeline, &target.id);
    assert_eq!(swapped.media_ref, "room");
    // Same real moment on the room mic's clock: lapel trim 540 + 2s·30.
    assert_eq!(swapped.trim_start_frame, 600);
    assert_eq!(swapped.start_frame, 600);
    assert_eq!(swapped.end_frame(), 1200);
    // Neighbors untouched.
    let lapel_count = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .filter(|(ti, ci)| timeline.tracks[*ti].clips[*ci].media_ref == "lapel")
        .count();
    assert_eq!(lapel_count, 2);
}

// ── Switching ────────────────────────────────────────────────────────────

#[test]
fn switch_rewrites_trim_through_sync_maps() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let outcome = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-b")],
        &assets,
    )
    .unwrap();
    assert_eq!(outcome.switched, 1);
    let clips = program_clips(&timeline, &group.id);
    assert_eq!(
        clips
            .iter()
            .map(|c| c.media_ref.as_str())
            .collect::<Vec<_>>(),
        ["camA", "camB", "camA"]
    );
    // Same real moment on cam-b's clock: 600 - (5-0)*30 = 450.
    assert_eq!(clips[1].trim_start_frame, 450);
    assert_eq!(clips[1].start_frame, 600);
    assert_eq!(clips[1].end_frame(), 1200);
    assert_eq!(clips[0].trim_start_frame, 0);
    assert_eq!(clips[2].trim_start_frame, 1200);
    assert!(clips
        .iter()
        .all(|c| c.multicam_group_id.as_deref() == Some(group.id.as_str())));
}

#[test]
fn switch_clamps_to_angle_coverage() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    // cam-b has no picture before group frame 150.
    let outcome = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(0..600, "cam-b")],
        &assets,
    )
    .unwrap();
    assert_eq!(outcome.clamped.len(), 1);
    assert_eq!(outcome.clamped[0].applied, 150..600);
    assert_eq!(outcome.clamped[0].culprit, "cam-b");
    let clips = program_clips(&timeline, &group.id);
    assert_eq!(
        clips
            .iter()
            .map(|c| c.media_ref.as_str())
            .collect::<Vec<_>>(),
        ["camA", "camB", "camA"]
    );
    assert_eq!(clips[1].start_frame, 150);
}

#[test]
fn user_framing_survives_angle_switch() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);

    // Cut a fragment out, then punch in on just that fragment.
    let program = program_clips(&timeline, &group.id)[0].clone();
    timeline_core::split_clip(&mut timeline, &program.id, 600);
    let fragment = program_clips(&timeline, &group.id)
        .into_iter()
        .find(|c| c.start_frame == 600)
        .unwrap();
    timeline_core::split_clip(&mut timeline, &fragment.id, 1200);
    let punched = Transform {
        width: 1.2,
        height: 1.2,
        ..Transform::default()
    };
    let punched_crop = Crop {
        left: 0.1,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };
    let framed = program_clips(&timeline, &group.id)
        .into_iter()
        .find(|c| c.start_frame == 600)
        .unwrap();
    {
        let loc = timeline_core::find_clip(&timeline, &framed.id).unwrap();
        let clip = &mut timeline.tracks[loc.track_index].clips[loc.clip_index];
        clip.transform = punched;
        clip.crop = punched_crop;
    }

    // The punch-in rides the swap; the crop is untouched.
    switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-b")],
        &assets,
    )
    .unwrap();
    let swapped = program_clips(&timeline, &group.id)
        .into_iter()
        .find(|c| c.start_frame == 600)
        .unwrap();
    assert_eq!(swapped.media_ref, "camB");
    assert_eq!(swapped.transform, punched);
    assert_eq!(swapped.crop, punched_crop);

    // An unframed fragment stays at the default fit after switching.
    switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(1500..1800, "cam-b")],
        &assets,
    )
    .unwrap();
    let plain = program_clips(&timeline, &group.id)
        .into_iter()
        .find(|c| c.start_frame == 1500)
        .unwrap();
    assert_eq!(plain.crop, Crop::default());
}

#[test]
fn same_angle_switch_merges_back() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-b")],
        &assets,
    )
    .unwrap();
    let back = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-a")],
        &assets,
    )
    .unwrap();
    assert_eq!(back.merged, 2);
    assert_eq!(program_clips(&timeline, &group.id).len(), 1);
}

#[test]
fn switch_survives_word_cut_fragments() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    // A word cut: ripple out [900, 1000) — the whole group shifts atomically.
    ripple_all_tracks(
        &mut timeline,
        &[FrameRange {
            start: 900,
            end: 1000,
        }],
    );
    // Switch across the seam.
    let outcome = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(800..1100, "cam-b")],
        &assets,
    )
    .unwrap();
    assert_eq!(outcome.switched, 2);
    let swapped: Vec<Clip> = program_clips(&timeline, &group.id)
        .into_iter()
        .filter(|c| c.media_ref == "camB")
        .collect();
    // Each fragment shows cam-b at the same real time its camA content had:
    // before the seam camA trim 800 → camB 650; after it camA trim 1000 → camB 850.
    assert!(swapped
        .iter()
        .any(|c| c.start_frame == 800 && c.trim_start_frame == 650));
    assert!(swapped.iter().any(|c| c.trim_start_frame == 850));
}

#[test]
fn switch_outside_group_skips() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let outcome = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(90000..90600, "cam-b")],
        &assets,
    )
    .unwrap();
    assert_eq!(outcome.switched, 0);
    assert_eq!(outcome.skipped.len(), 1);
}

#[test]
fn switch_layout_places_overlays_and_full_frame_clears_them() {
    // change_cam's layout mode: {range, layout, angles} places extra angles as
    // synced overlay clips above the program; a later full-frame entry over
    // the same range clears them.
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let outcome = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::with_layout(
            600..1200,
            VideoLayout::SideBySide,
            vec!["cam-a".to_string(), "cam-b".to_string()],
        )],
        &assets,
    )
    .unwrap();
    assert_eq!(outcome.overlay_clip_ids.len(), 1);
    let overlay = find_clip(&timeline, &outcome.overlay_clip_ids[0]);
    assert_eq!(overlay.media_ref, "camB");
    assert_eq!(overlay.start_frame, 600);
    assert_eq!(overlay.end_frame(), 1200);
    // Synced: same real moment on cam-b's clock.
    assert_eq!(overlay.trim_start_frame, 450);
    assert_eq!(
        overlay.multicam_group_id.as_deref(),
        Some(group.id.as_str())
    );

    let back = switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-a")],
        &assets,
    )
    .unwrap();
    assert!(back.overlay_clip_ids.is_empty());
    assert!(
        timeline_core::find_clip(&timeline, &overlay.id).is_none(),
        "full-frame switch clears the layout overlays"
    );
    assert_eq!(program_clips(&timeline, &group.id).len(), 1, "merged back");
}

// ── Lifecycle ────────────────────────────────────────────────────────────

#[test]
fn ungroup_strips_stamps_and_drops_metadata() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, clip_ids) = create_harness_group(&mut timeline, &assets);
    strip_group_stamps(&mut timeline, &group.id);
    assert!(multicam_clip_locations(&timeline, &group.id).is_empty());
    assert!(!referenced_group_ids([&timeline]).contains(&group.id));
    // Clips stay put as ordinary clips.
    let survivors: Vec<&Clip> = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| clip_ids.contains(&c.id))
        .collect();
    assert_eq!(survivors.len(), clip_ids.len());
    assert!(survivors.iter().all(|c| c.multicam_group_id.is_none()));
}

#[test]
fn unreferenced_groups_dont_persist() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, clip_ids) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    assert_eq!(live_groups(&groups, [&timeline]).len(), 1);
    timeline_core::remove_clips(&mut timeline, clip_ids, false);
    // Metadata may stay in memory for undo, but is filtered from saves.
    assert!(live_groups(&groups, [&timeline]).is_empty());
}

// ── Guardrails ───────────────────────────────────────────────────────────

#[test]
fn partial_ripple_across_group_refuses() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    let mic_track = *multicam_track_indexes_of(&timeline, &group.id, true)
        .first()
        .unwrap();
    // Only the mic track would shift (program track exempted) — must refuse.
    let shifting: HashSet<usize> = HashSet::from([mic_track]);
    let reason =
        multicam_atomicity_violation(&timeline, &groups, &shifting).expect("expected refusal");
    assert!(reason.to_lowercase().contains("multicam"), "{reason}");
}

fn multicam_track_indexes_of(timeline: &Timeline, group_id: &str, audio: bool) -> Vec<usize> {
    let mut out: Vec<usize> = multicam_clip_locations(timeline, group_id)
        .into_iter()
        .filter(|(ti, ci)| (timeline.tracks[*ti].clips[*ci].media_type == ClipType::Audio) == audio)
        .map(|(ti, _)| ti)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn atomic_ripple_keeps_relative_alignment() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    ripple_all_tracks(
        &mut timeline,
        &[FrameRange {
            start: 300,
            end: 400,
        }],
    );
    let program = program_clips(&timeline, &group.id);
    // Fragment after the cut resumes at source 400 — content at a position never changed.
    assert_eq!(
        program
            .iter()
            .find(|c| c.start_frame == 300)
            .map(|c| c.trim_start_frame),
        Some(400)
    );
    // The mic carries the same 100-frame cut: both sides of the seam keep
    // source-time = group-time - offset.
    let mut mic_fragments: Vec<Clip> = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .map(|(ti, ci)| timeline.tracks[ti].clips[ci].clone())
        .filter(|c| c.media_type == ClipType::Audio)
        .collect();
    mic_fragments.sort_by_key(|c| c.start_frame);
    assert_eq!(mic_fragments.len(), 2);
    assert_eq!(mic_fragments[1].start_frame, 300);
    // mic offset is 2s (60 frames): group 400 → mic source 340.
    assert_eq!(mic_fragments[1].trim_start_frame, 340);
}

#[test]
fn partial_move_refused() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    let program = program_clips(&timeline, &group.id)[0].clone();
    let track = timeline_core::find_clip(&timeline, &program.id)
        .unwrap()
        .track_index;
    // Horizontal shift of a subset must not land.
    let reason = multicam_move_violation(
        &timeline,
        &groups,
        &[(program.id.clone(), track, program.start_frame + 500)],
    );
    assert!(reason.is_some());
    assert!(reason.unwrap().contains("sync"));
}

#[test]
fn move_whole_group_allowed_and_lane_change_refused() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    // Whole group moving together passes the guard.
    let moves: Vec<(String, usize, i64)> = multicam_clip_locations(&timeline, &group.id)
        .into_iter()
        .map(|(ti, ci)| {
            let c = &timeline.tracks[ti].clips[ci];
            (c.id.clone(), ti, c.start_frame + 300)
        })
        .collect();
    assert_eq!(multicam_move_violation(&timeline, &groups, &moves), None);
    // A camera lane change is always refused.
    let program = program_clips(&timeline, &group.id)[0].clone();
    let track = timeline_core::find_clip(&timeline, &program.id)
        .unwrap()
        .track_index;
    let reason = multicam_move_violation(
        &timeline,
        &groups,
        &[(program.id.clone(), track + 1, program.start_frame)],
    );
    assert!(reason.unwrap().contains("program track stays fixed"));
}

#[test]
fn split_and_delete_stay_free() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let clip = program_clips(&timeline, &group.id)[0].clone();
    timeline_core::split_clip(&mut timeline, &clip.id, 600);
    let clips = program_clips(&timeline, &group.id);
    assert_eq!(clips.len(), 2);
    assert!(clips
        .iter()
        .all(|c| c.multicam_group_id.as_deref() == Some(group.id.as_str())));
    timeline_core::remove_clips(&mut timeline, [clips[1].id.clone()], false);
    assert_eq!(program_clips(&timeline, &group.id).len(), 1);
}

#[test]
fn manual_ripple_refused_only_when_straddling_group() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    let program = program_clips(&timeline, &group.id)[0].clone();
    timeline_core::split_clip(&mut timeline, &program.id, 600);

    // Deleting the first fragment shifts through the mic's middle — refused.
    let all_tracks: HashSet<usize> = (0..timeline.tracks.len()).collect();
    let reason = multicam_manual_ripple_violation(&timeline, &groups, &all_tracks, 600)
        .expect("straddling manual ripple refused");
    assert!(reason.to_lowercase().contains("multicam"), "{reason}");

    // Range ripples (remove_silence / remove_words) stay allowed: the
    // atomicity check alone passes when every group track shifts.
    assert_eq!(
        multicam_atomicity_violation(&timeline, &groups, &all_tracks),
        None
    );
}

#[test]
fn manual_ripple_after_group_stays_allowed() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    // A plain clip entirely after the group; deleting it shifts everything
    // after — including nothing of the group — so it must pass.
    let all_tracks: HashSet<usize> = (0..timeline.tracks.len()).collect();
    assert_eq!(
        multicam_manual_ripple_violation(&timeline, &groups, &all_tracks, 8600),
        None
    );
}

#[test]
fn angle_switch_leaves_unrelated_upper_track_clips_whole() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    // A title on a NEW top track overlapping the switch range.
    let idx = timeline_core::insert_track_at(&mut timeline, 0, ClipType::Video).unwrap();
    let mut title = program_clips(&timeline, &group.id)[0].clone();
    title.id = "title-clip".to_string();
    title.media_ref = "title".to_string();
    title.multicam_group_id = None;
    title.start_frame = 100;
    title.duration_frames = 1800;
    timeline.tracks[idx].clips.push(title);

    switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(300..900, "cam-b")],
        &assets,
    )
    .unwrap();
    let title_count = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| c.media_ref == "title")
        .count();
    assert_eq!(title_count, 1);
}

#[test]
fn duplicate_drops_stamp() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let program = program_clips(&timeline, &group.id)[0].clone();
    let track_idx = timeline_core::find_clip(&timeline, &program.id)
        .unwrap()
        .track_index;
    timeline_core::ClipClipboardEngine::duplicate_clips(
        &mut timeline,
        &[program.id.clone()],
        Some(track_idx),
        9000,
    );
    let clone = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.start_frame == 9000)
        .expect("duplicate placed");
    assert_eq!(clone.multicam_group_id, None);
}

#[test]
fn trim_stops_at_ripple_seam() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    // Ripple out [600, 700): a seam at 600 with 100 frames of cut time.
    ripple_all_tracks(
        &mut timeline,
        &[FrameRange {
            start: 600,
            end: 700,
        }],
    );
    // Delete the program fragment left of the seam; growth of the right
    // fragment leftward across the seam must be capped at 0 — those frames
    // were cut and the mics no longer carry them.
    let program = program_clips(&timeline, &group.id);
    let left = program
        .iter()
        .find(|c| c.end_frame() == 600)
        .unwrap()
        .clone();
    let right = program
        .iter()
        .find(|c| c.start_frame == 600)
        .unwrap()
        .clone();
    timeline_core::remove_clips(&mut timeline, [left.id], false);

    let bounds = multicam_trim_bounds(&timeline, &group, &right.id).expect("bounds");
    assert_eq!(bounds.0, 0, "seam stop: leftward growth is 0");
    // Coverage stop: rightward growth caps at the camera's remaining footage.
    let tail = find_clip(&timeline, &right.id);
    assert_eq!(bounds.1, tail.trim_end_frame);
}

// ── Chip, program read ───────────────────────────────────────────────────

#[test]
fn synced_members_show_no_link_offset_badge() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    let groups = vec![group.clone()];
    // Members sit at different trims by design (different files, one clock) —
    // the misalignment badge must stay silent for an in-sync group.
    assert!(multicam_group_offsets(&timeline, &groups).is_empty());

    // Ripple cuts shift each column by a different total — anchors differ
    // across columns but agree within one. No badge (remove_silence case).
    ripple_all_tracks(
        &mut timeline,
        &[
            FrameRange {
                start: 300,
                end: 400,
            },
            FrameRange {
                start: 900,
                end: 1050,
            },
        ],
    );
    assert!(multicam_group_offsets(&timeline, &groups).is_empty());

    // A genuine slip (dodging the guards) must still be flagged.
    let mic = mic_clip(&timeline, &group.id);
    let loc = timeline_core::find_clip(&timeline, &mic.id).unwrap();
    timeline.tracks[loc.track_index].clips[loc.clip_index].trim_start_frame += 24;
    assert!(!multicam_group_offsets(&timeline, &groups).is_empty());
}

#[test]
fn program_rows_run_length_merge() {
    let assets = harness_assets();
    let mut timeline = Timeline::default();
    let (group, _) = create_harness_group(&mut timeline, &assets);
    switch_angles(
        &mut timeline,
        &group,
        &[AngleSwitchRequest::full(600..1200, "cam-b")],
        &assets,
    )
    .unwrap();
    let rows = multicam_program_rows(&timeline, &group, None);
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        ["cam-a", "cam-b", "cam-a"]
    );
    assert_eq!(rows[1].1, 600);
    assert_eq!(rows[1].2, 1200);
}
