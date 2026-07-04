use core_model::{Clip, Timeline};

use crate::keyframes::rescale_clip_keyframes;
use crate::ClipMathExt;

/// Return value from `apply_fps` describing the FPS change.
#[derive(Debug, Clone, PartialEq)]
pub struct FpsChangeReport {
    pub old_fps: i64,
    pub new_fps: i64,
    pub rescaled_clip_count: usize,
    pub rescaled_keyframe_count: usize,
}

impl FpsChangeReport {
    pub fn total_frames_before(&self) -> i64 {
        // Not tracked at this level; kept for future use.
        0
    }
}

/// Apply a new FPS to the timeline, rescaling all frame-based values.
///
/// PCFG-002: When fps changes, rescale:
///   - clip startFrame, durationFrames
///   - clip trimStartFrame, trimEndFrame
///   - keyframe frame positions
///   - fade lengths
///
/// PCFG-003: FPS retiming must preserve same-track non-overlap after rounding.
///
/// PCFG-004: FPS retiming must collapse rounded keyframe collisions
/// deterministically, matching the current last-value-wins behavior (via
/// `rescale_clip_keyframes` + `clamp_clip_keyframes_to_duration`).
///
/// PCFG-007: Applying new project settings marks `settingsConfigured = true`.
pub fn apply_fps(timeline: &mut Timeline, new_fps: i64) -> FpsChangeReport {
    let old_fps = timeline.fps;
    if old_fps == new_fps || old_fps <= 0 || new_fps <= 0 {
        timeline.settings_configured = true;
        timeline.fps = new_fps;
        return FpsChangeReport {
            old_fps,
            new_fps,
            rescaled_clip_count: 0,
            rescaled_keyframe_count: 0,
        };
    }

    let scale = new_fps as f64 / old_fps as f64;
    let mut total_keyframes = 0usize;

    for track in &mut timeline.tracks {
        let clip_indices: Vec<usize> = (0..track.clips.len()).collect();
        // Sort by start_frame to maintain non-overlap
        let mut sorted_indices: Vec<usize> = clip_indices.into_iter().collect::<Vec<_>>();
        sorted_indices.sort_by_key(|&i| track.clips[i].start_frame);

        let mut previous_end: Option<i64> = None;

        for &ci in &sorted_indices {
            let clip = &mut track.clips[ci];
            let scaled_start = (clip.start_frame as f64 * scale).round() as i64;
            let scaled_end = (clip.end_frame() as f64 * scale).round() as i64;

            // PCFG-003: preserve non-overlap
            clip.start_frame = match previous_end {
                Some(prev_end) => scaled_start.max(prev_end),
                None => scaled_start,
            };
            clip.duration_frames = (scaled_end - clip.start_frame).max(1);

            clip.trim_start_frame = (clip.trim_start_frame as f64 * scale).round() as i64;
            clip.trim_end_frame = (clip.trim_end_frame as f64 * scale).round() as i64;

            // Count keyframes before rescaling
            total_keyframes += count_keyframes(clip);

            // PCFG-004: rescale keyframes (collisions resolved by rescale_clip_keyframes)
            rescale_clip_keyframes(clip, scale);

            clip.fade_in_frames = (clip.fade_in_frames as f64 * scale).round() as i64;
            clip.fade_out_frames = (clip.fade_out_frames as f64 * scale).round() as i64;

            // Clamp after rescaling
            crate::keyframes::clamp_clip_keyframes_to_duration(clip);
            crate::keyframes::clamp_clip_fades_to_duration(clip);

            previous_end = Some(clip.start_frame + clip.duration_frames);
        }
    }

    timeline.fps = new_fps;
    timeline.settings_configured = true;

    FpsChangeReport {
        old_fps,
        new_fps,
        rescaled_clip_count: timeline.tracks.iter().map(|t| t.clips.len()).sum(),
        rescaled_keyframe_count: total_keyframes,
    }
}

/// Refit auto-fitted clips when canvas size changes.
///
/// PCFG-005: When canvas size changes, clips that still sit on the old
/// auto-fit transform must be re-fit to the new canvas.
///
/// PCFG-006: When canvas size changes, manually adjusted clips must keep
/// their user-authored transform.
///
/// The `is_auto_fit` closure receives a clip and returns true if the clip's
/// current transform is the auto-fit result for the old canvas.
pub fn refit_transforms(
    timeline: &mut Timeline,
    mut is_auto_fit: impl FnMut(&Clip) -> bool,
    new_width: i64,
    new_height: i64,
) {
    if new_width <= 0 || new_height <= 0 {
        return;
    }

    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            if is_auto_fit(clip) {
                clip.transform.width = 1.0;
                clip.transform.height = 1.0;
                clip.transform.center_x = 0.5;
                clip.transform.center_y = 0.5;
            }
        }
    }

    timeline.width = new_width;
    timeline.height = new_height;
    timeline.settings_configured = true;
}

/// Apply new timeline settings (fps + canvas size) atomically.
///
/// Combines `apply_fps` and `refit_transforms` into a single call.
pub fn apply_settings(
    timeline: &mut Timeline,
    new_fps: i64,
    new_width: i64,
    new_height: i64,
    is_auto_fit: impl FnMut(&Clip) -> bool,
) -> FpsChangeReport {
    let report = apply_fps(timeline, new_fps);
    refit_transforms(timeline, is_auto_fit, new_width, new_height);
    report
}

fn count_keyframes(clip: &Clip) -> usize {
    let mut count = 0usize;
    if let Some(t) = &clip.opacity_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.position_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.scale_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.rotation_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.crop_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.volume_track {
        count += t.keyframes.len();
    }
    if let Some(t) = &clip.stroke_progress_track {
        count += t.keyframes.len();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{
        Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Timeline, Track, Transform,
    };

    fn make_clip(id: &str, start: i64, duration: i64) -> Clip {
        Clip {
            id: id.to_string(),
            media_ref: format!("asset-{id}"),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: start,
            duration_frames: duration,
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

    fn make_track(kind: ClipType, clips: Vec<Clip>) -> Track {
        Track {
            id: format!("track-{kind:?}"),
            r#type: kind,
            muted: false,
            hidden: false,
            sync_locked: true,
            clips,
        }
    }

    fn make_timeline(
        fps: i64,
        width: i64,
        height: i64,
        configured: bool,
        tracks: Vec<Track>,
    ) -> Timeline {
        Timeline {
            fps,
            width,
            height,
            settings_configured: configured,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks,
            transcription_language: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    // ── PCFG-001 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_001_timeline_settings_fields() {
        // Verifies the Timeline struct has fps, width, height, settingsConfigured
        let tl = Timeline::default();
        assert_eq!(tl.fps, 30);
        assert_eq!(tl.width, 1920);
        assert_eq!(tl.height, 1080);
        assert!(!tl.settings_configured);
    }

    // ── PCFG-002 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_002_apply_fps_rescales_all_frame_values() {
        // 30fps → 60fps: all frame values should double
        let clip = make_clip("c1", 100, 50);
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        let report = apply_fps(&mut timeline, 60);

        assert_eq!(report.old_fps, 30);
        assert_eq!(report.new_fps, 60);
        assert_eq!(timeline.fps, 60);
        assert_eq!(timeline.tracks[0].clips[0].start_frame, 200);
        assert_eq!(timeline.tracks[0].clips[0].duration_frames, 100);
    }

    #[test]
    fn pcfg_002_apply_fps_rescales_trims() {
        let mut clip = make_clip("c1", 0, 30);
        clip.trim_start_frame = 10;
        clip.trim_end_frame = 5;
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        apply_fps(&mut timeline, 60);

        assert_eq!(timeline.tracks[0].clips[0].trim_start_frame, 20);
        assert_eq!(timeline.tracks[0].clips[0].trim_end_frame, 10);
    }

    #[test]
    fn pcfg_002_apply_fps_rescales_fades() {
        let mut clip = make_clip("c1", 0, 100);
        clip.fade_in_frames = 10;
        clip.fade_out_frames = 20;
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        apply_fps(&mut timeline, 60);

        assert_eq!(timeline.tracks[0].clips[0].fade_in_frames, 20);
        assert_eq!(timeline.tracks[0].clips[0].fade_out_frames, 40);
    }

    #[test]
    fn pcfg_002_apply_fps_rescales_keyframes() {
        let mut clip = make_clip("c1", 0, 100);
        clip.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 10,
                    value: 0.5,
                    interpolation_out: Interpolation::Smooth,
                },
                Keyframe {
                    frame: 50,
                    value: 1.0,
                    interpolation_out: Interpolation::Smooth,
                },
            ],
        });

        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        apply_fps(&mut timeline, 60);

        let kfs = timeline.tracks[0].clips[0].opacity_track.as_ref().unwrap();
        assert_eq!(kfs.keyframes.len(), 2);
        assert_eq!(kfs.keyframes[0].frame, 20);
        assert_eq!(kfs.keyframes[1].frame, 100);
    }

    #[test]
    fn pcfg_002_apply_fps_downscale() {
        // 60fps → 30fps: all frame values halved
        let clip = make_clip("c1", 200, 100);
        let mut timeline = make_timeline(
            60,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        apply_fps(&mut timeline, 30);

        assert_eq!(timeline.tracks[0].clips[0].start_frame, 100);
        assert_eq!(timeline.tracks[0].clips[0].duration_frames, 50);
    }

    // ── PCFG-003 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_003_preserves_non_overlap_after_upscale() {
        // Two clips: [0..50) and [60..100) at 30fps
        let c1 = make_clip("c1", 0, 50);
        let c2 = make_clip("c2", 60, 40);
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![c1, c2])],
        );

        apply_fps(&mut timeline, 60);

        let clips = &timeline.tracks[0].clips;
        // After 2x scale: c1 is [0..100), c2 should start at 100 or later
        assert!(
            clips[1].start_frame >= clips[0].start_frame + clips[0].duration_frames,
            "clips must not overlap: c1 end={}, c2 start={}",
            clips[0].start_frame + clips[0].duration_frames,
            clips[1].start_frame
        );
    }

    #[test]
    fn pcfg_003_preserves_non_overlap_after_downscale() {
        // Two clips: [0..100) and [150..200) at 60fps
        let c1 = make_clip("c1", 0, 100);
        let c2 = make_clip("c2", 150, 50);
        let mut timeline = make_timeline(
            60,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![c1, c2])],
        );

        apply_fps(&mut timeline, 30);

        let clips = &timeline.tracks[0].clips;
        assert!(
            clips[1].start_frame >= clips[0].start_frame + clips[0].duration_frames,
            "clips must not overlap after downscale: c1 end={}, c2 start={}",
            clips[0].start_frame + clips[0].duration_frames,
            clips[1].start_frame
        );
    }

    #[test]
    fn pcfg_003_preserves_non_overlap_three_clips() {
        let c1 = make_clip("c1", 0, 30);
        let c2 = make_clip("c2", 40, 20);
        let c3 = make_clip("c3", 70, 30);
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![c1, c2, c3])],
        );

        apply_fps(&mut timeline, 24);

        let clips = &timeline.tracks[0].clips;
        for i in 1..clips.len() {
            let prev_end = clips[i - 1].start_frame + clips[i - 1].duration_frames;
            assert!(
                clips[i].start_frame >= prev_end,
                "clips[{i}] starts at {} but prev ends at {}",
                clips[i].start_frame,
                prev_end
            );
        }
    }

    #[test]
    fn pcfg_003_different_tracks_independent() {
        // Non-overlap is per-track
        let c1 = make_clip("c1", 0, 50);
        let c2 = make_clip("c2", 0, 50); // same start, different track — fine
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![
                make_track(ClipType::Video, vec![c1]),
                make_track(ClipType::Audio, vec![c2]),
            ],
        );

        apply_fps(&mut timeline, 60);

        assert_eq!(timeline.tracks[0].clips[0].start_frame, 0);
        assert_eq!(timeline.tracks[1].clips[0].start_frame, 0);
    }

    // ── PCFG-004 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_004_collapsed_keyframes_after_rescale() {
        // Keyframes at frames 5 and 7 at 30fps → scaled to 24fps: 4 and ~5.6
        // After rounding they may collide; clamp handles it.
        let mut clip = make_clip("c1", 0, 50);
        clip.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 5,
                    value: 0.0,
                    interpolation_out: Interpolation::Smooth,
                },
                Keyframe {
                    frame: 7,
                    value: 1.0,
                    interpolation_out: Interpolation::Smooth,
                },
            ],
        });

        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        apply_fps(&mut timeline, 24);

        // Must not crash; keyframes should be in deterministic state
        let kfs = timeline.tracks[0].clips[0].opacity_track.as_ref().unwrap();
        assert!(!kfs.keyframes.is_empty(), "should have keyframes");

        // All frames should be within [0, duration]
        let duration = timeline.tracks[0].clips[0].duration_frames;
        for kf in &kfs.keyframes {
            assert!(
                kf.frame >= 0 && kf.frame <= duration,
                "frame {} out of range [0, {}]",
                kf.frame,
                duration
            );
        }
    }

    #[test]
    fn pcfg_004_out_of_range_keyframe_dropped_not_collapsed_onto_boundary() {
        // A keyframe past the clip duration (reachable via set_keyframes, which
        // does not clamp) must be DROPPED on rescale, not clamped onto the boundary
        // where it would overwrite a legitimate boundary keyframe (Swift parity).
        let mut clip = make_clip("c1", 0, 10);
        clip.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe { frame: 9, value: 0.2, interpolation_out: Interpolation::Linear },
                Keyframe { frame: 10, value: 0.5, interpolation_out: Interpolation::Linear },
                Keyframe { frame: 15, value: 0.9, interpolation_out: Interpolation::Linear },
            ],
        });
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );
        apply_fps(&mut timeline, 36); // scale 1.2 → new duration 12
        let kfs = &timeline.tracks[0].clips[0].opacity_track.as_ref().unwrap().keyframes;
        // 9→11, 10→12 (kept); 15→18 > 12 → dropped. The frame-12 boundary keeps 0.5.
        assert_eq!(kfs.len(), 2, "out-of-range keyframe dropped: {kfs:?}");
        assert_eq!(kfs[0].frame, 11);
        assert!((kfs[0].value - 0.2).abs() < 1e-9);
        assert_eq!(kfs[1].frame, 12);
        assert!(
            (kfs[1].value - 0.5).abs() < 1e-9,
            "boundary value must be Q=0.5, not R=0.9: {:?}",
            kfs[1]
        );
    }

    // ── PCFG-005 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_005_refit_auto_fitted_clips() {
        let c1 = make_clip("c1", 0, 50);
        let mut c2 = make_clip("c2", 60, 50);
        // c2 has a custom transform (not auto-fit)
        c2.transform = Transform {
            center_x: 0.3,
            center_y: 0.3,
            width: 0.8,
            height: 0.8,
            rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        };
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![c1, c2])],
        );

        let auto_fit_ids: Vec<String> = vec!["c1".into()];
        refit_transforms(&mut timeline, |c| auto_fit_ids.contains(&c.id), 3840, 2160);

        // Auto-fitted clip should be reset to default
        assert_eq!(timeline.tracks[0].clips[0].transform, Transform::default());

        // Manual clip should keep its transform
        assert!(
            (timeline.tracks[0].clips[1].transform.center_x - 0.3).abs() < 1e-9,
            "manual clip transform should be preserved"
        );

        assert_eq!(timeline.width, 3840);
        assert_eq!(timeline.height, 2160);
    }

    // ── PCFG-006 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_006_manual_clips_keep_transform() {
        let mut clip = make_clip("c1", 0, 50);
        clip.transform = Transform {
            center_x: 0.2,
            center_y: 0.8,
            width: 0.5,
            height: 0.5,
            rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        };
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        refit_transforms(&mut timeline, |c| c.id == "nonexistent", 2560, 1440);

        // Manual clip should be untouched
        assert!((timeline.tracks[0].clips[0].transform.center_x - 0.2).abs() < 1e-9);
    }

    // ── PCFG-007 ─────────────────────────────────────────────────────

    #[test]
    fn pcfg_007_apply_fps_sets_settings_configured() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        apply_fps(&mut timeline, 60);
        assert!(timeline.settings_configured);
    }

    #[test]
    fn pcfg_007_refit_sets_settings_configured() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        refit_transforms(&mut timeline, |_| false, 2560, 1440);
        assert!(timeline.settings_configured);
    }

    // ── Edge cases ───────────────────────────────────────────────────

    #[test]
    fn pcfg_apply_fps_same_fps_no_op() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        let report = apply_fps(&mut timeline, 30);
        assert_eq!(report.rescaled_clip_count, 0);
    }

    #[test]
    fn pcfg_apply_fps_zero_or_negative_clamped() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        // Should not panic
        apply_fps(&mut timeline, 0);
        apply_fps(&mut timeline, -10);
    }

    #[test]
    fn pcfg_apply_fps_empty_timeline() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        let report = apply_fps(&mut timeline, 60);
        assert_eq!(report.rescaled_clip_count, 0);
        assert_eq!(report.rescaled_keyframe_count, 0);
        assert_eq!(timeline.fps, 60);
    }

    #[test]
    fn pcfg_refit_zero_dimensions_no_op() {
        let mut timeline = make_timeline(30, 1920, 1080, false, vec![]);
        refit_transforms(&mut timeline, |_| true, 0, 0);
        assert_eq!(timeline.width, 1920);
    }

    #[test]
    fn pcfg_apply_fps_multiple_tracks() {
        let c1 = make_clip("c1", 0, 30);
        let c2 = make_clip("c2", 0, 60);
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            true,
            vec![
                make_track(ClipType::Video, vec![c1]),
                make_track(ClipType::Audio, vec![c2]),
            ],
        );

        let report = apply_fps(&mut timeline, 60);

        assert_eq!(report.rescaled_clip_count, 2);
        assert_eq!(timeline.tracks[0].clips[0].start_frame, 0);
        assert_eq!(timeline.tracks[0].clips[0].duration_frames, 60);
        assert_eq!(timeline.tracks[1].clips[0].start_frame, 0);
        assert_eq!(timeline.tracks[1].clips[0].duration_frames, 120);
    }

    #[test]
    fn pcfg_apply_settings_combined() {
        let mut clip = make_clip("c1", 0, 30);
        clip.transform = Transform::default(); // auto-fit default
        let mut timeline = make_timeline(
            30,
            1920,
            1080,
            false,
            vec![make_track(ClipType::Video, vec![clip])],
        );

        let report = apply_settings(&mut timeline, 60, 3840, 2160, |_| true);

        assert_eq!(report.new_fps, 60);
        assert_eq!(timeline.width, 3840);
        assert_eq!(timeline.height, 2160);
        assert!(timeline.settings_configured);
        // Clip should be rescaled AND refit
        assert_eq!(timeline.tracks[0].clips[0].transform, Transform::default());
    }
}
