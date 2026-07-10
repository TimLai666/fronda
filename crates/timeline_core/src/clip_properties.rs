use core_model::{Clip, Interpolation, KeyframeTrack, TextStyle, Transform};

// ---------------------------------------------------------------------------
// Upstream #114: Partial transform must carry forward missing fields.
// ---------------------------------------------------------------------------

/// A partial transform where `None` fields are carried forward from the clip.
#[derive(Debug, Clone, Copy, Default)]
pub struct PartialTransform {
    pub center_x: Option<f64>,
    pub center_y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub rotation: Option<f64>,
    pub flip_horizontal: Option<bool>,
    pub flip_vertical: Option<bool>,
}

impl PartialTransform {
    pub fn has_any_field(&self) -> bool {
        self.center_x.is_some()
            || self.center_y.is_some()
            || self.width.is_some()
            || self.height.is_some()
            || self.rotation.is_some()
            || self.flip_horizontal.is_some()
            || self.flip_vertical.is_some()
    }

    /// Merge into a full Transform, carrying forward missing fields from `base`.
    /// This is the upstream #114 fix: every field not in the input is preserved.
    pub fn merge_into(&self, base: &Transform) -> Transform {
        Transform {
            center_x: self.center_x.unwrap_or(base.center_x),
            center_y: self.center_y.unwrap_or(base.center_y),
            width: self.width.unwrap_or(base.width),
            height: self.height.unwrap_or(base.height),
            rotation: self.rotation.unwrap_or(base.rotation),
            flip_horizontal: self.flip_horizontal.unwrap_or(base.flip_horizontal),
            flip_vertical: self.flip_vertical.unwrap_or(base.flip_vertical),
        }
    }
}

// ---------------------------------------------------------------------------
// set_clip_properties equivalent
// ---------------------------------------------------------------------------

/// Result of applying property changes to a single clip.
#[derive(Debug, Clone, Default)]
pub struct PropertyChanges {
    pub changed: Vec<String>,
}

/// Apply property changes to a clip, returning which fields changed.
///
/// Core logic from the Swift `applyPropertyChanges`:
///   - `duration_frames` clamps keyframes/fades
///   - `speed` recomputes duration unless `duration_frames` is also set
///   - `volume` / `opacity` set the scalar and clear the keyframe track
///   - `transform` uses `PartialTransform::merge_into` (#114 fix)
///   - `content` / `font_name` etc. update text style fields
/// The set of clip properties an update may change. Every field is optional;
/// `None` leaves the property untouched. Replaces the old 12-positional-argument
/// signature of [`set_clip_properties`].
#[derive(Default)]
pub struct ClipPropertyUpdate<'a> {
    pub duration_frames: Option<i64>,
    pub trim_start_frame: Option<i64>,
    pub trim_end_frame: Option<i64>,
    pub speed: Option<f64>,
    pub volume: Option<f64>,
    pub opacity: Option<f64>,
    pub transform: Option<&'a PartialTransform>,
    // Text-clip style fields.
    pub content: Option<&'a str>,
    pub font_name: Option<&'a str>,
    pub font_size: Option<f64>,
    pub font_weight: Option<f64>,
    pub color: Option<core_model::TextRgba>,
    pub alignment: Option<core_model::TextAlignment>,
    pub background: Option<core_model::TextFill>,
    pub border: Option<core_model::TextFill>,
}

pub fn set_clip_properties(clip: &mut Clip, u: &ClipPropertyUpdate) -> PropertyChanges {
    let mut changed: Vec<String> = Vec::new();

    // Duration
    if let Some(d) = u.duration_frames {
        if d >= 1 {
            clip.duration_frames = d;
            crate::clamp_clip_keyframes_to_duration(clip);
            crate::clamp_clip_fades_to_duration(clip);
            changed.push("durationFrames".into());
        }
    }

    // Trim
    if let Some(v) = u.trim_start_frame {
        clip.trim_start_frame = v;
        changed.push("trimStartFrame".into());
    }
    if let Some(v) = u.trim_end_frame {
        clip.trim_end_frame = v;
        changed.push("trimEndFrame".into());
    }

    // Speed
    if let Some(s) = u.speed {
        if s > 0.0 && u.duration_frames.is_none() {
            // recompute duration from source coverage
            let source_consumed = (clip.duration_frames as f64 * clip.speed).round();
            clip.duration_frames = (source_consumed / s).round().max(1.0) as i64;
            crate::clamp_clip_keyframes_to_duration(clip);
            crate::clamp_clip_fades_to_duration(clip);
            changed.push("durationFrames".into());
        }
        clip.speed = s;
        changed.push("speed".into());
    }

    // Volume/opacity: scalar set clears keyframe track
    if let Some(v) = u.volume {
        clip.volume = v;
        clip.volume_track = None;
        changed.push("volume".into());
    }
    if let Some(v) = u.opacity {
        clip.opacity = v;
        clip.opacity_track = None;
        changed.push("opacity".into());
    }

    // Transform — upstream #114: carry forward missing fields
    if let Some(t) = u.transform {
        if t.has_any_field() {
            clip.transform = t.merge_into(&clip.transform);
            changed.push("transform".into());
        }
    }

    // Text fields
    if let Some(c) = u.content {
        clip.text_content = Some(c.to_string());
        changed.push("content".into());
    }
    if let Some(n) = u.font_name {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.font_name = n.to_string();
            changed.push("fontName".into());
        }
    }
    if let Some(s) = u.font_size {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.font_size = s;
            changed.push("fontSize".into());
        }
    }
    if let Some(w) = u.font_weight {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.font_weight = w;
            changed.push("fontWeight".into());
        }
    }
    if let Some(c) = u.color {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.color = c;
            changed.push("color".into());
        }
    }
    if let Some(a) = u.alignment {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.alignment = a;
            changed.push("alignment".into());
        }
    }
    if let Some(bg) = &u.background {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.background = bg.clone();
            changed.push("background".into());
        }
    }
    if let Some(bd) = &u.border {
        ensure_text_style(clip);
        if let Some(ref mut ts) = clip.text_style {
            ts.border = bd.clone();
            changed.push("border".into());
        }
    }

    PropertyChanges { changed }
}

/// Ensure a clip has a non-None text_style, filling defaults if needed.
fn ensure_text_style(clip: &mut Clip) {
    if clip.text_style.is_none() {
        clip.text_style = Some(TextStyle::default());
    }
}

// ---------------------------------------------------------------------------
// Upstream #115: writePosition must not corrupt static transform when
// position keyframes are active.
// ---------------------------------------------------------------------------

/// Write position into a clip at the given clip-relative frame.
///
/// When `position_track` has active keyframes, only keyframes are updated
/// and the static `transform.center_x/y` are left untouched.
/// This is the upstream #115 fix.
pub fn write_position(clip: &mut Clip, frame: i64, set_x: Option<f64>, set_y: Option<f64>) {
    // Determine effective top-left at the given frame.
    let (current_x, current_y) = top_left_at(clip, frame);
    let new_x = set_x.unwrap_or(current_x);
    let new_y = set_y.unwrap_or(current_y);

    let size = size_at(clip, frame);

    // Check if position track is active (non-empty).
    let track_is_active = clip
        .position_track
        .as_ref()
        .map(|t| !t.keyframes.is_empty())
        .unwrap_or(false);

    if track_is_active {
        // #115 fix: only update keyframes, leave static transform alone.
        let value = AnimPair { a: new_x, b: new_y };
        clip.position_track = Some(upsert_keyframe(clip.position_track.take(), frame, value));
    } else {
        // No keyframes → write directly to static transform.
        clip.transform.center_x = new_x + size.a / 2.0;
        clip.transform.center_y = new_y + size.b / 2.0;
    }
}

/// Write scale into a clip at the given clip-relative frame.
pub fn write_scale(clip: &mut Clip, frame: i64, new_scale: f64, source_aspect: f64) {
    let w = new_scale;
    let h = if source_aspect > 0.0 {
        new_scale / source_aspect
    } else {
        new_scale
    };

    if clip
        .scale_track
        .as_ref()
        .map(|t| !t.keyframes.is_empty())
        .unwrap_or(false)
    {
        let value = AnimPair { a: w, b: h };
        clip.scale_track = Some(upsert_keyframe(clip.scale_track.take(), frame, value));
    } else {
        clip.transform.width = w;
        clip.transform.height = h;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

use crate::keyframes::KeyframeValue;
use core_model::{AnimPair, Keyframe};

/// Effective top-left position of a clip at a given clip-relative frame,
/// accounting for keyframes.
fn top_left_at(clip: &Clip, frame: i64) -> (f64, f64) {
    if let Some(ref track) = clip.position_track {
        if !track.keyframes.is_empty() {
            let pos = crate::sample_keyframe_track(track, frame, AnimPair { a: 0.0, b: 0.0 });
            return (pos.a, pos.b);
        }
    }
    let t = clip.transform;
    let cx = t.center_x;
    let cy = t.center_y;
    let w = t.width;
    let h = t.height;
    (cx - w / 2.0, cy - h / 2.0)
}

/// Effective size of a clip at a given clip-relative frame.
fn size_at(clip: &Clip, frame: i64) -> AnimPair {
    if let Some(ref track) = clip.scale_track {
        if !track.keyframes.is_empty() {
            return crate::sample_keyframe_track(track, frame, AnimPair { a: 0.0, b: 0.0 });
        }
    }
    AnimPair {
        a: clip.transform.width,
        b: clip.transform.height,
    }
}

/// Upsert a keyframe at the given clip-relative frame.
/// If a keyframe already exists at that frame, replace its value.
/// Otherwise insert in sorted order.
fn upsert_keyframe<V>(track: Option<KeyframeTrack<V>>, frame: i64, value: V) -> KeyframeTrack<V>
where
    V: KeyframeValue,
{
    let mut track = track.unwrap_or(KeyframeTrack {
        keyframes: Vec::new(),
    });

    if let Some(existing) = track.keyframes.iter_mut().find(|k| k.frame == frame) {
        existing.value = value;
    } else {
        track.keyframes.push(Keyframe {
            frame,
            value,
            interpolation_out: Interpolation::Linear,
        });
        track.keyframes.sort_by_key(|k| k.frame);
    }

    track
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{ClipType, Crop, Interpolation, KeyframeTrack, Transform};

    fn make_clip() -> Clip {
        Clip {
            id: "test".into(),
            media_ref: "asset".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 100,
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
            blend_mode: Default::default(),
            chroma_key: None,
            multicam_group_id: None,
            text_animation: None,
            word_timings: None,
        }
    }

    // --- PartialTransform (#114) ---

    #[test]
    fn partial_transform_empty_returns_base() {
        let base = Transform {
            center_x: 0.3,
            center_y: 0.4,
            width: 0.5,
            height: 0.6,
            rotation: 45.0,
            flip_horizontal: true,
            flip_vertical: false,
        };
        let partial = PartialTransform::default();
        let result = partial.merge_into(&base);
        assert_eq!(result.center_x, 0.3);
        assert_eq!(result.center_y, 0.4);
        assert_eq!(result.width, 0.5);
        assert_eq!(result.height, 0.6);
        assert_eq!(result.rotation, 45.0);
        assert!(result.flip_horizontal);
        assert!(!result.flip_vertical);
    }

    #[test]
    fn partial_transform_merges_selected_fields() {
        let base = Transform::default();
        let partial = PartialTransform {
            center_x: Some(0.7),
            rotation: Some(90.0),
            ..Default::default()
        };
        let result = partial.merge_into(&base);
        assert_eq!(result.center_x, 0.7);
        assert_eq!(result.center_y, 0.5); // from base
        assert_eq!(result.rotation, 90.0);
        assert_eq!(result.width, 1.0); // from base
        assert_eq!(result.height, 1.0); // from base
        assert!(!result.flip_horizontal); // from base
    }

    // --- set_clip_properties ---

    #[test]
    fn set_clip_properties_duration_and_speed() {
        let mut clip = make_clip();
        let result = set_clip_properties(
            &mut clip,
            &ClipPropertyUpdate {
                duration_frames: Some(50),
                ..Default::default()
            },
        );
        assert_eq!(clip.duration_frames, 50);
        assert!(result.changed.contains(&"durationFrames".to_string()));
    }

    #[test]
    fn set_clip_properties_speed_recomputes_duration() {
        let mut clip = make_clip();
        clip.duration_frames = 100;
        clip.speed = 1.0;
        let result = set_clip_properties(
            &mut clip,
            &ClipPropertyUpdate {
                speed: Some(2.0),
                ..Default::default()
            },
        );
        assert_eq!(clip.speed, 2.0);
        // 100 * 1.0 / 2.0 = 50
        assert_eq!(clip.duration_frames, 50);
        assert!(result.changed.contains(&"speed".to_string()));
        assert!(result.changed.contains(&"durationFrames".to_string()));
    }

    #[test]
    fn set_clip_properties_volume_clears_track() {
        let mut clip = make_clip();
        clip.volume_track = Some(KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: 0.5,
                interpolation_out: Interpolation::Linear,
            }],
        });
        let result = set_clip_properties(
            &mut clip,
            &ClipPropertyUpdate {
                volume: Some(0.8),
                ..Default::default()
            },
        );
        assert_eq!(clip.volume, 0.8);
        assert!(clip.volume_track.is_none());
        assert!(result.changed.contains(&"volume".to_string()));
    }

    #[test]
    fn set_clip_properties_transform_preserves_rotation() {
        // #114: partial transform must carry forward rotation
        let mut clip = make_clip();
        clip.transform.rotation = 45.0;
        clip.transform.flip_horizontal = true;

        let partial = PartialTransform {
            center_x: Some(0.7),
            width: Some(0.8),
            ..Default::default()
        };
        let result = set_clip_properties(
            &mut clip,
            &ClipPropertyUpdate {
                transform: Some(&partial),
                ..Default::default()
            },
        );
        assert_eq!(clip.transform.center_x, 0.7);
        assert_eq!(clip.transform.width, 0.8);
        // These must NOT be zeroed:
        assert_eq!(clip.transform.rotation, 45.0);
        assert!(clip.transform.flip_horizontal);
        assert!(result.changed.contains(&"transform".to_string()));
    }

    #[test]
    fn set_clip_properties_text_fields() {
        let mut clip = make_clip();
        clip.media_type = ClipType::Text;
        clip.source_clip_type = ClipType::Text;
        let result = set_clip_properties(
            &mut clip,
            &ClipPropertyUpdate {
                content: Some("Hello"),
                font_name: Some("Helvetica"),
                font_size: Some(48.0),
                ..Default::default()
            },
        );
        assert_eq!(clip.text_content.as_deref(), Some("Hello"));
        assert_eq!(
            clip.text_style.as_ref().map(|s| &*s.font_name),
            Some("Helvetica")
        );
        assert_eq!(clip.text_style.as_ref().map(|s| s.font_size), Some(48.0));
        assert!(result.changed.contains(&"content".to_string()));
    }

    // --- writePosition (#115) ---

    #[test]
    fn write_position_no_keyframe_updates_static() {
        let mut clip = make_clip();
        clip.transform = Transform {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            ..Transform::default()
        };
        // Without any keyframes, write_position sets the static transform.
        write_position(&mut clip, 50, Some(0.3), Some(0.4));
        // top-left (0.3, 0.4) + size/2 → center (0.8, 0.9)
        assert!((clip.transform.center_x - 0.8).abs() < 1e-9);
        assert!((clip.transform.center_y - 0.9).abs() < 1e-9);
    }

    #[test]
    fn write_position_with_active_keyframe_writes_keyframe_only() {
        // #115 fix: with active position track, only keyframes are updated.
        let mut clip = make_clip();
        clip.position_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: AnimPair { a: 0.0, b: 0.0 },
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: AnimPair { a: 1.0, b: 1.0 },
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        clip.transform = Transform {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            ..Transform::default()
        };

        write_position(&mut clip, 50, Some(0.4), Some(0.6));

        // Static transform must be UNCHANGED:
        assert_eq!(clip.transform.center_x, 0.5);
        assert_eq!(clip.transform.center_y, 0.5);

        // A keyframe should have been upserted at frame 50:
        let track = clip.position_track.as_ref().unwrap();
        let kf = track.keyframes.iter().find(|k| k.frame == 50).unwrap();
        assert!((kf.value.a - 0.4).abs() < 1e-9);
        assert!((kf.value.b - 0.6).abs() < 1e-9);
    }

    #[test]
    fn write_position_empty_track_is_inactive() {
        // An empty track should behave as inactive → write to static transform.
        let mut clip = make_clip();
        clip.position_track = Some(KeyframeTrack { keyframes: vec![] });
        clip.transform = Transform::default();

        write_position(&mut clip, 50, Some(0.2), Some(0.3));
        // top-left (0.2, 0.3) + size(1.0,1.0)/2 → center (0.7, 0.8)
        assert!((clip.transform.center_x - 0.7).abs() < 1e-9);
        assert!((clip.transform.center_y - 0.8).abs() < 1e-9);
    }

    // --- writeScale ---

    #[test]
    fn write_scale_no_keyframe_updates_static() {
        let mut clip = make_clip();
        write_scale(&mut clip, 50, 0.8, 16.0 / 9.0);
        assert!((clip.transform.width - 0.8).abs() < 1e-9);
        let expected_h = 0.8 / (16.0 / 9.0);
        assert!((clip.transform.height - expected_h).abs() < 1e-9);
    }

    #[test]
    fn write_scale_with_active_keyframe_writes_keyframe_only() {
        let mut clip = make_clip();
        clip.scale_track = Some(KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: AnimPair { a: 0.5, b: 0.5 },
                interpolation_out: Interpolation::Linear,
            }],
        });
        clip.transform = Transform::default();

        write_scale(&mut clip, 50, 0.9, 16.0 / 9.0);
        assert_eq!(clip.transform.width, 1.0); // unchanged
        assert_eq!(clip.transform.height, 1.0); // unchanged

        let track = clip.scale_track.as_ref().unwrap();
        let kf = track.keyframes.iter().find(|k| k.frame == 50).unwrap();
        assert!((kf.value.a - 0.9).abs() < 1e-9);
    }
}
