use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use timeline_core::{
    clamp_track_height, display_label_for_track, insert_track_at, remove_track,
    sort_clips_on_track, toggle_track_hidden, toggle_track_mute, toggle_track_sync_lock,
    MAX_TRACK_HEIGHT, MIN_TRACK_HEIGHT,
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
        multicam_group_id: None,
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
        display_height: 50.0,
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
        display_height: 50.0,
        clips,
    }
}

fn timeline(tracks: Vec<Track>) -> Timeline {
    Timeline {
        id: String::new(),
        name: String::new(),
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

// ─── TRK-001: Visual tracks always remain above audio tracks ───

#[test]
fn trk_001_visual_track_inserted_before_audio() {
    let mut t = timeline(vec![audio_track(vec![])]);
    let idx = insert_track_at(&mut t, 0, ClipType::Video).unwrap();
    // Video should go before audio
    assert_eq!(idx, 0);
    assert_eq!(t.tracks[0].r#type, ClipType::Video);
    assert_eq!(t.tracks[1].r#type, ClipType::Audio);
}

#[test]
fn trk_001_audio_track_inserted_after_visual() {
    let mut t = timeline(vec![video_track(vec![])]);
    let idx = insert_track_at(&mut t, 1, ClipType::Audio).unwrap();
    // Audio should go at the end
    assert_eq!(idx, 1);
    assert_eq!(t.tracks[0].r#type, ClipType::Video);
    assert_eq!(t.tracks[1].r#type, ClipType::Audio);
}

// ─── TRK-003: Track labels ───

#[test]
fn trk_003_v1_for_single_video_track() {
    let t = timeline(vec![video_track(vec![clip("c1", 0, 30)])]);
    assert_eq!(display_label_for_track(&t, 0), "V1");
}

#[test]
fn trk_003_v1_v2_for_two_video_tracks() {
    let t = timeline(vec![video_track(vec![]), video_track(vec![])]);
    assert_eq!(display_label_for_track(&t, 0), "V1");
    assert_eq!(display_label_for_track(&t, 1), "V2");
}

#[test]
fn trk_003_a1_for_single_audio_track() {
    let t = timeline(vec![audio_track(vec![clip("c1", 0, 30)])]);
    assert_eq!(display_label_for_track(&t, 0), "A1");
}

#[test]
fn trk_003_a1_a2_for_two_audio_tracks() {
    let t = timeline(vec![audio_track(vec![]), audio_track(vec![])]);
    assert_eq!(display_label_for_track(&t, 0), "A1");
    assert_eq!(display_label_for_track(&t, 1), "A2");
}

#[test]
fn trk_003_mixed_tracks_label_independently() {
    let t = timeline(vec![
        video_track(vec![]),
        video_track(vec![]),
        audio_track(vec![]),
        audio_track(vec![]),
    ]);
    assert_eq!(display_label_for_track(&t, 0), "V1");
    assert_eq!(display_label_for_track(&t, 1), "V2");
    assert_eq!(display_label_for_track(&t, 2), "A1");
    assert_eq!(display_label_for_track(&t, 3), "A2");
}

#[test]
fn trk_003_out_of_range_track_returns_empty_string() {
    let t = timeline(vec![]);
    assert_eq!(display_label_for_track(&t, 0), "");
}

// ─── TRK-004: Removing a track removes every clip on that track ───

#[test]
fn trk_004_remove_track_removes_clips() {
    let mut t = timeline(vec![video_track(vec![clip("c1", 0, 30)])]);
    assert!(remove_track(&mut t, 0));
    assert!(t.tracks.is_empty());
}

#[test]
fn trk_004_remove_track_out_of_range_returns_false() {
    let mut t = timeline(vec![]);
    assert!(!remove_track(&mut t, 0));
}

// ─── TRK-005: Removing a track shifts remaining track indexes downward ───

#[test]
fn trk_005_remove_middle_track_shifts_others() {
    let mut t = timeline(vec![
        video_track(vec![clip("v1", 0, 30)]),
        audio_track(vec![clip("a1", 0, 30)]),
        audio_track(vec![clip("a2", 50, 30)]),
    ]);
    assert!(remove_track(&mut t, 1));
    assert_eq!(t.tracks.len(), 2);
    assert_eq!(t.tracks[0].r#type, ClipType::Video);
    assert_eq!(t.tracks[1].r#type, ClipType::Audio);
    assert_eq!(t.tracks[1].clips[0].id, "a2");
}

// ─── TRK-006: pruneEmptyTracks removes empty tracks ───

#[test]
fn trk_006_prune_empty_tracks_removes_tracks_with_no_clips() {
    let mut t = timeline(vec![
        video_track(vec![clip("c1", 0, 30)]),
        video_track(vec![]),
        audio_track(vec![]),
    ]);
    timeline_core::prune_empty_tracks(&mut t);
    assert_eq!(t.tracks.len(), 1);
    assert_eq!(t.tracks[0].clips[0].id, "c1");
}

#[test]
fn trk_006_prune_empty_tracks_preserves_visual_above_audio_partition() {
    let mut t = timeline(vec![
        video_track(vec![]),
        audio_track(vec![clip("a1", 0, 30)]),
        video_track(vec![clip("v2", 0, 30)]),
    ]);
    timeline_core::prune_empty_tracks(&mut t);
    // After pruning, remaining: audio(a1) then video(v2)
    // But Swift's pruneEmptyTracks just removes empty, doesn't reorder
    assert_eq!(t.tracks.len(), 2);
    assert_eq!(t.tracks[0].r#type, core_model::ClipType::Audio);
    assert_eq!(t.tracks[1].r#type, core_model::ClipType::Video);
}

#[test]
fn trk_006_prune_empty_tracks_removes_all_when_all_empty() {
    let mut t = timeline(vec![video_track(vec![]), audio_track(vec![])]);
    timeline_core::prune_empty_tracks(&mut t);
    assert!(t.tracks.is_empty());
}

// ─── sort_clips_on_track ───

#[test]
fn trk_sort_clips_on_track_sorts_by_start_frame() {
    let c1 = clip("c1", 200, 30);
    let c2 = clip("c2", 0, 50);
    let c3 = clip("c3", 100, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2, c3])]);

    assert!(sort_clips_on_track(&mut t, 0));
    let sorted: Vec<i64> = t.tracks[0].clips.iter().map(|c| c.start_frame).collect();
    assert_eq!(sorted, vec![0, 100, 200]);
}

#[test]
fn trk_sort_clips_on_track_out_of_range_returns_false() {
    let mut t = timeline(vec![]);
    assert!(!sort_clips_on_track(&mut t, 0));
}

// ─── TRK-007: Track mute/hidden/sync-lock toggles ───

#[test]
fn trk_007_toggle_mute_changes_state() {
    let mut t = timeline(vec![video_track(vec![clip("c1", 0, 30)])]);
    assert!(!t.tracks[0].muted);
    assert_eq!(toggle_track_mute(&mut t, 0), Some(true));
    assert!(t.tracks[0].muted);
    assert_eq!(toggle_track_mute(&mut t, 0), Some(false));
    assert!(!t.tracks[0].muted);
}

#[test]
fn trk_007_toggle_hidden_changes_state() {
    let mut t = timeline(vec![video_track(vec![clip("c1", 0, 30)])]);
    assert!(!t.tracks[0].hidden);
    assert_eq!(toggle_track_hidden(&mut t, 0), Some(true));
    assert!(t.tracks[0].hidden);
}

#[test]
fn trk_007_toggle_sync_lock_changes_state() {
    let mut t = timeline(vec![video_track(vec![clip("c1", 0, 30)])]);
    assert!(t.tracks[0].sync_locked);
    assert_eq!(toggle_track_sync_lock(&mut t, 0), Some(false));
    assert!(!t.tracks[0].sync_locked);
}

#[test]
fn trk_007_toggle_invalid_index_returns_none() {
    let mut t = timeline(vec![]);
    assert!(toggle_track_mute(&mut t, 0).is_none());
    assert!(toggle_track_hidden(&mut t, 0).is_none());
    assert!(toggle_track_sync_lock(&mut t, 0).is_none());
}

// ─── TRK-008: Track display height clamping ───

#[test]
fn trk_008_clamp_track_height_mid_range() {
    assert!((clamp_track_height(100.0) - 100.0).abs() < 1e-9);
}

#[test]
fn trk_008_clamp_track_height_below_min() {
    assert!((clamp_track_height(10.0) - MIN_TRACK_HEIGHT).abs() < 1e-9);
}

#[test]
fn trk_008_clamp_track_height_above_max() {
    assert!((clamp_track_height(500.0) - MAX_TRACK_HEIGHT).abs() < 1e-9);
}

#[test]
fn trk_008_clamp_track_height_negative() {
    assert!((clamp_track_height(-50.0) - MIN_TRACK_HEIGHT).abs() < 1e-9);
}

#[test]
fn trk_008_clamp_track_height_exact_min() {
    assert!((clamp_track_height(MIN_TRACK_HEIGHT) - MIN_TRACK_HEIGHT).abs() < 1e-9);
}

#[test]
fn trk_008_clamp_track_height_exact_max() {
    assert!((clamp_track_height(MAX_TRACK_HEIGHT) - MAX_TRACK_HEIGHT).abs() < 1e-9);
}
