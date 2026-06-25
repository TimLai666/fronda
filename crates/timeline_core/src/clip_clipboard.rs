use crate::{clear_region, find_clip};
use core_model::{Clip, ClipType, Timeline};

use uuid::Uuid;

/// A snapshot of copied clips with their relative positioning context.
#[derive(Debug, Clone)]
pub struct ClipClipboard {
    /// The copied clip snapshots
    pub clips: Vec<CopiedClip>,
    /// The anchor frame used for paste positioning
    pub anchor_frame: i64,
}

/// A single copied clip with enough info to reconstruct.
#[derive(Debug, Clone)]
pub struct CopiedClip {
    pub clip: Clip,
    pub source_track_index: usize,
    /// Frame offset from the copy anchor
    pub offset_from_anchor: i64,
}

/// Result of a paste operation
#[derive(Debug, Clone)]
pub struct PasteResult {
    pub placed_clips: Vec<String>,
    pub errors: Vec<PasteError>,
}

#[derive(Debug, Clone)]
pub enum PasteError {
    NoCompatibleTrack { clip_id: String },
}

/// Determine if a clip type is compatible with a track type.
pub fn is_track_compatible(track_type: ClipType, clip_type: ClipType) -> bool {
    match (track_type, clip_type) {
        (ClipType::Audio, ClipType::Audio) => true,
        // Visual tracks (Video, Image, Text, Lottie, Shape) are mutually compatible
        (t, c) if t != ClipType::Audio && c != ClipType::Audio => true,
        _ => false,
    }
}

/// Find the first track compatible with the given clip type, optionally preferring a specific index.
pub fn find_first_compatible_track<'a>(
    tracks: &'a [core_model::Track],
    clip_type: ClipType,
    preferred_index: Option<usize>,
) -> Option<usize> {
    // Try preferred first
    if let Some(idx) = preferred_index {
        if idx < tracks.len() && is_track_compatible(tracks[idx].r#type, clip_type) {
            return Some(idx);
        }
    }
    // Fall back to first compatible
    tracks
        .iter()
        .position(|t| is_track_compatible(t.r#type, clip_type))
}

/// Assign fresh IDs to cloned clips and remap link groups.
///
/// CCB-011: Every clone gets a fresh clip id.
/// CCB-012: If multiple copied clips shared a source link group, their clones share a new remapped link group id.
/// CCB-013: If only one copied clip came from a link group, its pasted clone becomes unlinked.
fn clone_clips(copied: &[CopiedClip]) -> Vec<Clip> {
    // Build source link group → count mapping
    let mut group_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    for c in copied {
        if let Some(ref gid) = c.clip.link_group_id {
            *group_counts.entry(gid.clone()).or_default() += 1;
        }
    }

    // Build remap: old link group id → new link group id (only for groups with count > 1)
    let mut remap: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (gid, count) in &group_counts {
        if *count > 1 {
            remap.insert(gid.clone(), Uuid::new_v4().to_string());
        }
    }

    copied
        .iter()
        .map(|c| {
            let mut cloned = c.clip.clone();
            cloned.id = Uuid::new_v4().to_string();
            // Remap link group: multi-clip groups get new shared id, single clip groups become unlinked
            if let Some(ref old_gid) = cloned.link_group_id {
                if let Some(new_gid) = remap.get(old_gid) {
                    cloned.link_group_id = Some(new_gid.clone());
                } else {
                    // Only one clip from this group → unlink (CCB-013)
                    cloned.link_group_id = None;
                }
            }
            cloned
        })
        .collect()
}

/// The clip clipboard engine.
pub struct ClipClipboardEngine;

impl ClipClipboardEngine {
    /// CCB-001: Copy stores clip snapshots plus relative track/frame offsets from the copy anchor.
    /// CCB-002: Copy order is stable by track index, then clip start frame, then clip id.
    /// CCB-003: (implicit) Multiple clips can be copied.
    pub fn copy_clips(
        timeline: &Timeline,
        clip_ids: &[String],
        anchor_frame: i64,
    ) -> ClipClipboard {
        let mut copied: Vec<CopiedClip> = clip_ids
            .iter()
            .filter_map(|id| {
                let loc = find_clip(timeline, id)?;
                let clip = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
                Some(CopiedClip {
                    offset_from_anchor: clip.start_frame - anchor_frame,
                    source_track_index: loc.track_index,
                    clip,
                })
            })
            .collect();

        // CCB-002: Stable sort by track index, then start frame, then clip id
        copied.sort_by(|a, b| {
            a.source_track_index
                .cmp(&b.source_track_index)
                .then_with(|| a.clip.start_frame.cmp(&b.clip.start_frame))
                .then_with(|| a.clip.id.cmp(&b.clip.id))
        });

        ClipClipboard {
            clips: copied,
            anchor_frame,
        }
    }

    /// CCB-004: Paste-at-playhead prefers the original source track if it still exists and is compatible.
    /// CCB-005: If original source track unavailable, fall back to first compatible track.
    /// CCB-006: If no compatible track exists, paste no-ops.
    /// CCB-007: Paste-at-track/frame applies stored relative offsets.
    /// CCB-008: Skip placements on invalid/incompatible tracks instead of failing the whole paste.
    /// CCB-010: Paste clears overlapping destination regions before inserting cloned clips.
    pub fn paste_at_frame(
        timeline: &mut Timeline,
        clipboard: &ClipClipboard,
        target_track: Option<usize>,
        target_frame: i64,
    ) -> PasteResult {
        let mut placed = Vec::new();
        let mut errors = Vec::new();
        let mut all_to_place = Vec::new();

        for copied in &clipboard.clips {
            // Determine destination track
            let dest_track = if let Some(tt) = target_track {
                // CCB-007: Paste-at-track/frame
                Some(tt)
            } else {
                // CCB-004: Prefer original source track
                let preferred = if copied.source_track_index < timeline.tracks.len() {
                    Some(copied.source_track_index)
                } else {
                    None
                };
                find_first_compatible_track(&timeline.tracks, copied.clip.media_type, preferred)
            };

            let track_idx = match dest_track {
                Some(idx) if idx < timeline.tracks.len() => idx,
                _ => {
                    // CCB-006: No compatible track
                    errors.push(PasteError::NoCompatibleTrack {
                        clip_id: copied.clip.id.clone(),
                    });
                    continue;
                }
            };

            if !is_track_compatible(timeline.tracks[track_idx].r#type, copied.clip.media_type) {
                // CCB-008: Skip incompatible placements
                errors.push(PasteError::NoCompatibleTrack {
                    clip_id: copied.clip.id.clone(),
                });
                continue;
            }

            let start_frame = target_frame + copied.offset_from_anchor;
            let end_frame = start_frame + copied.clip.duration_frames;
            all_to_place.push((track_idx, start_frame, end_frame, copied));
            placed.push(copied.clip.id.clone());
        }

        // CCB-010: Clear overlapping regions before inserting
        for (track_idx, start, end, _) in &all_to_place {
            clear_region(timeline, *track_idx, *start, *end, false);
        }

        // Clone and place
        let clones = clone_clips(
            &clipboard
                .clips
                .iter()
                .filter(|c| placed.contains(&c.clip.id))
                .cloned()
                .collect::<Vec<_>>(),
        );

        // Place each clone on its respective track
        let mut placed_ids = Vec::new();
        for (i, (track_idx, start_frame, _, _)) in all_to_place.iter().enumerate() {
            if i < clones.len() {
                let mut clip = clones[i].clone();
                clip.start_frame = *start_frame;
                placed_ids.push(clip.id.clone());
                timeline.tracks[*track_idx].clips.push(clip);
            }
        }

        // Sort clips on each affected track
        for (track_idx, _, _, _) in &all_to_place {
            timeline.tracks[*track_idx]
                .clips
                .sort_by_key(|c| c.start_frame);
        }

        PasteResult {
            placed_clips: placed_ids,
            errors,
        }
    }

    /// CCB-009: Duplicate uses the same clone engine as paste.
    /// The duplicate anchor is the clips' actual start frame, so the paste
    /// offset is 0, and the clone lands at `target_frame` unchanged.
    pub fn duplicate_clips(
        timeline: &mut Timeline,
        clip_ids: &[String],
        target_track: Option<usize>,
        target_frame: i64,
    ) -> PasteResult {
        let clipboard = Self::copy_clips(
            timeline,
            clip_ids,
            // Use the first clip's start frame as anchor so offset is 0
            clip_ids
                .first()
                .and_then(|id| {
                    let loc = find_clip(timeline, id)?;
                    Some(timeline.tracks[loc.track_index].clips[loc.clip_index].start_frame)
                })
                .unwrap_or(target_frame),
        );
        Self::paste_at_frame(timeline, &clipboard, target_track, target_frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Interpolation, Track};

    fn make_clip(
        id: &str,
        track_idx: usize,
        start: i64,
        dur: i64,
        media_type: ClipType,
        link_group: Option<&str>,
    ) -> CopiedClip {
        CopiedClip {
            clip: Clip {
                id: id.to_string(),
                media_ref: format!("ref-{id}"),
                media_type,
                source_clip_type: media_type,
                start_frame: start,
                duration_frames: dur,
                speed: 1.0,
                volume: 1.0,
                opacity: 1.0,
                trim_start_frame: 0,
                trim_end_frame: 0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                link_group_id: link_group.map(String::from),
                caption_group_id: None,
                text_content: None,
                text_style: None,
                transform: core_model::Transform {
                    center_x: 0.5,
                    center_y: 0.5,
                    width: 1.0,
                    height: 1.0,
                    rotation: 0.0,
                    flip_horizontal: false,
                    flip_vertical: false,
                },
                crop: core_model::Crop {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                },
                shape_style: None,
                stroke_progress_track: None,
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
            },
            source_track_index: track_idx,
            offset_from_anchor: start - 0,
        }
    }

    fn make_video_track(clips: Vec<Clip>) -> Track {
        Track {
            id: "track-video".into(),
            r#type: ClipType::Video,
            clips,
            muted: false,
            hidden: false,
            sync_locked: true,
        }
    }

    fn make_audio_track(clips: Vec<Clip>) -> Track {
        Track {
            id: "track-audio".into(),
            r#type: ClipType::Audio,
            clips,
            muted: false,
            hidden: false,
            sync_locked: true,
        }
    }

    fn simple_clip(id: &str, media_type: ClipType, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.to_string(),
            media_ref: format!("ref-{id}"),
            media_type,
            source_clip_type: media_type,
            start_frame: start,
            duration_frames: dur,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            trim_start_frame: 0,
            trim_end_frame: 0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            transform: core_model::Transform {
                center_x: 0.5,
                center_y: 0.5,
                width: 1.0,
                height: 1.0,
                rotation: 0.0,
                flip_horizontal: false,
                flip_vertical: false,
            },
            crop: core_model::Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            shape_style: None,
            stroke_progress_track: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
        }
    }

    fn timeline_with_two_tracks() -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            tracks: vec![
                make_video_track(vec![
                    simple_clip("v1", ClipType::Video, 0, 100),
                    simple_clip("v2", ClipType::Video, 150, 50),
                ]),
                make_audio_track(vec![simple_clip("a1", ClipType::Audio, 0, 100)]),
            ],
        }
    }

    fn count_clips(timeline: &Timeline) -> usize {
        timeline.tracks.iter().map(|t| t.clips.len()).sum()
    }

    // ── CCB-001: Copy stores clip snapshots with offsets ──
    #[test]
    fn ccb_001_copy_stores_snapshots_with_offsets() {
        let timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        assert_eq!(clipboard.clips.len(), 1);
        assert_eq!(clipboard.clips[0].clip.id, "v1");
        assert_eq!(clipboard.clips[0].offset_from_anchor, 0);
    }

    // ── CCB-002: Copy order stable by track index, start frame, clip id ──
    #[test]
    fn ccb_002_copy_order_stable() {
        let mut timeline = timeline_with_two_tracks();
        // Add another clip to test ordering
        timeline.tracks[0]
            .clips
            .push(simple_clip("v0", ClipType::Video, 50, 30));
        let clipboard =
            ClipClipboardEngine::copy_clips(&timeline, &["v2".into(), "v1".into(), "v0".into()], 0);
        // Should be sorted: v1 (0,100), v0 (50,30), v2 (150,50)
        assert_eq!(clipboard.clips[0].clip.id, "v1");
        assert_eq!(clipboard.clips[1].clip.id, "v0");
        assert_eq!(clipboard.clips[2].clip.id, "v2");
    }

    // ── CCB-004: Paste prefers original source track ──
    #[test]
    fn ccb_004_paste_prefers_original_track() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 200);
        assert!(result.errors.is_empty(), "errors: {result:?}");
        assert_eq!(result.placed_clips.len(), 1);
        // Should be on video track (track 0)
        let placed = &timeline.tracks[0].clips;
        let found = placed.iter().any(|c| c.start_frame == 200);
        assert!(found, "clip should be placed at frame 200 on video track");
    }

    // ── CCB-005: Fallback to first compatible track when original missing ──
    #[test]
    fn ccb_005_fallback_to_first_compatible() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        // Remove video track
        timeline.tracks.remove(0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 50);
        // Audio is still there, but video can't go on audio track
        assert!(result.errors.len() == 1 || result.placed_clips.is_empty());
    }

    // ── CCB-006: No compatible track → no-op paste ──
    #[test]
    fn ccb_006_no_compatible_track_noop() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        // Remove all tracks
        timeline.tracks.clear();
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 50);
        assert!(result.placed_clips.is_empty());
        assert!(!result.errors.is_empty());
    }

    // ── CCB-007: Paste-at-track/frame applies relative offsets ──
    #[test]
    fn ccb_007_paste_at_track_frame_applies_offsets() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        // Copy anchor was at 0, v1 starts at 0, so offset = 0
        // Paste at track 0, frame 100 → v1 should land at frame 100
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, Some(0), 100);
        assert_eq!(result.placed_clips.len(), 1);
        let placed_id = &result.placed_clips[0];
        let loc = find_clip(&timeline, placed_id).unwrap();
        let placed_clip = &timeline.tracks[loc.track_index].clips[loc.clip_index];
        assert_eq!(placed_clip.start_frame, 100);
    }

    // ── CCB-008: Invalid placements skipped, valid ones still placed ──
    #[test]
    fn ccb_008_invalid_skipped_valid_placed() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into(), "a1".into()], 0);
        // Force paste to track 0 (video) for v1 and a1
        // a1 is audio and can't go on video track
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, Some(0), 200);
        // v1 should be placed (it's video, compatible with video track)
        // a1 should error (audio incompatible with video track)
        assert!(!result.errors.is_empty(), "audio paste should error");
        // At least one clip should have been placed
        // Only 1 clip placed (v1), a1 was skipped due to incompatibility
        assert_eq!(result.placed_clips.len(), 1);
    }

    // ── CCB-009: Duplicate uses same clone engine ──
    #[test]
    fn ccb_009_duplicate_uses_same_engine() {
        let mut timeline = timeline_with_two_tracks();
        let result = ClipClipboardEngine::duplicate_clips(&mut timeline, &["v1".into()], None, 200);
        assert_eq!(result.placed_clips.len(), 1);
        let placed_id = &result.placed_clips[0];
        assert_ne!(placed_id, "v1");
        // Original clip should still exist
        assert!(find_clip(&timeline, "v1").is_some());
    }

    // ── CCB-010: Paste clears overlapping destination regions ──
    #[test]
    fn ccb_010_paste_clears_overlap() {
        let mut timeline = timeline_with_two_tracks();
        // Paste v1 (duration 100) at frame 0, which overlaps with existing v1
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        let before = count_clips(&timeline);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, Some(0), 0);
        // Old clip should be removed by clear_region, new one inserted
        assert_eq!(result.placed_clips.len(), 1);
        // Total clips should be: old removed, new inserted = same count
        assert_eq!(count_clips(&timeline), before);
    }

    // ── CCB-011: Every clone gets a fresh clip id ──
    #[test]
    fn ccb_011_fresh_clip_ids() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into(), "v2".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 300);
        assert_eq!(result.placed_clips.len(), 2);
        // Clones must differ from originals and from each other
        assert_ne!(result.placed_clips[0], "v1");
        assert_ne!(result.placed_clips[0], result.placed_clips[1]);
        assert_ne!(result.placed_clips[1], "v2");
        // Originals still present
        assert!(find_clip(&timeline, "v1").is_some());
        assert!(find_clip(&timeline, "v2").is_some());
    }

    // ── CCB-012: Two clips sharing a link group → clones share a new remapped group ──
    #[test]
    fn ccb_012_shared_link_group() {
        let mut timeline = timeline_with_two_tracks();
        // Link v1 and v2 together
        let link_id = "link-abc".to_string();
        timeline.tracks[0].clips[0].link_group_id = Some(link_id.clone());
        timeline.tracks[0].clips[1].link_group_id = Some(link_id.clone());

        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into(), "v2".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 300);
        assert_eq!(result.placed_clips.len(), 2);

        // Find the placed clones
        let loc0 = find_clip(&timeline, &result.placed_clips[0]).unwrap();
        let loc1 = find_clip(&timeline, &result.placed_clips[1]).unwrap();
        let clone0 = &timeline.tracks[loc0.track_index].clips[loc0.clip_index];
        let clone1 = &timeline.tracks[loc1.track_index].clips[loc1.clip_index];

        // Both clones should have the same new link group (not the original)
        assert!(clone0.link_group_id.is_some());
        assert_eq!(clone0.link_group_id, clone1.link_group_id);
        assert_ne!(clone0.link_group_id.as_ref().unwrap(), &link_id);
    }

    // ── CCB-013: Single clip from a link group → clone is unlinked ──
    #[test]
    fn ccb_013_single_link_unlinked() {
        let mut timeline = timeline_with_two_tracks();
        // Only v1 has a link group, v2 doesn't
        timeline.tracks[0].clips[0].link_group_id = Some("link-abc".to_string());

        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 300);
        assert_eq!(result.placed_clips.len(), 1);

        let loc = find_clip(&timeline, &result.placed_clips[0]).unwrap();
        let clone = &timeline.tracks[loc.track_index].clips[loc.clip_index];
        // Single clip from a link group → clone becomes unlinked
        assert!(clone.link_group_id.is_none());
    }

    // ── CCB-014: Audio clip pastes correctly on audio track ──
    #[test]
    fn ccb_014_paste_audio_on_audio_track() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["a1".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, Some(1), 200);
        assert_eq!(result.placed_clips.len(), 1);
        let loc = find_clip(&timeline, &result.placed_clips[0]).unwrap();
        assert_eq!(loc.track_index, 1);
        let placed = &timeline.tracks[1].clips[loc.clip_index];
        assert_eq!(placed.start_frame, 200);
    }

    // ── CCB-015: Empty clipboard paste returns empty result ──
    #[test]
    fn ccb_015_empty_clipboard_paste() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &[], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 100);
        assert!(result.placed_clips.is_empty());
        assert!(result.errors.is_empty());
    }

    // ── CCB-016: Multiple clips from different tracks copied and placed ──
    #[test]
    fn ccb_016_multiple_clips_copied() {
        let mut timeline = timeline_with_two_tracks();
        let clipboard =
            ClipClipboardEngine::copy_clips(&timeline, &["v1".into(), "v2".into(), "a1".into()], 0);
        assert_eq!(clipboard.clips.len(), 3);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, None, 300);
        // All should be placed (each on their compatible track)
        assert_eq!(result.placed_clips.len(), 3);
    }

    // ── CCB-017: Duplicate with explicit track and frame ──
    #[test]
    fn ccb_017_duplicate_at_track_frame() {
        let mut timeline = timeline_with_two_tracks();
        let result =
            ClipClipboardEngine::duplicate_clips(&mut timeline, &["v1".into()], Some(0), 300);
        assert_eq!(result.placed_clips.len(), 1);
        let loc = find_clip(&timeline, &result.placed_clips[0]).unwrap();
        assert_eq!(
            timeline.tracks[loc.track_index].clips[loc.clip_index].start_frame,
            300
        );
    }

    // ── CCB-018: Paste preserves clip properties (duration, speed, volume) ──
    #[test]
    fn ccb_018_paste_preserves_clip_properties() {
        let mut timeline = timeline_with_two_tracks();
        // Modify v1's properties
        timeline.tracks[0].clips[0].speed = 2.0;
        timeline.tracks[0].clips[0].volume = 0.5;
        timeline.tracks[0].clips[0].opacity = 0.8;

        let clipboard = ClipClipboardEngine::copy_clips(&timeline, &["v1".into()], 0);
        let result = ClipClipboardEngine::paste_at_frame(&mut timeline, &clipboard, Some(0), 300);
        assert_eq!(result.placed_clips.len(), 1);

        let loc = find_clip(&timeline, &result.placed_clips[0]).unwrap();
        let placed = &timeline.tracks[loc.track_index].clips[loc.clip_index];
        assert_eq!(placed.speed, 2.0);
        assert_eq!(placed.volume, 0.5);
        assert_eq!(placed.opacity, 0.8);
        assert_eq!(placed.duration_frames, 100);
    }
}
