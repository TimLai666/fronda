mod edit;
pub use keyframes::{
    clamp_clip_fades_to_duration, clamp_clip_keyframes_to_duration, sample_keyframe_track,
    set_clip_duration, split_all_clip_keyframe_tracks, split_keyframe_track,
};

mod keyframes;
mod linking;
mod overwrite;
mod range_selection;
mod ripple;
mod snapping;
mod track_ops;
mod workflow;

use core_model::{Clip, Timeline};

pub use edit::{
    apply_clip_speed, clear_region, find_clip, prune_empty_tracks, split_clip, ClipLocation,
};
pub use linking::{
    build_link_index, expand_to_link_group, link_clips, link_group_offsets, linked_partner_ids,
    partner_moves_for_move_of, unlink_clips, LinkIndex, LinkedPartnerMove,
};
pub use overwrite::{compute_overwrite, OverwriteAction};
pub use range_selection::TimelineRange;
pub use ripple::{
    compute_ripple_push, compute_ripple_shifts, compute_ripple_shifts_for_ranges,
    gap_is_still_empty, merge_ranges, validate_track_shifts, ClipShift, FrameRange, GapSelection,
    RippleValidationError,
};
pub use snapping::{
    collect_targets, find_snap, find_snap_simple, SnapResult, SnapState, SnapTarget,
    SnapTargetKind, PLAYHEAD_MULTIPLIER, STICKY_MULTIPLIER, THRESHOLD_PIXELS,
};
pub use track_ops::{
    display_label_for_track, insert_track_at, remove_track, sort_clips_on_track,
    TrackInsertionError,
};
pub use workflow::{
    compute_ripple_delete, compute_ripple_delete_gap, compute_ripple_insert, compute_trim_values,
    timing_propagation_partners, ClipFragment, RippleDeleteConfig, RippleDeleteOutcome,
    RippleDeleteReport, RippleInsertClipSpec, RippleInsertConfig, RippleInsertOutcome,
    RippleInsertReport, RippleShiftSet, TrimEdge,
};

pub trait ClipMathExt {
    fn end_frame(&self) -> i64;
    fn source_frames_consumed(&self) -> i64;
    fn source_duration_frames(&self) -> i64;
    fn contains_frame(&self, frame: i64) -> bool;
}

impl ClipMathExt for Clip {
    fn end_frame(&self) -> i64 {
        self.start_frame + self.duration_frames
    }

    fn source_frames_consumed(&self) -> i64 {
        ((self.duration_frames as f64) * self.speed).round() as i64
    }

    fn source_duration_frames(&self) -> i64 {
        self.source_frames_consumed() + self.trim_start_frame + self.trim_end_frame
    }

    fn contains_frame(&self, frame: i64) -> bool {
        frame >= self.start_frame && frame < self.end_frame()
    }
}

pub trait TimelineMathExt {
    fn total_frames(&self) -> i64;
    fn clamp_seek_frame(&self, frame: i64) -> i64;
}

impl TimelineMathExt for Timeline {
    fn total_frames(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(|track| track.clips.iter())
            .map(|clip| clip.end_frame())
            .max()
            .unwrap_or(0)
    }

    fn clamp_seek_frame(&self, frame: i64) -> i64 {
        frame.clamp(0, self.total_frames())
    }
}

pub fn is_valid_half_open_range(start_frame: i64, end_frame: i64) -> bool {
    end_frame > start_frame
}
