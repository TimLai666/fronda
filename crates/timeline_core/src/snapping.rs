use crate::ClipMathExt;

pub const THRESHOLD_PIXELS: f64 = 8.0;
pub const STICKY_MULTIPLIER: f64 = 1.5;
pub const PLAYHEAD_MULTIPLIER: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapTargetKind {
    Playhead,
    ClipEdge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapTarget {
    pub frame: i64,
    pub kind: SnapTargetKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapResult {
    pub frame: i64,
    pub probe_offset: i64,
    pub x: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SnapState {
    pub currently_snapped_to: Option<i64>,
    pub current_probe_offset: i64,
}

pub fn collect_targets(
    tracks: &[core_model::Track],
    playhead_frame: i64,
    exclude_clip_ids: &[String],
    include_playhead: bool,
) -> Vec<SnapTarget> {
    let mut targets = Vec::new();
    if include_playhead {
        targets.push(SnapTarget {
            frame: playhead_frame,
            kind: SnapTargetKind::Playhead,
        });
    }
    let exclude: std::collections::BTreeSet<&str> =
        exclude_clip_ids.iter().map(|s| s.as_str()).collect();
    for track in tracks {
        for clip in &track.clips {
            if exclude.contains(clip.id.as_str()) {
                continue;
            }
            targets.push(SnapTarget {
                frame: clip.start_frame,
                kind: SnapTargetKind::ClipEdge,
            });
            targets.push(SnapTarget {
                frame: clip.end_frame(),
                kind: SnapTargetKind::ClipEdge,
            });
        }
    }
    targets.sort_by_key(|t| t.frame);
    targets
}

pub fn find_snap(
    position: i64,
    probe_offsets: &[i64],
    targets: &[SnapTarget],
    state: &mut SnapState,
    base_threshold: f64,
    pixels_per_frame: f64,
) -> Option<SnapResult> {
    let base_frame_threshold = if pixels_per_frame > 0.0 {
        base_threshold / pixels_per_frame
    } else {
        f64::INFINITY
    };

    if let Some(snapped) = state.currently_snapped_to {
        let hold_threshold = base_frame_threshold * STICKY_MULTIPLIER;
        let probe_pos = position + state.current_probe_offset;
        if (probe_pos as f64 - snapped as f64).abs() <= hold_threshold
            && targets.iter().any(|t| t.frame == snapped)
        {
            return Some(SnapResult {
                frame: snapped,
                probe_offset: state.current_probe_offset,
                x: snapped as f64 * pixels_per_frame,
            });
        }
        state.currently_snapped_to = None;
        state.current_probe_offset = 0;
    }

    let mut best: Option<(i64, &SnapTarget, f64)> = None;
    for probe_offset in probe_offsets {
        let probe_pos = position + probe_offset;
        for target in targets {
            let threshold = match target.kind {
                SnapTargetKind::Playhead => base_frame_threshold * PLAYHEAD_MULTIPLIER,
                SnapTargetKind::ClipEdge => base_frame_threshold,
            };
            let dist = (probe_pos as f64 - target.frame as f64).abs();
            if dist <= threshold
                && best
                    .as_ref()
                    .is_none_or(|(_, _, best_dist)| dist < *best_dist)
            {
                best = Some((*probe_offset, target, dist));
            }
        }
    }

    let (probe_offset, target, _) = best?;
    state.currently_snapped_to = Some(target.frame);
    state.current_probe_offset = probe_offset;
    Some(SnapResult {
        frame: target.frame,
        probe_offset,
        x: target.frame as f64 * pixels_per_frame,
    })
}

/// SNP-007: Validates that dragging a selection to a target start frame
/// does not push any clip past frame 0.
/// Returns true if the drag is valid (no clip crosses frame 0).
pub fn validate_drag_not_past_zero(
    selected_clip_starts: &[i64],
    target_start_frame: i64,
    current_start_frame: i64,
) -> bool {
    let delta = target_start_frame - current_start_frame;
    for &start in selected_clip_starts {
        let new_start = start + delta;
        if new_start < 0 {
            return false;
        }
    }
    true
}

/// SNP-007: Clamp a drag target so no selected clip crosses frame 0.
/// Returns the clamped target frame.
pub fn clamp_drag_to_frame_zero(
    selected_clip_starts: &[i64],
    target_start_frame: i64,
    current_start_frame: i64,
) -> i64 {
    let delta = target_start_frame - current_start_frame;
    let min_new_start = selected_clip_starts
        .iter()
        .map(|s| s + delta)
        .min()
        .unwrap_or(0);
    if min_new_start < 0 {
        let earliest_current = selected_clip_starts.iter().min().copied().unwrap_or(0);
        let clamped_delta = -earliest_current;
        current_start_frame + clamped_delta
    } else {
        target_start_frame
    }
}

/// SNP-008: Resolve a snap target for a cut/razor preview.
/// Uses the same target set and base threshold as drag operations,
/// but with a single probe offset at position 0 (cursor position).
/// Returns the snapped frame, if any.
pub fn resolve_cut_preview_snap(
    pointer_frame: i64,
    targets: &[SnapTarget],
    base_threshold: f64,
    pixels_per_frame: f64,
) -> Option<i64> {
    let frame_threshold = if pixels_per_frame > 0.0 {
        base_threshold / pixels_per_frame
    } else {
        f64::INFINITY
    };

    let mut best: Option<(i64, f64)> = None;
    for target in targets {
        let dist = (pointer_frame as f64 - target.frame as f64).abs();
        if dist <= frame_threshold
            && best
                .as_ref()
                .is_none_or(|(_, best_dist)| dist < *best_dist)
        {
            best = Some((target.frame, dist));
        }
    }
    best.map(|(frame, _)| frame)
}

// Convenience wrapper w/ default probe_offsets = [0]
pub fn find_snap_simple(
    position: i64,
    targets: &[SnapTarget],
    state: &mut SnapState,
    base_threshold: f64,
    pixels_per_frame: f64,
) -> Option<SnapResult> {
    find_snap(
        position,
        &[0],
        targets,
        state,
        base_threshold,
        pixels_per_frame,
    )
}
