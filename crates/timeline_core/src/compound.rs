//! Nested timelines ("compound clips") — Swift #255 representation.
//!
//! A nest is a clip with `source_clip_type == Sequence` whose `media_ref` is a
//! SIBLING timeline's id (`ProjectFile.timelines`); nothing is embedded in the
//! parent. `nest_clips` groups clips into a new child timeline + carrier clip;
//! `decompose_nest` expands a carrier back; `flatten_nests` resolves carriers
//! into constituent clips for audio/export (video composes recursively in the
//! compositor so the carrier's transform applies to the group as a unit).
//!
//! v1 scope mirrors the previous compound implementation: single-track
//! grouping. Mirrors Swift `NestFlattener` windowing exactly: window =
//! `trim_start..trim_start+duration`, shift = `start - trim_start`, flattened
//! ids are `"{carrier_id}/{clip_id}"`. Sequence carriers don't retime
//! (Swift `supportsRetiming == false`).

use crate::edit::find_clip;
use core_model::{Clip, ClipType, Timeline};
use std::collections::HashMap;
use uuid::Uuid;

/// Swift `NestFlattener.maxDepth`.
pub const NEST_MAX_DEPTH: usize = 8;

/// Result of grouping clips into a nest: the NEW child timeline (the caller
/// must add it to the project's sibling timelines) and the carrier clip's id.
#[derive(Debug, Clone)]
pub struct NestResult {
    pub child: Timeline,
    pub carrier_id: String,
}

/// Group `clip_ids` (all on one track, adjacent) into a nested timeline.
///
/// The grouped clips move into a new child `Timeline` (re-based to 0); a
/// single sequence-carrier clip replaces them, spanning `[min_start, max_end)`.
/// The child is RETURNED — the caller stores it as a project sibling.
pub fn nest_clips(
    timeline: &mut Timeline,
    clip_ids: &[String],
    name: Option<&str>,
) -> Result<NestResult, String> {
    let mut unique: Vec<String> = Vec::new();
    for id in clip_ids {
        if !unique.contains(id) {
            unique.push(id.clone());
        }
    }
    if unique.is_empty() {
        return Err("Select at least one clip to nest.".to_string());
    }

    let mut locations = Vec::with_capacity(unique.len());
    for id in &unique {
        match find_clip(timeline, id) {
            Some(loc) => locations.push(loc),
            None => return Err(format!("Clip '{id}' was not found on the timeline.")),
        }
    }

    let track_index = locations[0].track_index;
    if locations.iter().any(|l| l.track_index != track_index) {
        return Err(
            "All clips must be on the same track to nest. Multi-track nesting isn't \
             supported yet."
                .to_string(),
        );
    }

    let mut clip_indices: Vec<usize> = locations.iter().map(|l| l.clip_index).collect();
    clip_indices.sort_unstable();
    let min_idx = clip_indices[0];
    let max_idx = clip_indices[clip_indices.len() - 1];
    if max_idx - min_idx + 1 != clip_indices.len() {
        return Err(
            "The selected clips must be adjacent (no other clip between them) to nest."
                .to_string(),
        );
    }

    let track = &timeline.tracks[track_index];
    let track_type = track.r#type;
    let grouped: Vec<Clip> = clip_indices.iter().map(|&i| track.clips[i].clone()).collect();

    let min_start = grouped.iter().map(|c| c.start_frame).min().unwrap_or(0);
    let max_end = grouped
        .iter()
        .map(|c| c.start_frame + c.duration_frames)
        .max()
        .unwrap_or(min_start);
    let span = (max_end - min_start).max(1);

    let nested_clips: Vec<Clip> = grouped
        .iter()
        .map(|c| {
            let mut nc = c.clone();
            nc.start_frame -= min_start;
            nc
        })
        .collect();

    let child = Timeline {
        id: Uuid::new_v4().to_string(),
        name: name.unwrap_or("Nested Timeline").to_string(),
        tracks: vec![core_model::Track {
            id: Uuid::new_v4().to_string(),
            r#type: track_type,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: nested_clips,
        }],
        fps: timeline.fps,
        width: timeline.width,
        height: timeline.height,
        settings_configured: timeline.settings_configured,
        transcription_language: timeline.transcription_language.clone(),
        ..Default::default()
    };

    // Carrier types mirror Swift: a video-track nest is a `.sequence` clip; an
    // audio-track nest keeps mediaType audio with sourceClipType sequence.
    let (media_type, source_type) = if track_type == ClipType::Audio {
        (ClipType::Audio, ClipType::Sequence)
    } else {
        (ClipType::Sequence, ClipType::Sequence)
    };
    let carrier = Clip {
        id: Uuid::new_v4().to_string(),
        media_ref: child.id.clone(),
        media_type,
        source_clip_type: source_type,
        start_frame: min_start,
        duration_frames: span,
        trim_start_frame: 0,
        trim_end_frame: 0,
        speed: 1.0,
        volume: 1.0,
        fade_in_frames: 0,
        fade_out_frames: 0,
        fade_in_interpolation: core_model::Interpolation::Linear,
        fade_out_interpolation: core_model::Interpolation::Linear,
        opacity: 1.0,
        transform: Default::default(),
        crop: Default::default(),
        link_group_id: None,
        caption_group_id: None,
        text_content: None,
        text_style: None,
        text_animation: None,
        word_timings: None,
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
    };
    let carrier_id = carrier.id.clone();

    let track = &mut timeline.tracks[track_index];
    track.clips.retain(|c| !unique.contains(&c.id));
    let insert_at = min_idx.min(track.clips.len());
    track.clips.insert(insert_at, carrier);

    timeline.selected_clip_ids.clear();
    timeline.selected_clip_ids.insert(carrier_id.clone());

    Ok(NestResult { child, carrier_id })
}

/// Expand a nest carrier back into its constituent clips on its own track.
/// `child` is the carrier's timeline (resolved by the caller from the project's
/// siblings). Only the carrier's visible window is restored, shifted so child
/// frame `trim_start` lands at the carrier's `start_frame` (NestFlattener
/// windowing). Returns the restored clip ids.
pub fn decompose_nest(
    timeline: &mut Timeline,
    carrier_id: &str,
    child: &Timeline,
) -> Result<Vec<String>, String> {
    let Some(loc) = find_clip(timeline, carrier_id) else {
        return Err(format!("Clip '{carrier_id}' was not found on the timeline."));
    };
    let carrier = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
    if carrier.source_clip_type != ClipType::Sequence {
        return Err("That clip isn't a nested timeline.".to_string());
    }

    let mut restored_ids = Vec::new();
    let mut restored_clips = Vec::new();
    for track in &child.tracks {
        for clip in &track.clips {
            if let Some(mut c) = remap_into_window(clip, &carrier) {
                c.id = Uuid::new_v4().to_string();
                restored_ids.push(c.id.clone());
                restored_clips.push(c);
            }
        }
    }

    let track = &mut timeline.tracks[loc.track_index];
    track.clips.retain(|c| c.id != carrier_id);
    track.clips.extend(restored_clips);
    track.clips.sort_by_key(|c| c.start_frame);

    timeline.selected_clip_ids.clear();
    timeline
        .selected_clip_ids
        .extend(restored_ids.iter().cloned());

    Ok(restored_ids)
}

/// NestFlattener window remap: trim `clip` to the carrier's visible window and
/// shift it onto the parent's frame axis. Returns None when fully outside.
fn remap_into_window(clip: &Clip, carrier: &Clip) -> Option<Clip> {
    let win_start = carrier.trim_start_frame;
    let win_end = carrier.trim_start_frame + carrier.duration_frames;
    let shift = carrier.start_frame - carrier.trim_start_frame;

    let clip_start = clip.start_frame;
    let clip_end = clip.start_frame + clip.duration_frames;
    let start = clip_start.max(win_start);
    let end = clip_end.min(win_end);
    if end <= start {
        return None;
    }

    let head_cut = start - clip_start;
    let tail_cut = clip_end - end;
    let mut c = if head_cut > 0 {
        // Same conventions as split_single_clip's right half: keyframes shift,
        // the cut becomes source trim, and the fade on the cut edge clears.
        let (_, mut right) = crate::keyframes::split_all_clip_keyframe_tracks(clip, head_cut);
        right.trim_start_frame = clip.trim_start_frame + (head_cut as f64 * clip.speed).round() as i64;
        right.fade_in_frames = 0;
        right
    } else {
        clip.clone()
    };
    c.start_frame = start + shift;
    c.duration_frames = end - start;
    if tail_cut > 0 {
        c.trim_end_frame = clip.trim_end_frame + (tail_cut as f64 * clip.speed).round() as i64;
        c.fade_out_frames = 0;
    }
    crate::keyframes::clamp_clip_keyframes_to_duration(&mut c);
    crate::keyframes::clamp_clip_fades_to_duration(&mut c);
    Some(c)
}

/// Resolve every nest carrier in `timeline` into its constituent clips at
/// their parent positions (recursively, cycle-safe, depth-capped at
/// [`NEST_MAX_DEPTH`]). Used by AUDIO mixing and the XML/FCPXML exporters —
/// video composes carriers recursively in the compositor instead so the
/// carrier's transform applies to the group as a unit.
///
/// The carrier's static opacity/volume multiply onto constituents; flattened
/// clip ids become `"{carrier_id}/{clip_id}"` (Swift NestFlattener).
pub fn flatten_nests(
    timeline: &Timeline,
    resolve: &dyn Fn(&str) -> Option<Timeline>,
) -> Timeline {
    flatten_nests_inner(timeline, resolve, 0, &mut Vec::new())
}

fn flatten_nests_inner(
    timeline: &Timeline,
    resolve: &dyn Fn(&str) -> Option<Timeline>,
    depth: usize,
    visiting: &mut Vec<String>,
) -> Timeline {
    let mut out = timeline.clone();
    if depth >= NEST_MAX_DEPTH {
        return out;
    }
    for track in &mut out.tracks {
        let mut flat: Vec<Clip> = Vec::with_capacity(track.clips.len());
        for clip in track.clips.drain(..) {
            if clip.source_clip_type != ClipType::Sequence {
                flat.push(clip);
                continue;
            }
            if visiting.iter().any(|v| v == &clip.media_ref) {
                continue; // cycle — drop the carrier
            }
            let Some(child) = resolve(&clip.media_ref) else {
                continue; // missing child renders/exports nothing
            };
            visiting.push(clip.media_ref.clone());
            let child_flat = flatten_nests_inner(&child, resolve, depth + 1, visiting);
            visiting.pop();
            for ntrack in child_flat.tracks {
                // An audio carrier pulls the child's audio; a video carrier its video.
                let want_audio = clip.media_type == ClipType::Audio;
                if want_audio != (ntrack.r#type == ClipType::Audio) {
                    continue;
                }
                if want_audio && ntrack.muted {
                    continue;
                }
                if !want_audio && ntrack.hidden {
                    continue;
                }
                for nc in ntrack.clips {
                    if let Some(mut c) = remap_into_window(&nc, &clip) {
                        c.id = format!("{}/{}", clip.id, c.id);
                        c.opacity *= clip.opacity;
                        c.volume *= clip.volume;
                        flat.push(c);
                    }
                }
            }
        }
        track.clips = flat;
        track.clips.sort_by_key(|c| c.start_frame);
    }
    out
}

/// Build a resolver over a project's sibling timelines (id → clone).
pub fn timeline_resolver(siblings: &[Timeline]) -> HashMap<String, Timeline> {
    siblings.iter().map(|t| (t.id.clone(), t.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{ClipType, Timeline, Track};

    fn clip(id: &str, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.to_string(),
            media_ref: format!("ref-{id}"),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
            opacity: 1.0,
            transform: Default::default(),
            crop: Default::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            text_animation: None,
            word_timings: None,
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
        }
    }

    fn timeline_with(clips: Vec<Clip>) -> Timeline {
        Timeline {
            tracks: vec![Track {
                id: "t0".to_string(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips,
            }],
            ..Default::default()
        }
    }

    fn resolver_of(children: Vec<Timeline>) -> impl Fn(&str) -> Option<Timeline> {
        move |id: &str| children.iter().find(|t| t.id == id).cloned()
    }

    #[test]
    fn nest_groups_contiguous_clips_into_a_sequence_carrier() {
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let nest = nest_clips(&mut tl, &["a".into(), "b".into()], Some("Scene 1")).unwrap();

        assert_eq!(tl.tracks[0].clips.len(), 2);
        let carrier = tl.tracks[0]
            .clips
            .iter()
            .find(|c| c.id == nest.carrier_id)
            .unwrap();
        assert_eq!(carrier.media_type, ClipType::Sequence);
        assert_eq!(carrier.source_clip_type, ClipType::Sequence);
        assert_eq!(carrier.media_ref, nest.child.id, "carrier points at the child");
        assert_eq!(carrier.start_frame, 0);
        assert_eq!(carrier.duration_frames, 60);
        assert!(carrier.compound_timeline_id.is_none(), "no legacy field");

        assert_eq!(nest.child.name, "Scene 1");
        assert_eq!(nest.child.tracks[0].clips.len(), 2);
        assert_eq!(nest.child.tracks[0].clips[0].start_frame, 0);
        assert_eq!(nest.child.tracks[0].clips[1].start_frame, 30);
    }

    #[test]
    fn nest_refusals() {
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let err = nest_clips(&mut tl, &["a".into(), "c".into()], None).unwrap_err();
        assert!(err.contains("adjacent"), "{err}");
        let err = nest_clips(&mut tl, &["ghost".into()], None).unwrap_err();
        assert!(err.contains("not found"), "{err}");
        tl.tracks.push(Track {
            id: "t1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![clip("d", 0, 30)],
        });
        let err = nest_clips(&mut tl, &["a".into(), "d".into()], None).unwrap_err();
        assert!(err.contains("same track"), "{err}");
    }

    #[test]
    fn decompose_restores_placement() {
        let mut tl = timeline_with(vec![clip("a", 10, 30), clip("b", 40, 20)]);
        let nest = nest_clips(&mut tl, &["a".into(), "b".into()], None).unwrap();
        let restored = decompose_nest(&mut tl, &nest.carrier_id, &nest.child).unwrap();

        assert_eq!(restored.len(), 2);
        assert_eq!(tl.tracks[0].clips.len(), 2);
        assert_eq!(tl.tracks[0].clips[0].start_frame, 10);
        assert_eq!(tl.tracks[0].clips[0].duration_frames, 30);
        assert_eq!(tl.tracks[0].clips[1].start_frame, 40);
    }

    #[test]
    fn decompose_respects_trim_window() {
        let mut tl = timeline_with(vec![clip("a", 0, 30), clip("b", 30, 30)]);
        let nest = nest_clips(&mut tl, &["a".into(), "b".into()], None).unwrap();
        // Trim the carrier to child frames 20..50 and move it to frame 100.
        {
            let c = tl.tracks[0]
                .clips
                .iter_mut()
                .find(|c| c.id == nest.carrier_id)
                .unwrap();
            c.trim_start_frame = 20;
            c.duration_frames = 30;
            c.start_frame = 100;
        }
        decompose_nest(&mut tl, &nest.carrier_id, &nest.child).unwrap();
        // a's tail (child 20..30) → parent 100..110; b's head (child 30..50) → 110..130.
        assert_eq!(tl.tracks[0].clips.len(), 2);
        assert_eq!(tl.tracks[0].clips[0].start_frame, 100);
        assert_eq!(tl.tracks[0].clips[0].duration_frames, 10);
        assert_eq!(
            tl.tracks[0].clips[0].trim_start_frame, 20,
            "head cut becomes source trim"
        );
        assert_eq!(tl.tracks[0].clips[1].start_frame, 110);
        assert_eq!(tl.tracks[0].clips[1].duration_frames, 20);
    }

    #[test]
    fn flatten_resolves_carrier_to_constituents() {
        let mut tl = timeline_with(vec![clip("a", 0, 30), clip("b", 30, 30)]);
        let nest = nest_clips(&mut tl, &["a".into(), "b".into()], None).unwrap();
        let resolve = resolver_of(vec![nest.child.clone()]);
        let flat = flatten_nests(&tl, &resolve);

        assert_eq!(flat.tracks[0].clips.len(), 2);
        assert_eq!(flat.tracks[0].clips[0].start_frame, 0);
        assert!(
            flat.tracks[0].clips[0].id.starts_with(&nest.carrier_id),
            "flattened ids are carrier-scoped: {}",
            flat.tracks[0].clips[0].id
        );
    }

    #[test]
    fn flatten_multiplies_carrier_opacity() {
        let mut tl = timeline_with(vec![clip("a", 0, 30)]);
        let nest = nest_clips(&mut tl, &["a".into()], None).unwrap();
        tl.tracks[0].clips[0].opacity = 0.5;
        let resolve = resolver_of(vec![nest.child.clone()]);
        let flat = flatten_nests(&tl, &resolve);
        assert!((flat.tracks[0].clips[0].opacity - 0.5).abs() < 1e-9);
    }

    #[test]
    fn flatten_drops_missing_child_and_survives_cycles() {
        let mut tl = timeline_with(vec![clip("a", 0, 30)]);
        let nest = nest_clips(&mut tl, &["a".into()], None).unwrap();
        // Missing child: carrier drops, no panic.
        let none = |_: &str| None;
        let flat = flatten_nests(&tl, &none);
        assert!(flat.tracks[0].clips.is_empty());

        // Cycle: child contains a carrier pointing back at itself.
        let mut cyclic = nest.child.clone();
        let mut back = clip("back", 0, 30);
        back.media_type = ClipType::Sequence;
        back.source_clip_type = ClipType::Sequence;
        back.media_ref = cyclic.id.clone();
        cyclic.tracks[0].clips.push(back);
        let resolve = resolver_of(vec![cyclic.clone()]);
        let flat = flatten_nests(&tl, &resolve);
        // The self-referencing carrier inside the child is dropped; a's remap survives.
        assert!(flat.tracks[0].clips.iter().all(|c| c.media_ref != cyclic.id));
    }

    #[test]
    fn nested_nests_flatten_recursively() {
        // Group a, then nest the carrier with c at the parent level.
        let mut tl = timeline_with(vec![clip("a", 0, 30), clip("c", 30, 30)]);
        let inner = nest_clips(&mut tl, &["a".into()], None).unwrap();
        let outer = nest_clips(&mut tl, &[inner.carrier_id.clone(), "c".into()], None).unwrap();
        assert_eq!(tl.tracks[0].clips.len(), 1);

        let resolve = resolver_of(vec![inner.child.clone(), outer.child.clone()]);
        let flat = flatten_nests(&tl, &resolve);
        assert_eq!(flat.tracks[0].clips.len(), 2, "both levels resolved");
        let starts: Vec<i64> = flat.tracks[0].clips.iter().map(|c| c.start_frame).collect();
        assert_eq!(starts, vec![0, 30]);
    }

    #[test]
    fn audio_carrier_pulls_only_audio_tracks() {
        let mut child = timeline_with(vec![clip("v", 0, 30)]);
        child.tracks.push(Track {
            id: "aud".into(),
            r#type: ClipType::Audio,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![{
                let mut a = clip("asrc", 0, 30);
                a.media_type = ClipType::Audio;
                a.source_clip_type = ClipType::Audio;
                a
            }],
        });
        child.id = "child-av".into();

        let mut carrier = clip("carrier", 0, 30);
        carrier.media_type = ClipType::Audio;
        carrier.source_clip_type = ClipType::Sequence;
        carrier.media_ref = "child-av".into();
        let mut parent = Timeline::default();
        parent.tracks.push(Track {
            id: "pa".into(),
            r#type: ClipType::Audio,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![carrier],
        });

        let resolve = resolver_of(vec![child]);
        let flat = flatten_nests(&parent, &resolve);
        assert_eq!(flat.tracks[0].clips.len(), 1);
        assert_eq!(flat.tracks[0].clips[0].media_type, ClipType::Audio);
        assert!(flat.tracks[0].clips[0].id.ends_with("/asrc"));
    }
}
