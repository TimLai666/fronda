use crate::{
    linked_partner_ids,
    ripple::{
        compute_ripple_shifts_for_ranges, gap_is_still_empty, merge_ranges, validate_track_shifts,
        ClipShift, FrameRange,
    },
    ClipMathExt,
};
use core_model::{Clip, Timeline};
use std::collections::BTreeSet;

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
