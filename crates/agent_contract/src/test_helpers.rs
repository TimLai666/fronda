use core_model::{Clip, ClipType, Crop, Interpolation, Transform};
use uuid::Uuid;

/// Create a minimal Video clip for testing.
pub fn make_clip(start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: Uuid::new_v4().to_string(),
        media_ref: "test-asset".to_string(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame,
        duration_frames,
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
    }
}
