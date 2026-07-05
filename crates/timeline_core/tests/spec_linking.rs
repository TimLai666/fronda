use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use std::collections::BTreeSet;
use timeline_core::{
    build_link_index, expand_to_link_group, gap_is_still_empty, link_clips, link_group_offsets,
    linked_partner_ids, partner_moves_for_move_of, unlink_clips, FrameRange,
};

fn clip(id: &str, media_type: ClipType, start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: id.to_string(),
        media_ref: format!("asset-{id}"),
        media_type,
        source_clip_type: media_type,
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

fn timeline(tracks: Vec<Track>) -> Timeline {
    Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks,
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
    }
}

#[test]
fn lnk_001_link_index_maps_group_ids_to_members() {
    let mut video = clip("v", ClipType::Video, 0, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 0, 30);
    audio.link_group_id = Some("g1".to_string());
    let solo = clip("solo", ClipType::Video, 0, 30);
    let timeline = timeline(vec![
        video_track(vec![video, solo]),
        audio_track(vec![audio]),
    ]);

    let index = build_link_index(&timeline);
    assert_eq!(
        index.get("g1").cloned(),
        Some(BTreeSet::from(["a".to_string(), "v".to_string(),]))
    );
    assert!(!index.values().any(|members| members.contains("solo")));
}

#[test]
fn lnk_002_expand_to_link_group_returns_input_when_none_are_linked() {
    let timeline = timeline(vec![video_track(vec![clip("a", ClipType::Video, 0, 30)])]);
    let ids = BTreeSet::from(["a".to_string()]);

    assert_eq!(expand_to_link_group(&timeline, &ids), ids);
}

#[test]
fn lnk_002_expand_to_link_group_pulls_in_all_partners() {
    let mut video = clip("v", ClipType::Video, 0, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 0, 30);
    audio.link_group_id = Some("g1".to_string());
    let timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);
    let seed = BTreeSet::from(["v".to_string()]);

    assert_eq!(
        expand_to_link_group(&timeline, &seed),
        BTreeSet::from(["a".to_string(), "v".to_string()])
    );
}

#[test]
fn lnk_002_expand_to_link_group_handles_multiple_groups() {
    let mut v1 = clip("v1", ClipType::Video, 0, 30);
    v1.link_group_id = Some("g1".to_string());
    let mut a1 = clip("a1", ClipType::Audio, 0, 30);
    a1.link_group_id = Some("g1".to_string());
    let mut v2 = clip("v2", ClipType::Video, 100, 30);
    v2.link_group_id = Some("g2".to_string());
    let mut a2 = clip("a2", ClipType::Audio, 100, 30);
    a2.link_group_id = Some("g2".to_string());
    let timeline = timeline(vec![video_track(vec![v1, v2]), audio_track(vec![a1, a2])]);
    let seed = BTreeSet::from(["v1".to_string(), "v2".to_string()]);

    assert_eq!(
        expand_to_link_group(&timeline, &seed),
        BTreeSet::from([
            "a1".to_string(),
            "a2".to_string(),
            "v1".to_string(),
            "v2".to_string(),
        ])
    );
}

#[test]
fn lnk_003_linked_partner_ids_exclude_self() {
    let mut video = clip("v", ClipType::Video, 0, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 0, 30);
    audio.link_group_id = Some("g1".to_string());
    let timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);

    assert_eq!(linked_partner_ids(&timeline, "v"), vec!["a".to_string()]);
    assert_eq!(linked_partner_ids(&timeline, "a"), vec!["v".to_string()]);
}

#[test]
fn lnk_003_linked_partner_ids_are_empty_for_ungrouped_or_unknown_clip() {
    let timeline = timeline(vec![video_track(vec![clip(
        "solo",
        ClipType::Video,
        0,
        30,
    )])]);

    assert!(linked_partner_ids(&timeline, "solo").is_empty());
    assert!(linked_partner_ids(&timeline, "ghost").is_empty());
}

#[test]
fn lnk_004_partner_moves_propagate_delta_and_preserve_partner_tracks() {
    let mut video = clip("v", ClipType::Video, 100, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 110, 30);
    audio.link_group_id = Some("g1".to_string());
    let timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);

    let moves = partner_moves_for_move_of(&timeline, "v", 80);
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].clip_id, "a");
    assert_eq!(moves[0].track_index, 1);
    assert_eq!(moves[0].to_frame, 90);
}

#[test]
fn lnk_004_partner_moves_clamp_negative_frames_to_zero() {
    let mut video = clip("v", ClipType::Video, 100, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 10, 30);
    audio.link_group_id = Some("g1".to_string());
    let timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);

    let moves = partner_moves_for_move_of(&timeline, "v", 0);
    assert_eq!(moves[0].to_frame, 0);
}

#[test]
fn lnk_006_link_group_offsets_use_start_minus_trim_start() {
    let mut video = clip("v", ClipType::Video, 100, 30);
    video.link_group_id = Some("g1".to_string());
    video.trim_start_frame = 20;
    let mut audio = clip("a", ClipType::Audio, 110, 30);
    audio.link_group_id = Some("g1".to_string());
    audio.trim_start_frame = 20;
    let timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);

    let offsets = link_group_offsets(&timeline);
    assert_eq!(offsets.get("v"), None);
    assert_eq!(offsets.get("a"), Some(&10));
}

#[test]
fn lnk_006_link_group_offsets_ignore_singletons() {
    let mut solo = clip("solo", ClipType::Video, 0, 30);
    solo.link_group_id = Some("g1".to_string());
    let timeline = timeline(vec![video_track(vec![solo])]);

    assert!(link_group_offsets(&timeline).is_empty());
}

#[test]
fn lnk_007_link_clips_stamps_shared_group_on_two_or_more_ids() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Audio, 0, 30);
    let mut timeline = timeline(vec![video_track(vec![c1]), audio_track(vec![c2])]);
    let ids = BTreeSet::from(["c1".to_string(), "c2".to_string()]);

    let group_id = link_clips(&mut timeline, &ids);
    assert!(group_id.is_some());

    let groups: Vec<String> = timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .filter_map(|clip| clip.link_group_id.clone())
        .collect();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0], groups[1]);
}

#[test]
fn lnk_007_link_clips_requires_at_least_two_ids() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let mut timeline = timeline(vec![video_track(vec![c1])]);
    let ids = BTreeSet::from(["c1".to_string()]);

    assert!(link_clips(&mut timeline, &ids).is_none());
    assert_eq!(timeline.tracks[0].clips[0].link_group_id, None);
}

#[test]
fn lnk_008_unlink_clips_clears_group_across_expanded_selection() {
    let mut video = clip("v", ClipType::Video, 0, 30);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 0, 30);
    audio.link_group_id = Some("g1".to_string());
    let mut timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);
    let ids = BTreeSet::from(["v".to_string()]);

    let cleared = unlink_clips(&mut timeline, &ids);
    assert_eq!(cleared, BTreeSet::from(["a".to_string(), "v".to_string()]));
    assert_eq!(timeline.tracks[0].clips[0].link_group_id, None);
    assert_eq!(timeline.tracks[1].clips[0].link_group_id, None);
}

#[test]
fn rpl_013_gap_is_still_empty_rejects_stale_filled_gap() {
    let track = video_track(vec![
        clip("c1", ClipType::Video, 0, 50),
        clip("c2", ClipType::Video, 60, 30),
        clip("c3", ClipType::Video, 100, 50),
    ]);

    assert!(!gap_is_still_empty(
        &track,
        FrameRange {
            start: 50,
            end: 100,
        },
    ));
}
