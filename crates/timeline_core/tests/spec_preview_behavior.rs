use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};
use timeline_core::TimelineMathExt;

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
    }
}

fn track(kind: ClipType, clips: Vec<Clip>) -> Track {
    Track {
        id: format!("{:?}-track", kind),
        r#type: kind,
        muted: false,
        hidden: false,
        sync_locked: true,
        clips,
    }
}

fn timeline_with_tracks(tracks: Vec<Track>) -> Timeline {
    Timeline {
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        tracks,
        transcription_language: None,
        compound_timelines: std::collections::HashMap::new(),
    }
}

// PRV-014: Starting playback from the end of the timeline rewinds to frame 0.
// TimelineMathExt::clamp_seek_frame ensures all seek positions stay within [0, total_frames].
// When the playhead is at or past the end, the caller is expected to rewind to 0;
// clamp_seek_frame provides the boundary that enables that check.
#[test]
fn prv_014_play_from_end_rewinds_to_zero() {
    // Timeline with clips ending at frame 200 (last clip at 150 + 50)
    let timeline = timeline_with_tracks(vec![track(
        ClipType::Video,
        vec![clip("c1", 0, 100), clip("c2", 150, 50)],
    )]);

    assert_eq!(timeline.total_frames(), 200);

    // Seeking to total_frames (at end) is clamped to total_frames
    assert_eq!(timeline.clamp_seek_frame(200), 200);

    // Seeking past end clamps to total_frames
    assert_eq!(timeline.clamp_seek_frame(500), 200);

    // Seeking to frame 0 stays at 0
    assert_eq!(timeline.clamp_seek_frame(0), 0);

    // Seeking past start clamps to 0
    assert_eq!(timeline.clamp_seek_frame(-10), 0);

    // Normal seek within range passes through unchanged
    assert_eq!(timeline.clamp_seek_frame(50), 50);
}

#[test]
fn prv_014_empty_timeline_clamps_to_zero() {
    // A timeline with no clips has total_frames = 0
    let timeline = timeline_with_tracks(vec![]);

    assert_eq!(timeline.total_frames(), 0);

    // Any seek in an empty timeline clamps to 0
    assert_eq!(timeline.clamp_seek_frame(0), 0);
    assert_eq!(timeline.clamp_seek_frame(100), 0);
    assert_eq!(timeline.clamp_seek_frame(-5), 0);
}

#[test]
fn prv_014_seek_exactly_at_zero() {
    // Seeking to frame 0 on a non-empty timeline is always valid
    let timeline = timeline_with_tracks(vec![track(ClipType::Video, vec![clip("c1", 100, 50)])]);

    assert_eq!(timeline.total_frames(), 150);
    assert_eq!(timeline.clamp_seek_frame(0), 0);
}

#[test]
fn prv_014_seek_one_before_end() {
    // Seeking to total_frames - 1 is valid and passed through
    let timeline = timeline_with_tracks(vec![track(
        ClipType::Video,
        vec![clip("c1", 0, 100), clip("c2", 200, 50)],
    )]);

    assert_eq!(timeline.total_frames(), 250);
    assert_eq!(timeline.clamp_seek_frame(249), 249);
}
