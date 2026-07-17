//! Multicam tool spec (upstream #283): direct transplantation of Swift
//! `MulticamToolTests.swift` (@Suite "multicam tools") through
//! `ToolExecutor::execute` — envelope, short-id, and guard behaviour included.

use agent_contract::tool_exec::ToolExecutor;
use core_model::{ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline};
use serde_json::{json, Value};

fn media_entry(id: &str, ty: ClipType, duration: f64, has_audio: bool) -> MediaManifestEntry {
    MediaManifestEntry {
        id: id.into(),
        name: format!("{id}.media"),
        r#type: ty,
        source: MediaSource::External {
            absolute_path: format!("/{id}"),
        },
        duration,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: Some(has_audio),
        folder_id: None,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
        source_timecode_frame: None,
        source_timecode_quanta: None,
        source_timecode_drop_frame: None,
        ai_tags: None,
        ai_description: None,
        ai_label_status: None,
        generation_status: None,
    }
}

fn harness() -> ToolExecutor {
    let mut manifest = MediaManifest::default();
    manifest
        .entries
        .push(media_entry("camA", ClipType::Video, 120.0, true));
    manifest
        .entries
        .push(media_entry("camB", ClipType::Video, 110.0, true));
    manifest
        .entries
        .push(media_entry("mic1", ClipType::Audio, 130.0, false));
    ToolExecutor::new(Timeline::default(), manifest)
}

/// Stub assets have no readable audio, so members pin offsets — the
/// no-correlation path (same as the Swift ToolHarness).
fn create_args() -> Value {
    json!({"create": {
        "name": "Podcast",
        "members": [
            {"mediaRef": "camA", "kind": "angle", "angleLabel": "cam-a", "offsetSeconds": 0},
            {"mediaRef": "camB", "kind": "angle", "angleLabel": "cam-b", "offsetSeconds": 5},
            {"mediaRef": "mic1", "kind": "mic", "angleLabel": "mic-1", "offsetSeconds": 2},
        ],
        "master": "mic-1",
        "startFrame": 0,
    }})
}

fn payload_of(res: &Value) -> Value {
    serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap()
}

/// Tool payloads shorten ids; direct state checks need the full id.
fn create_group(exec: &mut ToolExecutor) -> String {
    let res = exec.execute("manage_multicam", &create_args()).unwrap();
    let payload = payload_of(&res);
    let short = payload["created"]["groupId"].as_str().unwrap().to_string();
    exec.multicam_groups()
        .iter()
        .find(|g| g.id.starts_with(&short))
        .map(|g| g.id.clone())
        .expect("created group stored on the executor")
}

fn group_clip_ids(exec: &ToolExecutor, group_id: &str) -> Vec<String> {
    timeline_core::multicam_clip_locations(exec.timeline(), group_id)
        .into_iter()
        .map(|(ti, ci)| exec.timeline().tracks[ti].clips[ci].id.clone())
        .collect()
}

#[test]
fn create_reports_group_and_clips() {
    let mut exec = harness();
    let res = exec.execute("manage_multicam", &create_args()).unwrap();
    let payload = payload_of(&res);
    let created = &payload["created"];
    assert!(created["groupId"].is_string());
    let members = created["members"].as_array().unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.iter().all(|m| m["pinned"] == json!(true)));
    assert_eq!(created["clipIds"].as_array().unwrap().len(), 2);

    // The group's clips are plain clips in get_timeline; groups list in the payload.
    let tl = payload_of(&exec.execute("get_timeline", &json!({})).unwrap());
    let groups = tl["multicamGroups"]
        .as_array()
        .expect("multicamGroups listed");
    assert_eq!(groups[0]["angles"], json!(["cam-a", "cam-b"]));
    assert_eq!(groups[0]["mics"], json!(["mic-1"]));
    // Unlinked clips: program video and mic audio each visible on their track.
    let clips: usize = tl["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t.get("clips").and_then(|c| c.as_array()).map(|c| c.len()))
        .sum();
    assert_eq!(clips, 2);
}

#[test]
fn change_cam_cuts_in_place() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);

    let res = exec
        .execute(
            "change_cam",
            &json!({
                "groupId": group_id,
                "entries": [{"range": [600, 1200], "angle": "cam-b"}],
            }),
        )
        .unwrap();
    let payload = payload_of(&res);
    let program = payload["program"].as_array().expect("program rows");
    assert!(program
        .iter()
        .any(|row| row[0] == json!("cam-b") && row[1] == json!(600) && row[2] == json!(1200)));
    assert_eq!(payload["switched"], json!(1));

    let read = payload_of(
        &exec
            .execute("get_multicam", &json!({"groupId": group_id}))
            .unwrap(),
    );
    let rows = read["program"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r[0].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["cam-a", "cam-b", "cam-a"]
    );
}

#[test]
fn change_cam_validates_entries() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);

    let both = exec.execute(
        "change_cam",
        &json!({
            "groupId": group_id,
            "entries": [{"range": [0, 60], "angle": "cam-a", "layout": "grid_2x2"}],
        }),
    );
    assert!(both.is_err());

    let unknown_angle = exec.execute(
        "change_cam",
        &json!({
            "groupId": group_id,
            "entries": [{"range": [0, 60], "angle": "cam-z"}],
        }),
    );
    let err = unknown_angle.unwrap_err();
    assert!(err.contains("cam-a"), "lists valid angles: {err}");
}

#[test]
fn create_rejects_bad_kinds() {
    let mut exec = harness();
    let err = exec
        .execute(
            "manage_multicam",
            &json!({"create": {"members": [
                {"mediaRef": "mic1", "kind": "angle"},
                {"mediaRef": "camA", "kind": "mic"},
            ]}}),
        )
        .unwrap_err();
    assert!(err.contains("video"), "{err}");
}

#[test]
fn resolve_by_clip_id_works() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let clip_id = group_clip_ids(&exec, &group_id)[0].clone();
    let read = payload_of(
        &exec
            .execute("get_multicam", &json!({"clipId": clip_id}))
            .unwrap(),
    );
    let short_id = read["groupId"].as_str().unwrap();
    assert!(group_id.starts_with(short_id));
}

// ── Lifecycle verbs ──────────────────────────────────────────────────────

#[test]
fn ungroup_leaves_ordinary_clips() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let clip_count = group_clip_ids(&exec, &group_id).len();
    let res = exec
        .execute(
            "manage_multicam",
            &json!({"ungroup": {"groupId": group_id}}),
        )
        .unwrap();
    let payload = payload_of(&res);
    assert!(payload["ungrouped"].is_string());
    assert!(group_clip_ids(&exec, &group_id).is_empty());
    assert!(exec.multicam_groups().iter().all(|g| g.id != group_id));
    // Same clips, just unstamped.
    let survivors: usize = exec.timeline().tracks.iter().map(|t| t.clips.len()).sum();
    assert_eq!(survivors, clip_count);
}

// ── Guardrails through the tools ─────────────────────────────────────────

#[test]
fn move_refused_on_group_clips() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let clip_id = group_clip_ids(&exec, &group_id)[0].clone();
    let err = exec
        .execute(
            "move_clips",
            &json!({"moves": [{"clipId": clip_id, "toFrame": 999}]}),
        )
        .unwrap_err();
    assert!(err.contains("sync"), "{err}");
}

#[test]
fn move_whole_group_allowed() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let before: Vec<(String, i64)> =
        timeline_core::multicam_clip_locations(exec.timeline(), &group_id)
            .into_iter()
            .map(|(ti, ci)| {
                let c = &exec.timeline().tracks[ti].clips[ci];
                (c.id.clone(), c.start_frame)
            })
            .collect();
    let moves: Vec<Value> = before
        .iter()
        .map(|(id, start)| json!({"clipId": id, "toFrame": start + 300}))
        .collect();
    exec.execute("move_clips", &json!({"moves": moves}))
        .unwrap();
    let mut starts: Vec<i64> = timeline_core::multicam_clip_locations(exec.timeline(), &group_id)
        .into_iter()
        .map(|(ti, ci)| exec.timeline().tracks[ti].clips[ci].start_frame)
        .collect();
    starts.sort();
    let mut expected: Vec<i64> = before.iter().map(|(_, s)| s + 300).collect();
    expected.sort();
    assert_eq!(starts, expected);
}

#[test]
fn timing_fields_refused_on_group_clips() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let clip_id = group_clip_ids(&exec, &group_id)[0].clone();

    let timing = exec.execute(
        "set_clip_properties",
        &json!({"clipIds": [clip_id], "trimStartFrame": 30}),
    );
    assert!(timing.is_err());
    let speed = exec.execute(
        "set_clip_properties",
        &json!({"clipIds": [clip_id], "speed": 2.0}),
    );
    assert!(speed.is_err());
    let property = exec.execute(
        "set_clip_properties",
        &json!({"clipIds": [clip_id], "opacity": 0.5}),
    );
    assert!(property.is_ok(), "{property:?}");
}

#[test]
fn sync_clips_refused_on_group_clips() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    // A stray non-group clip to use as the reference.
    exec.execute(
        "add_clips",
        &json!({"entries": [{"mediaRef": "camA", "startFrame": 5000}]}),
    )
    .unwrap();
    let stray = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.multicam_group_id.is_none() && c.media_type == ClipType::Video)
        .expect("stray clip placed")
        .id
        .clone();
    let target = group_clip_ids(&exec, &group_id)[0].clone();
    let err = exec
        .execute(
            "sync_clips",
            &json!({"referenceClipId": stray, "targetClipIds": [target]}),
        )
        .unwrap_err();
    assert!(err.contains("already aligned"), "{err}");
}

#[test]
fn group_track_removal_and_unlock_refused() {
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let track_idx = timeline_core::multicam_clip_locations(exec.timeline(), &group_id)[0].0;

    let remove = exec.execute("manage_tracks", &json!({"remove": [track_idx]}));
    assert!(remove.is_err());
    let unlock = exec.execute(
        "manage_tracks",
        &json!({"set": [{"index": track_idx, "syncLocked": false}]}),
    );
    assert!(unlock.is_err());
    // Mute/hide stay free.
    let mute = exec.execute(
        "manage_tracks",
        &json!({"set": [{"index": track_idx, "hidden": true}]}),
    );
    assert!(mute.is_ok(), "{mute:?}");
}

// ── Ripple atomicity through the tool ────────────────────────────────────

#[test]
fn partial_ripple_across_group_refused_via_tool() {
    // ripple_delete_ranges with the program track exempted would shift only
    // the mic — the multicam atomicity guard must refuse (upstream
    // `partialRippleAcrossGroupRefuses`, agent-path form).
    let mut exec = harness();
    let group_id = create_group(&mut exec);
    let mic_track = timeline_core::multicam_clip_locations(exec.timeline(), &group_id)
        .into_iter()
        .find(|(ti, ci)| exec.timeline().tracks[*ti].clips[*ci].media_type == ClipType::Audio)
        .unwrap()
        .0;
    let program_track = timeline_core::multicam_clip_locations(exec.timeline(), &group_id)
        .into_iter()
        .find(|(ti, ci)| exec.timeline().tracks[*ti].clips[*ci].media_type != ClipType::Audio)
        .unwrap()
        .0;
    let err = exec
        .execute(
            "ripple_delete_ranges",
            &json!({
                "trackIndex": mic_track,
                "ranges": [[300, 400]],
                "ignoreSyncLockedTracks": [program_track],
            }),
        )
        .unwrap_err();
    assert!(err.to_lowercase().contains("multicam"), "{err}");

    // The atomic form (no exemptions) stays allowed.
    let ok = exec.execute(
        "ripple_delete_ranges",
        &json!({"trackIndex": 0, "ranges": [[300, 400]]}),
    );
    assert!(ok.is_ok(), "{ok:?}");
}

// ── Manual segment switch (untooled executor operation) ──────────────────

/// Two-mic harness mirroring the engine spec's
/// `switch_mic_rewrites_audio_clip_in_place`: camA angle, lapel (master,
/// +2s) and room (0s) mics, lapel lane split so [600, 1200) is the target.
fn segment_switch_harness() -> (ToolExecutor, String, String) {
    let mut manifest = MediaManifest::default();
    manifest
        .entries
        .push(media_entry("camA", ClipType::Video, 120.0, true));
    manifest
        .entries
        .push(media_entry("lapel", ClipType::Audio, 130.0, false));
    manifest
        .entries
        .push(media_entry("room", ClipType::Audio, 125.0, false));
    let mut exec = ToolExecutor::new(Timeline::default(), manifest);
    exec.execute(
        "manage_multicam",
        &json!({"create": {
            "name": "MC",
            "members": [
                {"mediaRef": "camA", "kind": "angle", "angleLabel": "cam-a", "offsetSeconds": 0},
                {"mediaRef": "lapel", "kind": "mic", "angleLabel": "lapel", "offsetSeconds": 2},
                {"mediaRef": "room", "kind": "mic", "angleLabel": "room", "offsetSeconds": 0},
            ],
            "master": "lapel",
            "startFrame": 0,
        }}),
    )
    .unwrap();
    let group_id = exec.multicam_groups()[0].id.clone();
    let lapel = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.media_ref == "lapel")
        .expect("lapel mic clip")
        .id
        .clone();
    exec.execute(
        "split_clips",
        &json!({"splits": [{"clipId": lapel, "atFrame": 600}]}),
    )
    .unwrap();
    let mid = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.media_ref == "lapel" && c.start_frame == 600)
        .expect("middle lapel piece")
        .id
        .clone();
    exec.execute(
        "split_clips",
        &json!({"splits": [{"clipId": mid, "atFrame": 1200}]}),
    )
    .unwrap();
    let target = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.media_ref == "lapel" && c.start_frame == 600)
        .expect("target piece")
        .id
        .clone();
    (exec, group_id, target)
}

#[test]
fn switch_segment_rewrites_mic_in_place_with_undo() {
    let (mut exec, group_id, target) = segment_switch_harness();
    // The mic roster the tab offers (Swift multicamAudioBearers).
    assert_eq!(
        exec.multicam_audio_bearer_labels(&group_id),
        ["cam-a", "lapel", "room"]
    );

    let rev = exec.revision();
    let undo_len = exec.undo_stack().len();
    exec.switch_multicam_segment(&target, "room").unwrap();

    // Engine-spec expectations (`switch_mic_rewrites_audio_clip_in_place`).
    let swapped = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == target)
        .expect("target survives in place")
        .clone();
    assert_eq!(swapped.media_ref, "room");
    // Same real moment on the room mic's clock: lapel trim 540 + 2s·30.
    assert_eq!(swapped.trim_start_frame, 600);
    assert_eq!(swapped.start_frame, 600);
    assert_eq!(swapped.start_frame + swapped.duration_frames, 1200);
    let lapel_count = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .filter(|c| c.media_ref == "lapel")
        .count();
    assert_eq!(lapel_count, 2, "neighbors untouched");

    // Undo-tracked + revision-bumped like a tool mutation.
    assert_eq!(exec.undo_stack().len(), undo_len + 1);
    assert!(exec.revision() > rev);
    exec.execute("undo", &json!({})).unwrap();
    let reverted = exec
        .timeline()
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.id == target)
        .unwrap()
        .clone();
    assert_eq!(reverted.media_ref, "lapel");
    assert_eq!(reverted.trim_start_frame, 540);
}

#[test]
fn switch_segment_errors_leave_state_untouched() {
    let (mut exec, _group_id, target) = segment_switch_harness();
    let before = exec.timeline().clone();
    let rev = exec.revision();
    let undo_len = exec.undo_stack().len();

    let err = exec
        .switch_multicam_segment(&target, "nope")
        .unwrap_err();
    assert!(err.contains("Unknown mic"), "{err}");
    assert!(err.contains("room"), "lists valid mics: {err}");

    let stray = exec.switch_multicam_segment("not-a-clip", "room").unwrap_err();
    assert!(stray.contains("not part of a multicam group"), "{stray}");

    assert_eq!(exec.timeline(), &before);
    assert_eq!(exec.revision(), rev);
    assert_eq!(exec.undo_stack().len(), undo_len);
}

#[test]
fn create_timecode_sync_uses_seam_exact_ntsc_seconds() {
    // D1: NDF NTSC members sync by shared timecode; the ClipAudioSource seam
    // supplies the exact 1001/30000 per-TC-frame duration, so the member's
    // offset is 3003 × 1001/30000 s — not the naive 3003/30 — matching the
    // sync_clips path exactly.
    struct TcSeam;
    impl agent_contract::ClipAudioSource for TcSeam {
        fn decode_source_pcm(
            &self,
            _source: &MediaSource,
            _sample_rate: u32,
            _channels: usize,
        ) -> Option<Vec<f32>> {
            None
        }
        fn timecode_frame_duration(&self, _source: &MediaSource) -> Option<(i64, i64)> {
            Some((1001, 30_000))
        }
    }
    let mut manifest = MediaManifest::default();
    let mut mic = media_entry("mic1", ClipType::Audio, 130.0, true);
    mic.source_timecode_frame = Some(90_000);
    mic.source_timecode_quanta = Some(30);
    let mut cam = media_entry("camA", ClipType::Video, 120.0, true);
    cam.source_timecode_frame = Some(93_003);
    cam.source_timecode_quanta = Some(30);
    manifest.entries.push(mic);
    manifest.entries.push(cam);
    let mut exec = ToolExecutor::new(Timeline::default(), manifest);
    exec.set_audio_source(std::sync::Arc::new(TcSeam));
    exec.execute(
        "manage_multicam",
        &json!({"create": {
            "members": [
                {"mediaRef": "mic1", "kind": "mic", "angleLabel": "mic-1"},
                {"mediaRef": "camA", "kind": "angle", "angleLabel": "cam-a"},
            ],
            "master": "mic-1",
            "startFrame": 0,
        }}),
    )
    .unwrap();
    let group = &exec.multicam_groups()[0];
    let cam_member = group
        .members
        .iter()
        .find(|m| m.media_ref == "camA")
        .expect("camA member stored");
    let exact = (93_003i64 * 1001) as f64 / 30_000.0 - (90_000i64 * 1001) as f64 / 30_000.0;
    assert_eq!(
        cam_member.sync.offset_seconds, exact,
        "seam-exact NTSC offset"
    );
    assert!(
        (cam_member.sync.offset_seconds - 3003.0 / 30.0).abs() > 1e-4,
        "must not be the naive 1/quanta offset: {}",
        cam_member.sync.offset_seconds
    );
}
