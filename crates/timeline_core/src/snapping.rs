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

#[derive(Debug, Clone, PartialEq)]
pub struct SnapState {
    pub currently_snapped_to: Option<i64>,
    pub current_probe_offset: i64,
}

impl Default for SnapState {
    fn default() -> Self {
        Self {
            currently_snapped_to: None,
            current_probe_offset: 0,
        }
    }
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
                    .map_or(true, |(_, _, best_dist)| dist < *best_dist)
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
