//! Compound clips (nested sequences) — Issue #155.
//!
//! `create_compound_clip` groups a contiguous run of clips on a single track
//! into one compound clip whose constituents move into a nested `Timeline`
//! stored in the project's `compound_timelines` map. `dissolve_compound_clip`
//! flattens it back onto the track. Rendering and export call
//! `flatten_compound_clips` first so a compound clip renders its nested content
//! rather than an empty frame.
//!
//! v1 scope: single-track grouping only (all selected clips must be on the same
//! track and adjacent). Multi-track grouping and composing a transform applied
//! to the compound clip itself onto the group are follow-ups; a freshly created
//! compound clip has identity transform/opacity/fades, so create→flatten
//! reproduces the pre-group placement exactly.

use crate::edit::find_clip;
use core_model::{Clip, ClipType, Timeline};
use uuid::Uuid;

/// Group `clip_ids` (all on one track, adjacent) into a compound clip.
///
/// The grouped clips move into a new nested timeline (re-based so the earliest
/// starts at frame 0); a single compound clip replaces them on the track,
/// spanning `[min_start, max_end)`. Returns the new compound clip's id.
///
/// The compound clip's `media_ref` is set to the nested-timeline id so a caller
/// that owns the media manifest can register a display name for it.
pub fn create_compound_clip(
    timeline: &mut Timeline,
    clip_ids: &[String],
    _name: Option<&str>,
) -> Result<String, String> {
    let mut unique: Vec<String> = Vec::new();
    for id in clip_ids {
        if !unique.contains(id) {
            unique.push(id.clone());
        }
    }
    if unique.is_empty() {
        return Err("Select at least one clip to group into a compound clip.".to_string());
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
            "All clips must be on the same track to group into a compound clip. \
             Multi-track grouping isn't supported yet."
                .to_string(),
        );
    }

    let mut clip_indices: Vec<usize> = locations.iter().map(|l| l.clip_index).collect();
    clip_indices.sort_unstable();
    let min_idx = clip_indices[0];
    let max_idx = clip_indices[clip_indices.len() - 1];
    if max_idx - min_idx + 1 != clip_indices.len() {
        return Err(
            "The selected clips must be adjacent (no other clip between them) to group \
             into a compound clip."
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

    // Carry any nested timelines owned by grouped compound clips into the new
    // nested timeline so nesting-in-nesting survives the move.
    let mut nested_compounds = std::collections::HashMap::new();
    for clip in &grouped {
        if let Some(cid) = &clip.compound_timeline_id {
            if let Some(inner) = timeline.compound_timelines.remove(cid) {
                nested_compounds.insert(cid.clone(), inner);
            }
        }
    }

    let nested_clips: Vec<Clip> = grouped
        .iter()
        .map(|c| {
            let mut nc = c.clone();
            nc.start_frame -= min_start;
            nc
        })
        .collect();

    let nested = Timeline {
        id: Uuid::new_v4().to_string(),
        name: "Compound Clip".to_string(),
        folder_id: None,
        fps: timeline.fps,
        width: timeline.width,
        height: timeline.height,
        settings_configured: timeline.settings_configured,
        selected_clip_ids: Default::default(),
        tracks: vec![core_model::Track {
            id: Uuid::new_v4().to_string(),
            r#type: track_type,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: nested_clips,
        }],
        transcription_language: timeline.transcription_language.clone(),
        compound_timelines: nested_compounds,
    };

    let nested_id = Uuid::new_v4().to_string();
    let compound_type = if track_type == ClipType::Audio {
        ClipType::Audio
    } else {
        ClipType::Video
    };
    let compound = Clip {
        id: Uuid::new_v4().to_string(),
        media_ref: nested_id.clone(),
        media_type: compound_type,
        source_clip_type: compound_type,
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
        compound_timeline_id: Some(nested_id.clone()),
        blend_mode: Default::default(),
        chroma_key: None,
    };
    let compound_id = compound.id.clone();

    let track = &mut timeline.tracks[track_index];
    track.clips.retain(|c| !unique.contains(&c.id));
    let insert_at = min_idx.min(track.clips.len());
    track.clips.insert(insert_at, compound);

    timeline
        .compound_timelines
        .insert(nested_id, Box::new(nested));

    timeline.selected_clip_ids.clear();
    timeline.selected_clip_ids.insert(compound_id.clone());

    Ok(compound_id)
}

/// Flatten a compound clip back into its constituent clips on its own track.
/// Returns the restored clip ids (their original ids). Errors if `clip_id` is
/// not a compound clip or its nested timeline is missing.
pub fn dissolve_compound_clip(
    timeline: &mut Timeline,
    clip_id: &str,
) -> Result<Vec<String>, String> {
    let Some(loc) = find_clip(timeline, clip_id) else {
        return Err(format!("Clip '{clip_id}' was not found on the timeline."));
    };
    let compound = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
    let Some(nested_id) = compound.compound_timeline_id.clone() else {
        return Err("That clip isn't a compound clip.".to_string());
    };
    let Some(nested) = timeline.compound_timelines.remove(&nested_id) else {
        return Err("The compound clip's nested timeline is missing.".to_string());
    };

    // Re-base constituents back to absolute frames. Any inner nested timelines
    // this compound owned return to the parent's compound map.
    let offset = compound.start_frame;
    let mut restored_ids = Vec::new();
    let mut restored_clips = Vec::new();
    for track in nested.tracks {
        for mut clip in track.clips {
            clip.start_frame += offset;
            restored_ids.push(clip.id.clone());
            restored_clips.push(clip);
        }
    }
    for (cid, inner) in nested.compound_timelines {
        timeline.compound_timelines.insert(cid, inner);
    }

    let track = &mut timeline.tracks[loc.track_index];
    track.clips.retain(|c| c.id != clip_id);
    track.clips.extend(restored_clips);
    track.clips.sort_by_key(|c| c.start_frame);

    timeline.selected_clip_ids.clear();
    timeline
        .selected_clip_ids
        .extend(restored_ids.iter().cloned());

    Ok(restored_ids)
}

/// Return a copy of `timeline` with every compound clip expanded into its
/// constituent clips at their absolute positions (recursively). Used by the
/// compositor and exporters so compound clips render/export their content.
///
/// The compound clip's own static opacity is multiplied onto each constituent
/// (exact for single-track nesting, where constituents never overlap). A
/// transform or fades applied to the compound clip itself are not yet composed
/// onto the group — a follow-up. The result has an empty `compound_timelines`.
pub fn flatten_compound_clips(timeline: &Timeline) -> Timeline {
    let mut out = timeline.clone();
    for track in &mut out.tracks {
        let mut flat: Vec<Clip> = Vec::with_capacity(track.clips.len());
        for clip in track.clips.drain(..) {
            match &clip.compound_timeline_id {
                Some(nested_id) => match timeline.compound_timelines.get(nested_id) {
                    Some(nested) => {
                        let nested_flat = flatten_compound_clips(nested);
                        let offset = clip.start_frame;
                        let group_opacity = clip.opacity;
                        for ntrack in nested_flat.tracks {
                            for mut nc in ntrack.clips {
                                nc.start_frame += offset;
                                nc.opacity *= group_opacity;
                                flat.push(nc);
                            }
                        }
                    }
                    None => flat.push(clip),
                },
                None => flat.push(clip),
            }
        }
        track.clips = flat;
        track.clips.sort_by_key(|c| c.start_frame);
    }
    out.compound_timelines.clear();
    out
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
                clips,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn create_groups_contiguous_clips_into_one_compound() {
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let cid =
            create_compound_clip(&mut tl, &["a".into(), "b".into()], Some("Group")).unwrap();

        // a+b replaced by one compound spanning [0,60); c untouched.
        assert_eq!(tl.tracks[0].clips.len(), 2);
        let compound = tl.tracks[0].clips.iter().find(|c| c.id == cid).unwrap();
        assert_eq!(compound.start_frame, 0);
        assert_eq!(compound.duration_frames, 60);
        assert!(compound.compound_timeline_id.is_some());
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == "c"));
        assert!(!tl.tracks[0].clips.iter().any(|c| c.id == "a"));

        // Nested timeline holds a+b re-based to 0.
        let nested = &tl.compound_timelines[compound.compound_timeline_id.as_ref().unwrap()];
        assert_eq!(nested.tracks[0].clips.len(), 2);
        assert_eq!(nested.tracks[0].clips[0].start_frame, 0);
        assert_eq!(nested.tracks[0].clips[1].start_frame, 30);
    }

    #[test]
    fn create_and_dissolve_a_single_clip_round_trips() {
        let mut tl = timeline_with(vec![clip("a", 10, 40), clip("b", 50, 30)]);
        let cid = create_compound_clip(&mut tl, &["a".into()], None).unwrap();
        // One-clip group: the compound spans exactly clip a; b is untouched.
        assert_eq!(tl.tracks[0].clips.len(), 2);
        let compound = tl.tracks[0].clips.iter().find(|c| c.id == cid).unwrap();
        assert_eq!(compound.start_frame, 10);
        assert_eq!(compound.duration_frames, 40);
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == "b"));

        let restored = dissolve_compound_clip(&mut tl, &cid).unwrap();
        assert_eq!(restored, vec!["a".to_string()]);
        let a = tl.tracks[0].clips.iter().find(|c| c.id == "a").unwrap();
        assert_eq!(a.start_frame, 10, "restored at original frame");
        assert!(tl.compound_timelines.is_empty());
    }

    #[test]
    fn create_refuses_non_adjacent_clips() {
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let err = create_compound_clip(&mut tl, &["a".into(), "c".into()], None).unwrap_err();
        assert!(err.contains("adjacent"), "err={err}");
    }

    #[test]
    fn create_refuses_multi_track() {
        let mut tl = timeline_with(vec![clip("a", 0, 30)]);
        tl.tracks.push(Track {
            id: "t1".to_string(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![clip("b", 0, 30)],
        });
        let err = create_compound_clip(&mut tl, &["a".into(), "b".into()], None).unwrap_err();
        assert!(err.contains("same track"), "err={err}");
    }

    #[test]
    fn create_refuses_unknown_clip() {
        let mut tl = timeline_with(vec![clip("a", 0, 30)]);
        let err = create_compound_clip(&mut tl, &["a".into(), "ghost".into()], None).unwrap_err();
        assert!(err.contains("not found"), "err={err}");
    }

    #[test]
    fn dissolve_restores_original_clips() {
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let cid =
            create_compound_clip(&mut tl, &["a".into(), "b".into()], Some("G")).unwrap();
        let restored = dissolve_compound_clip(&mut tl, &cid).unwrap();

        assert_eq!(restored.len(), 2);
        assert!(tl.compound_timelines.is_empty());
        assert_eq!(tl.tracks[0].clips.len(), 3);
        let a = tl.tracks[0].clips.iter().find(|c| c.id == "a").unwrap();
        assert_eq!(a.start_frame, 0);
        let b = tl.tracks[0].clips.iter().find(|c| c.id == "b").unwrap();
        assert_eq!(b.start_frame, 30);
    }

    #[test]
    fn dissolve_refuses_non_compound() {
        let mut tl = timeline_with(vec![clip("a", 0, 30)]);
        let err = dissolve_compound_clip(&mut tl, "a").unwrap_err();
        assert!(err.contains("isn't a compound clip"), "err={err}");
    }

    #[test]
    fn create_then_flatten_reproduces_original_placement() {
        let original = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 45),
            clip("c", 75, 30),
        ]);
        let mut tl = original.clone();
        create_compound_clip(&mut tl, &["a".into(), "b".into(), "c".into()], None).unwrap();

        let flat = flatten_compound_clips(&tl);
        assert!(flat.compound_timelines.is_empty());
        assert_eq!(flat.tracks[0].clips.len(), 3);
        for want in &original.tracks[0].clips {
            let got = flat.tracks[0].clips.iter().find(|c| c.id == want.id).unwrap();
            assert_eq!(got.start_frame, want.start_frame, "clip {}", want.id);
            assert_eq!(got.duration_frames, want.duration_frames);
        }
    }

    #[test]
    fn flatten_multiplies_group_opacity_onto_constituents() {
        let mut tl = timeline_with(vec![clip("a", 0, 30), clip("b", 30, 30)]);
        let cid = create_compound_clip(&mut tl, &["a".into(), "b".into()], None).unwrap();
        // Apply half opacity to the whole compound.
        let loc = find_clip(&tl, &cid).unwrap();
        tl.tracks[loc.track_index].clips[loc.clip_index].opacity = 0.5;

        let flat = flatten_compound_clips(&tl);
        for c in &flat.tracks[0].clips {
            assert!((c.opacity - 0.5).abs() < 1e-9, "opacity={}", c.opacity);
        }
    }

    #[test]
    fn flatten_leaves_plain_timeline_untouched() {
        let tl = timeline_with(vec![clip("a", 0, 30), clip("b", 30, 30)]);
        let flat = flatten_compound_clips(&tl);
        assert_eq!(flat.tracks[0].clips.len(), 2);
        assert_eq!(flat.tracks[0].clips[0].id, "a");
    }

    #[test]
    fn nested_compound_survives_grouping_and_dissolves_back() {
        // Group a+b, then group the compound with c, then dissolve twice.
        let mut tl = timeline_with(vec![
            clip("a", 0, 30),
            clip("b", 30, 30),
            clip("c", 60, 30),
        ]);
        let inner = create_compound_clip(&mut tl, &["a".into(), "b".into()], None).unwrap();
        let outer = create_compound_clip(&mut tl, &[inner.clone(), "c".into()], None).unwrap();
        assert_eq!(tl.tracks[0].clips.len(), 1);
        // The inner nested timeline moved inside the outer one, so only the outer
        // lives at the top level (proper hierarchical nesting).
        assert_eq!(tl.compound_timelines.len(), 1);

        // Flatten collapses both levels to a,b,c at absolute frames.
        let flat = flatten_compound_clips(&tl);
        assert_eq!(flat.tracks[0].clips.len(), 3);
        let ids: Vec<&str> = flat.tracks[0].clips.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"a") && ids.contains(&"b") && ids.contains(&"c"));
        let c = flat.tracks[0].clips.iter().find(|c| c.id == "c").unwrap();
        assert_eq!(c.start_frame, 60);

        // Dissolving the outer compound restores the inner compound + c.
        let restored = dissolve_compound_clip(&mut tl, &outer).unwrap();
        assert!(restored.contains(&inner));
        assert!(tl.tracks[0].clips.iter().any(|c| c.id == inner));
        assert!(tl.compound_timelines.contains_key(
            tl.tracks[0]
                .clips
                .iter()
                .find(|c| c.id == inner)
                .unwrap()
                .compound_timeline_id
                .as_ref()
                .unwrap()
        ));
    }
}
