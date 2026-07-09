mod clip_clipboard;
mod clip_properties;
mod compound;
mod drag_payload;
mod edit;
mod inspector;
mod project_presets;
mod project_settings;
mod project_settings_guard;
pub use keyframes::{
    clamp_clip_fades_to_duration, clamp_clip_keyframes_to_duration, rescale_word_timings,
    sample_keyframe_track, set_clip_duration, split_all_clip_keyframe_tracks, split_keyframe_track,
};

mod keyframes;
mod linking;
mod overwrite;
mod range_selection;
mod ripple;
mod snapping;
mod track_ops;
mod word_cut;
mod workflow;

use core_model::{Clip, Timeline};

pub use clip_clipboard::{
    find_first_compatible_track, is_track_compatible, ClipClipboard, ClipClipboardEngine,
    CopiedClip, PasteError, PasteResult,
};
pub use clip_properties::{
    set_clip_properties, write_position, write_scale, ClipPropertyUpdate, PartialTransform,
    PropertyChanges,
};
pub use compound::{
    decompose_nest, flatten_nests, nest_clips, timeline_resolver, NestResult, NEST_MAX_DEPTH,
};
pub use drag_payload::{
    is_internal_drag_payload, parse_asset_segment, parse_drag_payload, DragItem, DragPayload,
};
pub use edit::{
    apply_clip_speed, clear_region, find_clip, link_audio_for_placed_clips, move_clips,
    place_clips, prune_empty_tracks, remove_clips, split_clip, ClipLocation,
};
pub use inspector::{
    clamp_crop_visibility, clip_at_point, constrain_crop_aspect, db_from_linear,
    fade_multiplier_at, fit_text_clip_to_content, format_aspect_ratio, format_duration,
    linear_from_db,
    resize_preserving_aspect, resize_text_font, resolved_crop_at, resolved_opacity_at,
    resolved_transform_at, resolved_volume_at, unrotate_crop_delta, AspectConstraint,
    VOLUME_CEILING_DB, VOLUME_FLOOR_DB,
};
pub use linking::{
    build_link_index, expand_to_link_group, link_clips, link_group_offsets, linked_partner_ids,
    partner_moves_for_move_of, unlink_clips, LinkIndex, LinkedPartnerMove,
};
pub use overwrite::{compute_overwrite, OverwriteAction};
pub use project_presets::{
    AspectPreset, QualityPreset, ZoomPreset, ASPECT_PRESETS, FPS_PRESETS, QUALITY_PRESETS,
    ZOOM_PRESETS,
};
pub use project_settings::{apply_fps, apply_settings, refit_transforms, FpsChangeReport};
pub use project_settings_guard::{
    is_settings_configured, ProjectSettingsGuard, SettingsGuardAction, SettingsMismatch,
};
pub use range_selection::{
    drag_range_edge, find_all_gaps, find_gap_at_frame, shift_drag_range, RangeEdge, TimelineRange,
};
pub use ripple::{
    compute_ripple_push, compute_ripple_shifts, compute_ripple_shifts_for_ranges,
    gap_is_still_empty, merge_ranges, validate_track_shifts, ClipShift, FrameRange, GapSelection,
    RippleValidationError,
};
pub use snapping::{
    clamp_drag_to_frame_zero, collect_targets, find_snap, find_snap_simple,
    resolve_cut_preview_snap, validate_drag_not_past_zero, SnapResult, SnapState, SnapTarget,
    SnapTargetKind, PLAYHEAD_MULTIPLIER, STICKY_MULTIPLIER, THRESHOLD_PIXELS,
};
pub use track_ops::{
    clamp_track_height, display_label_for_track, insert_track_at, remove_track,
    sort_clips_on_track, toggle_track_hidden, toggle_track_mute, toggle_track_sync_lock,
    TrackInsertionError, MAX_TRACK_HEIGHT, MIN_TRACK_HEIGHT,
};
pub use word_cut::{
    cut_ranges, map_word_stamps, ms_to_frames, plan_word_removal, span_frames,
    CutAggressiveness, PlannerWord, TimelineWord, WordRemovalPlan,
};
pub use workflow::{
    apply_ripple_insert_with_split, compute_ripple_delete, compute_ripple_delete_gap,
    compute_ripple_insert, compute_ripple_insert_with_split, compute_trim_values,
    timing_propagation_partners, ClipFragment, RippleDeleteConfig, RippleDeleteOutcome,
    RippleDeleteReport, RippleInsertClipSpec, RippleInsertConfig, RippleInsertOutcome,
    RippleInsertReport, RippleInsertWithSplitOutcome, RippleInsertWithSplitPlan, RippleShiftSet,
    TrimEdge,
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
