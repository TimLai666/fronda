/// Preview and playback state management for the composition engine.
///
/// Corresponds to PRV-001-015 in the rewrite spec.
use crate::{CompositionPlan, CompositionTrack};
use core_model::{ClipType, Timeline};

// ---------------------------------------------------------------------------
// PRV-001 / PRV-002: Timeline vs source preview distinction
// ---------------------------------------------------------------------------

/// Whether a timeline has any visual composition content (non-hidden visual
/// tracks with at least one clip, or a black background that fills frame 0).
pub fn has_visual_composition_content(plan: &CompositionPlan) -> bool {
    plan.tracks
        .iter()
        .filter(|t| t.is_visual && !t.is_hidden)
        .any(|t| !t.clips.is_empty())
}

/// Whether a media type renders as a timeline composition (PRV-001) vs a
/// direct source preview (PRV-002).
pub fn is_timeline_preview_type(media_type: &ClipType) -> bool {
    matches!(media_type, ClipType::Video | ClipType::Audio)
}

// ---------------------------------------------------------------------------
// PRV-005: Offline / unprocessable media detection
// ---------------------------------------------------------------------------

/// Check whether the plan has any offline media refs.
pub fn has_offline_media(plan: &CompositionPlan) -> bool {
    !plan.offline_media_refs.is_empty()
}

/// Check whether the plan has any unprocessable media refs.
pub fn has_unprocessable_media(plan: &CompositionPlan) -> bool {
    !plan.unprocessable_media_refs.is_empty()
}

// ---------------------------------------------------------------------------
// PRV-006 / PRV-007: Hidden / muted tracks
// ---------------------------------------------------------------------------

/// Collect all visible visual tracks (excluding hidden ones).
pub fn visible_visual_tracks<'a>(plan: &'a CompositionPlan) -> Vec<&'a CompositionTrack> {
    plan.tracks
        .iter()
        .filter(|t| t.is_visual && !t.is_hidden)
        .collect()
}

/// Collect all unmuted audio tracks.
pub fn unmuted_audio_tracks<'a>(plan: &'a CompositionPlan) -> Vec<&'a CompositionTrack> {
    plan.tracks
        .iter()
        .filter(|t| !t.is_visual && !t.is_muted)
        .collect()
}

// ---------------------------------------------------------------------------
// PRV-014: Playhead seek on end-of-timeline
// ---------------------------------------------------------------------------

/// Determine the correct frame to seek to when starting playback.
///
/// PRV-014: If the playhead is at or past the end of the timeline,
/// rewind to frame 0 before playing.
pub fn seek_frame_for_playback(current_frame: i64, total_frames: i64) -> i64 {
    if current_frame >= total_frames {
        0
    } else {
        current_frame
    }
}

/// Determine the correct frame to seek to for a source preview tab.
/// Source preview always starts at 0.
pub fn seek_frame_for_source_preview() -> i64 {
    0
}

// ---------------------------------------------------------------------------
// PRV-015: Timescale-aware composition timing
// ---------------------------------------------------------------------------

/// Convert a source duration from a source's natural timescale to project frames.
///
/// PRV-015: Video-backed trim starts and durations must use the source track's
/// natural timescale, not blindly assume project fps.
///
/// `source_duration_in_project_frames` - the value currently stored (in project frames)
/// `source_timescale` - the source media's natural timescale (e.g. 60 fps → 60)
/// `project_fps` - the timeline's fps (e.g. 30)
pub fn convert_source_duration_to_project_frames(
    source_duration_in_project_frames: i64,
    source_timescale: i64,
    project_fps: i64,
) -> i64 {
    if source_timescale <= 0 || project_fps <= 0 {
        return source_duration_in_project_frames;
    }
    // Convert: project_frames * source_timescale / project_fps → source-native frames
    // But the stored value is already in "project frames" — the fix means we
    // need to reinterpret: the stored source_trim_start/duration were computed
    // using project fps, but should be converted to source timescale before
    // being used in AVFoundation composition.
    //
    // For the Rust data model, we provide the conversion factor so the platform
    // adapter can apply it.
    //
    // Ratio: source_timescale / project_fps
    // If source is 60fps, project is 30fps, the factor is 2.0.
    // So the "project frame" duration value corresponds to source_timescale/project_fps
    // times as many source frames.
    let result = (source_duration_in_project_frames as f64 * source_timescale as f64
        / project_fps as f64)
        .round() as i64;
    result.max(0)
}

// ---------------------------------------------------------------------------
// PRV-008: Text overlays are overlay-rendered
// ---------------------------------------------------------------------------

/// Whether a composition plan has any text overlay clips that must be rendered
/// separately from the main composition.
pub fn has_text_overlays(plan: &CompositionPlan) -> bool {
    plan.tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .any(|c| c.is_text_overlay)
}

/// Whether a composition plan has any still-image clips that need synthetic
/// video generation (PRV-012).
pub fn has_image_clips(plan: &CompositionPlan) -> bool {
    plan.tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .any(|c| c.is_image)
}

/// Whether a composition plan has any Lottie clips (PRV-013).
pub fn has_lottie_clips(plan: &CompositionPlan) -> bool {
    plan.tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .any(|c| c.is_lottie)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CompositionClip, CompositionPlan, CompositionTrack, RenderResolution};
    use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};

    fn make_timeline() -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
                clips: vec![Clip {
                    id: "clip1".into(),
                    media_ref: "asset-v".into(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    start_frame: 0,
                    duration_frames: 100,
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
                }],
            }],
        }
    }

    fn make_empty_timeline() -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks: Vec::new(),
        }
    }

    // PRV-001: Timeline preview has visual content.
    #[test]
    fn prv_001_timeline_preview_has_visual_content() {
        let timeline = make_timeline();
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(has_visual_composition_content(&plan));
    }

    #[test]
    fn prv_001_empty_timeline_no_visual_content() {
        let timeline = make_empty_timeline();
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(!has_visual_composition_content(&plan));
    }

    // PRV-002: Media-asset preview renders source directly.
    #[test]
    fn prv_002_source_preview_type() {
        assert!(is_timeline_preview_type(&ClipType::Video));
        assert!(is_timeline_preview_type(&ClipType::Audio));
        assert!(!is_timeline_preview_type(&ClipType::Image));
        assert!(!is_timeline_preview_type(&ClipType::Text));
        assert!(!is_timeline_preview_type(&ClipType::Lottie));
    }

    // PRV-003: Text overlays are detected.
    #[test]
    fn prv_003_text_overlays_detected() {
        let mut timeline = make_timeline();
        timeline.tracks[0].clips.push(Clip {
            id: "txt1".into(),
            media_ref: String::new(),
            media_type: ClipType::Text,
            source_clip_type: ClipType::Text,
            start_frame: 10,
            duration_frames: 50,
            ..timeline.tracks[0].clips[0].clone()
        });
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(has_text_overlays(&plan));
    }

    #[test]
    fn prv_003_no_text_overlays() {
        let timeline = make_timeline();
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(!has_text_overlays(&plan));
    }

    // PRV-004: Invalid timeline settings cause composition build failure.
    // Already tested in composition_validation_rejects_zero_fps and
    // composition_validation_rejects_tiny_resolution.

    // PRV-005: Offline media tracking.
    #[test]
    fn prv_005_offline_media_detection() {
        let timeline = make_timeline();
        let mut plan =
            CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(!has_offline_media(&plan));
        plan.offline_media_refs.push("missing".into());
        assert!(has_offline_media(&plan));
    }

    #[test]
    fn prv_005_unprocessable_media_detection() {
        let timeline = make_timeline();
        let mut plan =
            CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(!has_unprocessable_media(&plan));
        plan.unprocessable_media_refs.push("broken".into());
        assert!(has_unprocessable_media(&plan));
    }

    // PRV-006: Hidden visual tracks contribute no output.
    #[test]
    fn prv_006_hidden_track_excluded() {
        let timeline = make_timeline();
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert_eq!(visible_visual_tracks(&plan).len(), 1);

        let mut timeline2 = make_timeline();
        timeline2.tracks[0].hidden = true;
        let plan2 =
            CompositionPlan::from_timeline(&timeline2, RenderResolution::native(&timeline2));
        assert!(visible_visual_tracks(&plan2).is_empty());
    }

    // PRV-007: Muted audio tracks contribute zero output.
    #[test]
    fn prv_007_muted_track_excluded() {
        let mut timeline = make_timeline();
        timeline.tracks.push(Track {
            id: "a1".into(),
            r#type: ClipType::Audio,
            muted: false,
            hidden: false,
            sync_locked: true,
            clips: vec![Clip {
                id: "a1-clip".into(),
                media_ref: "asset-a".into(),
                media_type: ClipType::Audio,
                source_clip_type: ClipType::Audio,
                start_frame: 0,
                duration_frames: 100,
                ..timeline.tracks[0].clips[0].clone()
            }],
        });
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert_eq!(unmuted_audio_tracks(&plan).len(), 1);

        timeline.tracks[1].muted = true;
        let plan2 = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(unmuted_audio_tracks(&plan2).is_empty());
    }

    // PRV-008: Text clips are overlay-rendered (already encoded in is_text_overlay).
    #[test]
    fn prv_008_text_clip_is_overlay() {
        let clip = CompositionClip {
            clip_id: "txt1".into(),
            media_type: ClipType::Text,
            composition_start: 0,
            duration_frames: 50,
            source_trim_start: 0,
            source_trim_end: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            is_text_overlay: true,
            is_image: false,
            is_lottie: false,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
        };
        assert!(clip.is_text_overlay);
        // Non-text clips are not overlays
        let video_clip = CompositionClip {
            media_type: ClipType::Video,
            ..clip.clone()
        };
        assert!(!video_clip.is_text_overlay);
    }

    // PRV-009: Visual clips on same track are sorted and non-overlapping.
    // Already tested in composition_validation_warns_on_overlapping_visual_clips.

    // PRV-010 / PRV-011: Audio track allocation.
    // Already tested in audio_allocation_shared_at_normal_speed and
    // audio_allocation_variable_speed_gets_dedicated_track.

    // PRV-012: Still images are flagged as synthetic video.
    #[test]
    fn prv_012_image_clip_detection() {
        let mut timeline = make_timeline();
        timeline.tracks[0].clips.push(Clip {
            id: "img1".into(),
            media_ref: "asset-img".into(),
            media_type: ClipType::Image,
            source_clip_type: ClipType::Image,
            start_frame: 0,
            duration_frames: 50,
            ..timeline.tracks[0].clips[0].clone()
        });
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(has_image_clips(&plan));
        // The image clip should be in the general clips too
        assert!(plan
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .any(|c| c.is_image));
    }

    // PRV-013: Lottie assets are flagged.
    #[test]
    fn prv_013_lottie_clip_detection() {
        let mut timeline = make_timeline();
        timeline.tracks[0].clips.push(Clip {
            id: "lot1".into(),
            media_ref: "asset-lottie".into(),
            media_type: ClipType::Lottie,
            source_clip_type: ClipType::Lottie,
            start_frame: 0,
            duration_frames: 60,
            ..timeline.tracks[0].clips[0].clone()
        });
        let plan = CompositionPlan::from_timeline(&timeline, RenderResolution::native(&timeline));
        assert!(has_lottie_clips(&plan));
    }

    // PRV-014: Playback from end rewinds to 0.
    #[test]
    fn prv_014_seek_from_end_rewinds_to_zero() {
        assert_eq!(seek_frame_for_playback(100, 100), 0);
        assert_eq!(seek_frame_for_playback(101, 100), 0);
        assert_eq!(seek_frame_for_playback(200, 100), 0);
    }

    #[test]
    fn prv_014_seek_within_timeline_keeps_frame() {
        assert_eq!(seek_frame_for_playback(0, 100), 0);
        assert_eq!(seek_frame_for_playback(50, 100), 50);
        assert_eq!(seek_frame_for_playback(99, 100), 99);
    }

    #[test]
    fn prv_014_source_preview_always_at_zero() {
        assert_eq!(seek_frame_for_source_preview(), 0);
    }

    // PRV-015: Timescale-aware composition timing.
    #[test]
    fn prv_015_timescale_conversion_double_fps() {
        // Source at 60fps, project at 30fps → factor of 2
        let result = convert_source_duration_to_project_frames(100, 60, 30);
        assert_eq!(result, 200);
    }

    #[test]
    fn prv_015_timescale_conversion_same_fps() {
        // Source at 30fps, project at 30fps → factor of 1
        let result = convert_source_duration_to_project_frames(100, 30, 30);
        assert_eq!(result, 100);
    }

    #[test]
    fn prv_015_timescale_conversion_half_fps() {
        // Source at 30fps, project at 60fps → factor of 0.5
        let result = convert_source_duration_to_project_frames(100, 30, 60);
        assert_eq!(result, 50);
    }

    #[test]
    fn prv_015_timescale_conversion_zero_protection() {
        let result = convert_source_duration_to_project_frames(100, 0, 30);
        assert_eq!(result, 100);
    }
}
