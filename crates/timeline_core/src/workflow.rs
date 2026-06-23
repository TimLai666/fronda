use crate::{
    linked_partner_ids,
    ripple::{
        compute_ripple_push, compute_ripple_shifts_for_ranges, gap_is_still_empty, merge_ranges,
        validate_track_shifts, ClipShift, FrameRange,
    },
    ClipMathExt,
};
use core_model::{Clip, ClipType, Timeline};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleShiftSet {
    pub shifts_by_track: Vec<Vec<ClipShift>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleDeleteConfig {
    pub anchor_track_index: usize,
    pub ranges: Vec<FrameRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RippleDeleteOutcome {
    Ok(RippleDeleteReport),
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleDeleteReport {
    pub removed_frames: i64,
    pub cleared_track_indices: Vec<usize>,
    pub shifted_clip_count: usize,
    pub anchor_track_index: usize,
    pub resulting_fragments: Vec<ClipFragment>,
    pub removed_clip_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipFragment {
    pub clip_id: String,
    pub start_frame: i64,
    pub duration_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleInsertConfig {
    pub track_index: usize,
    pub insert_frame: i64,
    pub clips: Vec<RippleInsertClipSpec>,
    pub linked_audio_track_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleInsertClipSpec {
    pub asset_id: String,
    pub duration_frames: i64,
    pub trim_start_frame: Option<i64>,
    pub trim_end_frame: Option<i64>,
}

pub fn compute_ripple_delete(
    timeline: &Timeline,
    config: RippleDeleteConfig,
) -> RippleDeleteOutcome {
    let merged = merge_ranges(&config.ranges);
    if merged.is_empty() {
        return RippleDeleteOutcome::Refused("No non-empty ranges to delete".into());
    }
    let total_removed = merged.iter().map(FrameRange::length).sum();

    let mut clear_track_indices: BTreeSet<usize> = BTreeSet::from([config.anchor_track_index]);

    for clip in &timeline.tracks[config.anchor_track_index].clips {
        if clip.link_group_id.is_some()
            && merged
                .iter()
                .any(|r| r.start < clip.end_frame() && r.end > clip.start_frame)
        {
            for partner_id in linked_partner_ids(timeline, &clip.id) {
                if let Some((ti, _)) = find_clip(timeline, &partner_id) {
                    clear_track_indices.insert(ti);
                }
            }
        }
    }

    for (ti, track) in timeline.tracks.iter().enumerate() {
        if clear_track_indices.contains(&ti) || !track.sync_locked {
            continue;
        }
        let shifts = compute_ripple_shifts_for_ranges(&track.clips, &merged);
        if let Err(err) = validate_track_shifts(track, &shifts) {
            return RippleDeleteOutcome::Refused(format!("{err:?}"));
        }
    }

    RippleDeleteOutcome::Ok(RippleDeleteReport {
        removed_frames: total_removed,
        cleared_track_indices: clear_track_indices.into_iter().collect(),
        shifted_clip_count: 0,
        anchor_track_index: config.anchor_track_index,
        resulting_fragments: Vec::new(),
        removed_clip_ids: Vec::new(),
    })
}

pub fn compute_ripple_delete_gap(
    timeline: &Timeline,
    track_index: usize,
    range: FrameRange,
) -> Result<Vec<Vec<ClipShift>>, String> {
    if !gap_is_still_empty(&timeline.tracks[track_index], range) {
        return Err("gap no longer empty".into());
    }
    let mut shifts_by_track = Vec::new();
    for (ti, track) in timeline.tracks.iter().enumerate() {
        if ti != track_index && !track.sync_locked {
            shifts_by_track.push(Vec::new());
            continue;
        }
        let shifts = compute_ripple_shifts_for_ranges(&track.clips, &[range]);
        if ti != track_index {
            if let Err(err) = validate_track_shifts(track, &shifts) {
                return Err(format!("{err:?}"));
            }
        }
        shifts_by_track.push(shifts);
    }
    Ok(shifts_by_track)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RippleInsertOutcome {
    Ok(RippleInsertReport),
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleInsertReport {
    pub total_push: i64,
    pub push_track_indices: Vec<usize>,
    pub created_clip_ids: Vec<String>,
    pub shifts_by_track: Vec<Vec<ClipShift>>,
    /// The frame at which clips were inserted (needed to know where to place them).
    pub insert_frame: i64,
    /// The target track index where new clips are placed.
    pub track_index: usize,
    /// The clip specs used for insertion (needed to construct actual Clip objects).
    pub clips: Vec<RippleInsertClipSpec>,
}

pub fn compute_ripple_insert(
    timeline: &Timeline,
    config: RippleInsertConfig,
) -> RippleInsertOutcome {
    if config.clips.is_empty() {
        return RippleInsertOutcome::Refused("No clips to insert".into());
    }
    if config.insert_frame < 0 {
        return RippleInsertOutcome::Refused("Insert frame is negative".into());
    }
    if config.track_index >= timeline.tracks.len() {
        return RippleInsertOutcome::Refused("Track index out of bounds".into());
    }

    let total_push: i64 = config.clips.iter().map(|c| c.duration_frames).sum();
    if total_push <= 0 {
        return RippleInsertOutcome::Refused("Total push must be positive".into());
    }

    // Collect tracks to push: target + linked audio + sync-locked
    let mut push_track_indices: BTreeSet<usize> = BTreeSet::from([config.track_index]);
    if let Some(audio_ti) = config.linked_audio_track_index {
        if audio_ti < timeline.tracks.len() {
            push_track_indices.insert(audio_ti);
        }
    }
    for (ti, track) in timeline.tracks.iter().enumerate() {
        if track.sync_locked {
            push_track_indices.insert(ti);
        }
    }

    // Validate that no straddling clip is at an invalid split boundary
    for ti in &push_track_indices {
        if let Some(straddler) = timeline.tracks[*ti]
            .clips
            .iter()
            .find(|c| c.start_frame < config.insert_frame && config.insert_frame < c.end_frame())
        {
            if config.insert_frame <= straddler.start_frame
                || config.insert_frame >= straddler.end_frame()
            {
                return RippleInsertOutcome::Refused(
                    "Straddling clip insert frame at boundary".into(),
                );
            }
        }
    }

    // Compute push shifts for each pushed track
    let mut shifts_by_track: Vec<Vec<ClipShift>> = Vec::new();
    for ti in 0..timeline.tracks.len() {
        if push_track_indices.contains(&ti) {
            let shifts = compute_ripple_push(
                &timeline.tracks[ti].clips,
                config.insert_frame,
                total_push,
                &BTreeSet::new(),
            );
            if let Err(err) = validate_track_shifts(&timeline.tracks[ti], &shifts) {
                return RippleInsertOutcome::Refused(format!("{err:?}"));
            }
            shifts_by_track.push(shifts);
        } else {
            shifts_by_track.push(Vec::new());
        }
    }

    // Generate created clip IDs
    let created_clip_ids: Vec<String> = config
        .clips
        .iter()
        .map(|_| Uuid::new_v4().to_string())
        .collect();

    RippleInsertOutcome::Ok(RippleInsertReport {
        total_push,
        push_track_indices: push_track_indices.into_iter().collect(),
        created_clip_ids,
        shifts_by_track,
        insert_frame: config.insert_frame,
        track_index: config.track_index,
        clips: config.clips,
    })
}

// ─── Ripple insert with split ───

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RippleInsertWithSplitOutcome {
    Ok(RippleInsertWithSplitPlan),
    Refused(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RippleInsertWithSplitPlan {
    /// The regular insert report
    pub insert: RippleInsertReport,
    /// Split actions to execute before the insert.
    /// Each entry: (track_index, clip_id, split_at_frame).
    pub split_actions: Vec<(usize, String, i64)>,
}

/// Like [`compute_ripple_insert`] but also detects straddling clips at the
/// insertion point and generates split actions so the editor can split them
/// before pushing. The push shifts already account for the right half of any
/// straddled clip starting at `insert_frame` after the split.
pub fn compute_ripple_insert_with_split(
    timeline: &Timeline,
    config: RippleInsertConfig,
) -> RippleInsertWithSplitOutcome {
    let insert_report = match compute_ripple_insert(timeline, config.clone()) {
        RippleInsertOutcome::Ok(report) => report,
        RippleInsertOutcome::Refused(msg) => {
            return RippleInsertWithSplitOutcome::Refused(msg);
        }
    };

    let mut split_actions: Vec<(usize, String, i64)> = Vec::new();
    for ti in &insert_report.push_track_indices {
        if let Some(straddler) = timeline.tracks[*ti]
            .clips
            .iter()
            .find(|c| c.start_frame < config.insert_frame && config.insert_frame < c.end_frame())
        {
            split_actions.push((*ti, straddler.id.clone(), config.insert_frame));
        }
    }

    RippleInsertWithSplitOutcome::Ok(RippleInsertWithSplitPlan {
        insert: insert_report,
        split_actions,
    })
}

/// Execute a [`RippleInsertWithSplitPlan`] on a timeline, mutating it in place.
///
/// 1. Splits any straddling clips at the insertion point.
/// 2. Shifts all pushed clips with `start_frame >= insert_frame` downstream
///    by `total_push` on every pushed track.
/// 3. Inserts the new clips into the gap on the target track.
///
/// Note: shifts are applied positionally (not by clip ID) because split
/// creates new clip IDs that the pre-computed plan cannot reference.
pub fn apply_ripple_insert_with_split(timeline: &mut Timeline, plan: RippleInsertWithSplitPlan) {
    let insert_frame = plan.insert.insert_frame;
    let total_push = plan.insert.total_push;
    let target_track = plan.insert.track_index;
    let push_tracks: std::collections::BTreeSet<usize> =
        plan.insert.push_track_indices.into_iter().collect();

    // 1. Execute split actions (must happen before positional shifts)
    for (_track_index, clip_id, at_frame) in &plan.split_actions {
        crate::edit::split_clip(timeline, clip_id, *at_frame);
    }

    // 2. Positional shift: push every clip with start_frame >= insert_frame
    //    by total_push on every pushed track.
    for ti in 0..timeline.tracks.len() {
        if !push_tracks.contains(&ti) {
            continue;
        }
        for clip in &mut timeline.tracks[ti].clips {
            if clip.start_frame >= insert_frame {
                clip.start_frame += total_push;
            }
        }
    }

    // 3. Construct and insert new Clip objects at the gap
    let clip_specs = plan.insert.clips;
    let created_ids = plan.insert.created_clip_ids;
    let mut new_clips: Vec<Clip> = Vec::with_capacity(clip_specs.len());
    let mut offset: i64 = 0;
    for (i, spec) in clip_specs.into_iter().enumerate() {
        let clip_id = created_ids
            .get(i)
            .cloned()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        new_clips.push(Clip {
            id: clip_id,
            media_ref: spec.asset_id,
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: insert_frame + offset,
            duration_frames: spec.duration_frames,
            trim_start_frame: spec.trim_start_frame.unwrap_or(0),
            trim_end_frame: spec.trim_end_frame.unwrap_or(0),
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
            opacity: 1.0,
            transform: core_model::Transform::default(),
            crop: core_model::Crop::default(),
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
        });
        offset += spec.duration_frames;
    }

    if target_track < timeline.tracks.len() {
        timeline.tracks[target_track].clips.extend(new_clips);
        timeline.tracks[target_track]
            .clips
            .sort_by_key(|c| c.start_frame);
    }
}

pub fn timing_propagation_partners(
    timeline: &Timeline,
    clip_ids: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for id in clip_ids {
        for pid in linked_partner_ids(timeline, id) {
            if !clip_ids.contains(&pid) {
                out.insert(pid);
            }
        }
    }
    out
}

pub fn compute_trim_values(clip: &Clip, edge: TrimEdge, delta: i64) -> (i64, i64) {
    let source_delta = ((delta as f64) * clip.speed).round() as i64;
    let unbounded = matches!(
        clip.media_type,
        core_model::ClipType::Image | core_model::ClipType::Text
    );
    match edge {
        TrimEdge::Left => {
            let new_start = clip.trim_start_frame + source_delta;
            (
                if unbounded {
                    new_start
                } else {
                    new_start.max(0)
                },
                clip.trim_end_frame,
            )
        }
        TrimEdge::Right => {
            let new_end = clip.trim_end_frame - source_delta;
            (
                clip.trim_start_frame,
                if unbounded { new_end } else { new_end.max(0) },
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimEdge {
    Left,
    Right,
}

fn find_clip(timeline: &Timeline, clip_id: &str) -> Option<(usize, usize)> {
    timeline
        .tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track
                .clips
                .iter()
                .position(|clip| clip.id == clip_id)
                .map(|clip_index| (track_index, clip_index))
        })
}
