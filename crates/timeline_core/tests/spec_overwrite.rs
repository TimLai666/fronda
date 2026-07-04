use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use timeline_core::{clear_region, compute_overwrite, OverwriteAction};

fn clip(
    id: &str,
    start_frame: i64,
    duration_frames: i64,
    trim_start_frame: i64,
    speed: f64,
) -> Clip {
    Clip {
        id: id.to_string(),
        media_ref: format!("asset-{id}"),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame,
        duration_frames,
        trim_start_frame,
        trim_end_frame: 0,
        speed,
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

fn timeline(clips: Vec<Clip>) -> Timeline {
    Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![Track {
            id: "video-track".to_string(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            clips,
        }],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    }
}

#[test]
fn clp_006_compute_overwrite_handles_remove_and_trim_branches() {
    let inside = clip("inside", 60, 30, 0, 1.0);
    let left_overlap = clip("left", 0, 60, 0, 1.0);
    let right_overlap = clip("right", 100, 200, 0, 1.0);
    let actions = compute_overwrite(&[inside, left_overlap, right_overlap], 50, 150);

    assert_eq!(actions.len(), 3);
    assert!(matches!(actions[0], OverwriteAction::Remove { .. }));
    assert!(matches!(actions[1], OverwriteAction::TrimEnd { .. }));
    assert!(matches!(actions[2], OverwriteAction::TrimStart { .. }));
}

#[test]
fn clp_006_compute_overwrite_split_respects_speed_and_trim_start() {
    let clip = clip("c1", 0, 200, 10, 2.0);
    let actions = compute_overwrite(&[clip], 50, 150);

    match &actions[0] {
        OverwriteAction::Split {
            clip_id,
            left_duration,
            right_start_frame,
            right_trim_start,
            right_duration,
            ..
        } => {
            assert_eq!(clip_id, "c1");
            assert_eq!(*left_duration, 50);
            assert_eq!(*right_start_frame, 150);
            assert_eq!(*right_trim_start, 310);
            assert_eq!(*right_duration, 50);
        }
        other => panic!("expected split action, got {other:?}"),
    }
}

#[test]
fn clp_006_compute_overwrite_adjacent_edges_do_not_trigger() {
    let left = clip("left", 0, 50, 0, 1.0);
    let right = clip("right", 150, 50, 0, 1.0);

    let actions = compute_overwrite(&[left, right], 50, 150);
    assert!(actions.is_empty());
}

#[test]
fn clp_006_clear_region_removes_clip_fully_inside() {
    let inside = clip("inside", 50, 30, 0, 1.0);
    let mut timeline = timeline(vec![inside]);

    clear_region(&mut timeline, 0, 0, 100, true);
    assert!(timeline.tracks.is_empty());
}

#[test]
fn clp_006_clear_region_trims_left_overlapper() {
    let clip = clip("c1", 0, 100, 0, 1.0);
    let mut timeline = timeline(vec![clip]);

    clear_region(&mut timeline, 0, 50, 200, true);
    let remaining = &timeline.tracks[0].clips[0];

    assert_eq!(remaining.start_frame, 0);
    assert_eq!(remaining.duration_frames, 50);
}

#[test]
fn clp_006_clear_region_trims_right_overlapper() {
    let clip = clip("c1", 100, 100, 0, 1.0);
    let mut timeline = timeline(vec![clip]);

    clear_region(&mut timeline, 0, 0, 150, true);
    let remaining = &timeline.tracks[0].clips[0];

    assert_eq!(remaining.start_frame, 150);
    assert_eq!(remaining.duration_frames, 50);
}

#[test]
fn clp_006_clear_region_splits_enveloping_clip_and_removes_middle() {
    let clip = clip("c1", 0, 200, 0, 1.0);
    let mut timeline = timeline(vec![clip]);

    clear_region(&mut timeline, 0, 50, 150, true);
    let clips = &timeline.tracks[0].clips;

    assert_eq!(clips.len(), 2);
    assert_eq!(clips[0].start_frame, 0);
    assert_eq!(clips[0].duration_frames, 50);
    assert_eq!(clips[1].start_frame, 150);
    assert_eq!(clips[1].duration_frames, 50);
    assert!(clips
        .iter()
        .all(|clip| clip.start_frame >= 150 || clip.start_frame + clip.duration_frames <= 50));
}

#[test]
fn clp_006_clear_region_leaves_adjacent_clip_untouched() {
    let clip = clip("c1", 100, 30, 0, 1.0);
    let mut timeline = timeline(vec![clip]);

    clear_region(&mut timeline, 0, 0, 100, true);
    let remaining = &timeline.tracks[0].clips[0];

    assert_eq!(remaining.start_frame, 100);
    assert_eq!(remaining.duration_frames, 30);
}

#[test]
fn clp_006_clear_region_split_does_not_touch_linked_partner_track() {
    // Video v=[0,200) linked (g1) to audio a=[0,200) on a second track. Clearing
    // [50,150) on the video track must split ONLY the video and leave the audio
    // partner untouched. Regression: the Split branch used the link-aware
    // split_clip, which split (and orphaned) the audio clip on track 1.
    let mut v = clip("v", 0, 200, 0, 1.0);
    v.link_group_id = Some("g1".to_string());
    let mut a = clip("a", 0, 200, 0, 1.0);
    a.media_type = ClipType::Audio;
    a.source_clip_type = ClipType::Audio;
    a.link_group_id = Some("g1".to_string());

    let mut tl = Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks: vec![
            Track {
                id: "v".to_string(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips: vec![v],
            },
            Track {
                id: "a".to_string(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips: vec![a],
            },
        ],
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    clear_region(&mut tl, 0, 50, 150, false);

    // Audio track (index 1) must be untouched: a single [0,200) clip.
    assert_eq!(
        tl.tracks[1].clips.len(),
        1,
        "linked audio partner must not be split: {:?}",
        tl.tracks[1].clips.iter().map(|c| (c.start_frame, c.duration_frames)).collect::<Vec<_>>()
    );
    assert_eq!(tl.tracks[1].clips[0].start_frame, 0);
    assert_eq!(tl.tracks[1].clips[0].duration_frames, 200);

    // Video track: the middle [50,150) cleared → fragments [0,50) and [150,200).
    let mut vspans: Vec<(i64, i64)> = tl.tracks[0]
        .clips
        .iter()
        .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
        .collect();
    vspans.sort();
    assert_eq!(vspans, vec![(0, 50), (150, 200)]);
}
