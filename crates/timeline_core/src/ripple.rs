use core_model::{Clip, Track};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRange {
    pub start: i64,
    pub end: i64,
}

impl FrameRange {
    pub fn length(&self) -> i64 {
        self.end - self.start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GapSelection {
    pub track_index: usize,
    pub range: FrameRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipShift {
    pub clip_id: String,
    pub new_start_frame: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RippleValidationError {
    NegativeStart {
        clip_id: String,
    },
    Collision {
        leading_clip_id: String,
        trailing_clip_id: String,
    },
}

pub fn compute_ripple_shifts(clips: &[Clip], removed_ids: &BTreeSet<String>) -> Vec<ClipShift> {
    if removed_ids.is_empty() {
        return Vec::new();
    }

    let removed_ranges: Vec<FrameRange> = clips
        .iter()
        .filter(|clip| removed_ids.contains(&clip.id))
        .map(|clip| FrameRange {
            start: clip.start_frame,
            end: clip.start_frame + clip.duration_frames,
        })
        .collect();

    let survivors: Vec<Clip> = clips
        .iter()
        .filter(|clip| !removed_ids.contains(&clip.id))
        .cloned()
        .collect();

    compute_ripple_shifts_for_ranges(&survivors, &removed_ranges)
}

pub fn compute_ripple_shifts_for_ranges(
    clips: &[Clip],
    removed_ranges: &[FrameRange],
) -> Vec<ClipShift> {
    let merged = merge_ranges(removed_ranges);
    if merged.is_empty() {
        return Vec::new();
    }

    let mut ordered = clips.to_vec();
    ordered.sort_by_key(|clip| clip.start_frame);

    let mut shifts = Vec::new();
    for clip in ordered {
        let shift = merged
            .iter()
            .filter(|range| range.end <= clip.start_frame)
            .map(FrameRange::length)
            .sum::<i64>();
        if shift > 0 {
            shifts.push(ClipShift {
                clip_id: clip.id,
                new_start_frame: clip.start_frame - shift,
            });
        }
    }
    shifts
}

pub fn compute_ripple_push(
    clips: &[Clip],
    insert_frame: i64,
    push_amount: i64,
    exclude_ids: &BTreeSet<String>,
) -> Vec<ClipShift> {
    clips
        .iter()
        .filter(|clip| !exclude_ids.contains(&clip.id) && clip.start_frame >= insert_frame)
        .map(|clip| ClipShift {
            clip_id: clip.id.clone(),
            new_start_frame: clip.start_frame + push_amount,
        })
        .collect()
}

pub fn merge_ranges(ranges: &[FrameRange]) -> Vec<FrameRange> {
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|range| range.start);

    let mut merged: Vec<FrameRange> = Vec::new();
    for range in sorted {
        if let Some(last) = merged.last_mut() {
            if range.start <= last.end {
                last.end = last.end.max(range.end);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

pub fn validate_track_shifts(
    track: &Track,
    shifts: &[ClipShift],
) -> Result<(), RippleValidationError> {
    if shifts.is_empty() {
        return Ok(());
    }

    let shift_map: BTreeMap<&str, i64> = shifts
        .iter()
        .map(|shift| (shift.clip_id.as_str(), shift.new_start_frame))
        .collect();

    let mut intervals: Vec<(String, i64, i64)> = Vec::new();
    for clip in &track.clips {
        let start = shift_map
            .get(clip.id.as_str())
            .copied()
            .unwrap_or(clip.start_frame);
        if start < 0 {
            return Err(RippleValidationError::NegativeStart {
                clip_id: clip.id.clone(),
            });
        }
        intervals.push((clip.id.clone(), start, start + clip.duration_frames));
    }

    intervals.sort_by_key(|(_, start, _)| *start);
    for pair in intervals.windows(2) {
        let (leading_id, _, leading_end) = &pair[0];
        let (trailing_id, trailing_start, _) = &pair[1];
        if *trailing_start < *leading_end {
            return Err(RippleValidationError::Collision {
                leading_clip_id: leading_id.clone(),
                trailing_clip_id: trailing_id.clone(),
            });
        }
    }

    Ok(())
}

pub fn gap_is_still_empty(track: &Track, range: FrameRange) -> bool {
    !track.clips.iter().any(|clip| {
        clip.start_frame < range.end && clip.start_frame + clip.duration_frames > range.start
    })
}
