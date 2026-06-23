use core_model::{
    AnimPair, Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Timeline, Track,
    Transform,
};
use timeline_core::{
    apply_clip_speed, clear_region, link_audio_for_placed_clips, move_clips, place_clips,
    split_clip,
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
    }
}

#[test]
fn clp_015_apply_clip_speed_recomputes_duration_from_source_coverage() {
    let clip = clip("c1", ClipType::Video, 0, 60);
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    assert!(apply_clip_speed(&mut timeline, "c1", 2.0));
    let updated = &timeline.tracks[0].clips[0];

    assert_eq!(updated.speed, 2.0);
    assert_eq!(updated.duration_frames, 30);
}

#[test]
fn clp_015_apply_clip_speed_can_expand_duration() {
    let clip = clip("c1", ClipType::Video, 0, 60);
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    assert!(apply_clip_speed(&mut timeline, "c1", 0.5));
    assert_eq!(timeline.tracks[0].clips[0].duration_frames, 120);
}

#[test]
fn clp_016_apply_clip_speed_ripples_contiguous_same_track_followers() {
    let c1 = clip("c1", ClipType::Video, 0, 60);
    let c2 = clip("c2", ClipType::Video, 60, 30);
    let mut timeline = timeline(vec![video_track(vec![c1, c2])]);

    assert!(apply_clip_speed(&mut timeline, "c1", 2.0));
    let updated = &timeline.tracks[0].clips;

    assert_eq!(updated[0].duration_frames, 30);
    assert_eq!(updated[1].start_frame, 30);
}

#[test]
fn clp_016_apply_clip_speed_does_not_ripple_non_contiguous_followers() {
    let c1 = clip("c1", ClipType::Video, 0, 60);
    let c2 = clip("c2", ClipType::Video, 100, 30);
    let mut timeline = timeline(vec![video_track(vec![c1, c2])]);

    assert!(apply_clip_speed(&mut timeline, "c1", 2.0));
    let updated = timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.id == "c2")
        .unwrap();

    assert_eq!(updated.start_frame, 100);
}

#[test]
fn clp_017_apply_clip_speed_clamps_fades_and_keyframes_to_new_duration() {
    let mut clip = clip("c1", ClipType::Video, 0, 60);
    clip.fade_in_frames = 20;
    clip.fade_out_frames = 20;
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 15,
                value: 0.4,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 45,
                value: 0.9,
                interpolation_out: Interpolation::Smooth,
            },
        ],
    });
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    assert!(apply_clip_speed(&mut timeline, "c1", 2.0));
    let updated = &timeline.tracks[0].clips[0];

    assert_eq!(updated.duration_frames, 30);
    assert_eq!(updated.fade_in_frames, 20);
    assert_eq!(updated.fade_out_frames, 10);
    assert_eq!(
        updated
            .opacity_track
            .as_ref()
            .unwrap()
            .keyframes
            .iter()
            .map(|keyframe| keyframe.frame)
            .collect::<Vec<_>>(),
        vec![15]
    );
}

#[test]
fn clp_009_split_clip_divides_at_frame_and_returns_right_half_id() {
    let clip = clip("c1", ClipType::Video, 0, 60);
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    let right_ids = split_clip(&mut timeline, "c1", 30);
    let clips = &timeline.tracks[0].clips;

    assert_eq!(right_ids.len(), 1);
    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0].duration_frames, 30);
    assert_eq!(clips[1].duration_frames, 30);
    assert_eq!(clips[1].id, right_ids[0]);
}

#[test]
fn clp_009_split_clip_returns_empty_for_unknown_or_boundary_frame() {
    let clip = clip("c1", ClipType::Video, 0, 60);
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    assert!(split_clip(&mut timeline, "ghost", 10).is_empty());
    assert!(split_clip(&mut timeline, "c1", 0).is_empty());
    assert!(split_clip(&mut timeline, "c1", 60).is_empty());
    assert_eq!(timeline.tracks[0].clips.len(), 1);
}

#[test]
fn clp_010_and_clp_011_split_linked_partners_and_regroup_right_halves() {
    let mut video = clip("v", ClipType::Video, 0, 60);
    video.link_group_id = Some("g1".to_string());
    let mut audio = clip("a", ClipType::Audio, 0, 60);
    audio.link_group_id = Some("g1".to_string());
    let mut timeline = timeline(vec![video_track(vec![video]), audio_track(vec![audio])]);

    let right_ids: std::collections::BTreeSet<String> =
        split_clip(&mut timeline, "v", 30).into_iter().collect();
    let all_clips: Vec<&Clip> = timeline
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .collect();

    assert_eq!(right_ids.len(), 2);

    let right_groups: std::collections::BTreeSet<String> = all_clips
        .iter()
        .filter(|clip| right_ids.contains(&clip.id))
        .filter_map(|clip| clip.link_group_id.clone())
        .collect();
    assert_eq!(right_groups.len(), 1);
    assert_ne!(right_groups.first().map(String::as_str), Some("g1"));

    let left_groups: std::collections::BTreeSet<String> = all_clips
        .iter()
        .filter(|clip| clip.id == "v" || clip.id == "a")
        .filter_map(|clip| clip.link_group_id.clone())
        .collect();
    assert_eq!(
        left_groups,
        std::collections::BTreeSet::from(["g1".to_string()])
    );
}

#[test]
fn clp_012_split_inserts_boundary_keyframes_and_rebases_right_half() {
    let mut clip = clip("c1", ClipType::Video, 0, 60);
    clip.opacity_track = Some(KeyframeTrack {
        keyframes: vec![
            Keyframe {
                frame: 0,
                value: 0.0,
                interpolation_out: Interpolation::Linear,
            },
            Keyframe {
                frame: 60,
                value: 1.0,
                interpolation_out: Interpolation::Linear,
            },
        ],
    });
    clip.position_track = Some(KeyframeTrack {
        keyframes: vec![Keyframe {
            frame: 60,
            value: AnimPair { a: 0.2, b: 0.4 },
            interpolation_out: Interpolation::Smooth,
        }],
    });
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    let right_ids = split_clip(&mut timeline, "c1", 30);
    let left = &timeline.tracks[0].clips[0];
    let right = timeline.tracks[0]
        .clips
        .iter()
        .find(|clip| clip.id == right_ids[0])
        .unwrap();

    assert_eq!(
        left.opacity_track
            .as_ref()
            .unwrap()
            .keyframes
            .iter()
            .map(|kf| kf.frame)
            .collect::<Vec<_>>(),
        vec![0, 30]
    );
    assert_eq!(
        right
            .opacity_track
            .as_ref()
            .unwrap()
            .keyframes
            .iter()
            .map(|kf| kf.frame)
            .collect::<Vec<_>>(),
        vec![0, 30]
    );
    assert_eq!(left.opacity_track.as_ref().unwrap().keyframes[1].value, 0.5);
    assert_eq!(
        right.opacity_track.as_ref().unwrap().keyframes[0].value,
        0.5
    );
    assert_eq!(right.position_track.as_ref().unwrap().keyframes[0].frame, 0);
}

#[test]
fn clp_013_split_resets_fades_across_cut() {
    let mut clip = clip("c1", ClipType::Video, 0, 60);
    clip.fade_in_frames = 15;
    clip.fade_out_frames = 20;
    let mut timeline = timeline(vec![video_track(vec![clip])]);

    let _ = split_clip(&mut timeline, "c1", 30);
    let halves = &timeline.tracks[0].clips;

    assert_eq!(halves.len(), 2);
    assert_eq!(halves[0].fade_in_frames, 15);
    assert_eq!(halves[0].fade_out_frames, 0);
    assert_eq!(halves[1].fade_in_frames, 0);
    assert_eq!(halves[1].fade_out_frames, 20);
}

#[test]
fn clp_014_remove_clips_clears_stale_selected_clip_ids() {
    use std::collections::HashSet;
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);
    t.selected_clip_ids = HashSet::from(["c1".to_string(), "c2".to_string()]);

    // remove_clips is called indirectly through clear_region
    clear_region(&mut t, 0, 0, 60, false);

    assert!(
        t.selected_clip_ids.is_empty(),
        "stale ids after region clear"
    );
}

#[test]
fn clp_014_remove_clips_keeps_unrelated_selected_ids() {
    use std::collections::HashSet;
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);
    t.selected_clip_ids = HashSet::from(["c2".to_string(), "unrelated".to_string()]);

    clear_region(&mut t, 0, 0, 30, false);

    assert!(t.selected_clip_ids.contains("c2"), "c2 should remain");
    assert!(
        t.selected_clip_ids.contains("unrelated"),
        "unrelated id should remain"
    );
    assert_eq!(t.selected_clip_ids.len(), 2);
}

// ─── CLP-001/002: place_clips overwrite semantics ───

#[test]
fn clp_001_place_clips_uses_overwrite_semantics() {
    let existing = clip("existing", ClipType::Video, 10, 30);
    let new_clip = clip("new", ClipType::Video, 0, 20);
    let mut t = timeline(vec![video_track(vec![existing])]);

    let placed = place_clips(&mut t, 0, 0, &[new_clip]);

    assert_eq!(placed.len(), 1);
    assert_eq!(
        t.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == placed[0])
            .map(|c| c.start_frame),
        Some(0)
    );
    let existing_after = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == "existing")
        .unwrap();
    assert_eq!(existing_after.start_frame, 20);
    assert_eq!(existing_after.duration_frames, 20);
}

#[test]
fn clp_002_place_clips_clears_destination() {
    let c1 = clip("c1", ClipType::Video, 0, 50);
    let c2 = clip("c2", ClipType::Video, 50, 50);
    let new_clip = clip("new", ClipType::Video, 0, 100);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);

    let placed = place_clips(&mut t, 0, 0, &[new_clip]);

    assert_eq!(placed.len(), 1);
    assert!(t.tracks[0].clips.iter().all(|c| c.id == placed[0]));
}

#[test]
fn clp_001_place_clips_returns_empty_for_bad_track() {
    let c = clip("c1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![video_track(vec![])]);
    assert!(place_clips(&mut t, 99, 0, &[c]).is_empty());
}

#[test]
fn clp_001_place_clips_returns_empty_for_empty_slice() {
    let mut t = timeline(vec![video_track(vec![])]);
    assert!(place_clips(&mut t, 0, 0, &[]).is_empty());
}

// ─── CLP-003/004/005: move_clips ───

#[test]
fn clp_003_move_clips_removes_from_source() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);

    let placed = move_clips(&mut t, &["c1".to_string()], 0, 100);

    assert_eq!(placed.len(), 1);
    assert!(t.tracks[0].clips.iter().all(|c| c.id != "c1"));
    assert!(t.tracks[0]
        .clips
        .iter()
        .any(|c| c.id == "c2" && c.start_frame == 30));
}

#[test]
fn clp_004_move_clips_inserts_at_target() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);

    let placed = move_clips(&mut t, &["c1".to_string()], 0, 50);

    assert_eq!(placed.len(), 1);
    let moved = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == placed[0])
        .unwrap();
    assert_eq!(moved.start_frame, 50);
    assert_eq!(moved.duration_frames, 30);
    assert!(t.tracks[0].clips.iter().any(|c| c.id == "c2"));
}

#[test]
fn clp_004_move_clips_multiple_with_spacing() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 20);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);

    let placed = move_clips(&mut t, &["c1".to_string(), "c2".to_string()], 0, 100);

    assert_eq!(placed.len(), 2);
    let p1 = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == placed[0])
        .unwrap();
    let p2 = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == placed[1])
        .unwrap();
    assert_eq!(p1.start_frame, 100);
    assert_eq!(p1.duration_frames, 30);
    assert_eq!(p2.start_frame, 130);
    assert_eq!(p2.duration_frames, 20);
}

#[test]
fn clp_005_move_clips_enforces_track_compatibility() {
    let audio = clip("a1", ClipType::Audio, 0, 30);
    let mut t = timeline(vec![video_track(vec![]), audio_track(vec![audio])]);

    let result = move_clips(&mut t, &["a1".to_string()], 0, 0);
    assert!(result.is_empty(), "audio to video should be refused");
}

#[test]
fn clp_005_move_clips_allows_video_to_video() {
    let video = clip("v1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![video_track(vec![video]), audio_track(vec![])]);

    let result = move_clips(&mut t, &["v1".to_string()], 0, 50);
    assert!(!result.is_empty());
}

#[test]
fn clp_005_move_clips_allows_audio_to_audio() {
    let audio = clip("a1", ClipType::Audio, 0, 30);
    let mut t = timeline(vec![video_track(vec![]), audio_track(vec![audio])]);

    let result = move_clips(&mut t, &["a1".to_string()], 1, 50);
    assert!(!result.is_empty());
}

#[test]
fn clp_004_move_clips_cross_track() {
    let v1 = clip("v1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![video_track(vec![v1]), video_track(vec![])]);

    let placed = move_clips(&mut t, &["v1".to_string()], 1, 10);
    assert!(!placed.is_empty());
    assert!(!t.tracks[0].clips.iter().any(|c| c.id == "v1"));
    assert!(t.tracks[1].clips.iter().any(|c| c.id == placed[0]));
}

#[test]
fn clp_004_move_clips_overwrite_at_target() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 20, 30);
    let c3 = clip("c3", ClipType::Video, 100, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2, c3])]);

    let placed = move_clips(&mut t, &["c1".to_string()], 0, 20);
    assert_eq!(placed.len(), 1);
    let moved = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == placed[0])
        .unwrap();
    assert_eq!(moved.start_frame, 20);
    assert!(t.tracks[0].clips.iter().all(|c| c.id != "c2"));
}

#[test]
fn clp_003_move_clips_same_track_no_duplicate() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let c2 = clip("c2", ClipType::Video, 30, 30);
    let mut t = timeline(vec![video_track(vec![c1, c2])]);

    let placed = move_clips(&mut t, &["c1".to_string()], 0, 50);
    assert_eq!(placed.len(), 1);
    let moved = t.tracks[0]
        .clips
        .iter()
        .find(|c| c.id == placed[0])
        .unwrap();
    assert_eq!(moved.start_frame, 50);
    assert_eq!(t.tracks[0].clips.len(), 2);
}

#[test]
fn clp_003_move_clips_returns_empty_for_unknown_id() {
    let c1 = clip("c1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![video_track(vec![c1])]);
    assert!(move_clips(&mut t, &["ghost".to_string()], 0, 50).is_empty());
}

// ─── CLP-007/008: auto-linked audio ───

#[test]
fn clp_007_008_link_audio_creates_linked_audio_on_audio_track() {
    let video = clip("v1", ClipType::Video, 0, 60);
    let mut t = timeline(vec![video_track(vec![video]), audio_track(vec![])]);

    let audio_ids = link_audio_for_placed_clips(&mut t, &["v1".to_string()], 1);

    assert_eq!(audio_ids.len(), 1);
    let audio_id = &audio_ids[0];

    // Audio clip exists on track 1 with same position
    let audio = t.tracks[1]
        .clips
        .iter()
        .find(|c| c.id == *audio_id)
        .unwrap();
    assert_eq!(audio.start_frame, 0);
    assert_eq!(audio.duration_frames, 60);
    assert_eq!(audio.media_type, ClipType::Audio);

    // Both clips share a link_group_id
    let video_clip = &t.tracks[0].clips[0];
    assert!(video_clip.link_group_id.is_some());
    assert_eq!(video_clip.link_group_id, audio.link_group_id);
}

#[test]
fn clp_007_008_link_audio_returns_empty_for_nonexistent_video() {
    let mut t = timeline(vec![video_track(vec![]), audio_track(vec![])]);
    assert!(link_audio_for_placed_clips(&mut t, &["ghost".to_string()], 1).is_empty());
}

#[test]
fn clp_007_008_link_audio_returns_empty_for_bad_audio_track() {
    let video = clip("v1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![video_track(vec![video])]); // no audio track
    assert!(link_audio_for_placed_clips(&mut t, &["v1".to_string()], 1).is_empty());
}

#[test]
fn clp_007_008_link_audio_returns_empty_if_target_not_audio_type() {
    let video = clip("v1", ClipType::Video, 0, 30);
    let mut t = timeline(vec![
        video_track(vec![video]),
        video_track(vec![]), // second video track, not audio
    ]);
    assert!(link_audio_for_placed_clips(&mut t, &["v1".to_string()], 1).is_empty());
}

#[test]
fn clp_007_008_link_audio_overwrite_on_audio_track() {
    let video = clip("v1", ClipType::Video, 0, 60);
    let existing_audio = clip("existing", ClipType::Audio, 10, 80);
    let mut t = timeline(vec![
        video_track(vec![video]),
        audio_track(vec![existing_audio]),
    ]);

    let audio_ids = link_audio_for_placed_clips(&mut t, &["v1".to_string()], 1);

    assert_eq!(audio_ids.len(), 1);
    let new_audio = t.tracks[1]
        .clips
        .iter()
        .find(|c| c.id == audio_ids[0])
        .unwrap();
    assert_eq!(new_audio.start_frame, 0);
    // existing audio at frame 10-90 should be overwritten (trimmed right to start at 60)
    let existing = t.tracks[1]
        .clips
        .iter()
        .find(|c| c.id == "existing")
        .unwrap();
    assert_eq!(existing.start_frame, 60);
    assert_eq!(existing.duration_frames, 30);
}
