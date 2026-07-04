use crate::keyframes::KeyframeValue;
use core_model::{AnimPair, Clip, Crop, Interpolation, KeyframeTrack, Transform};
use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// INS-001 – Transform normalisation invariants (test-only, checked via
//           proptest on serde round-trips and Default impl).
// ---------------------------------------------------------------------------

/// INS-002: Resolve the effective transform at a clip-relative frame.
///
/// When a keyframe track is non-empty its interpolated value replaces the
/// corresponding static field.  Tracks that are absent or empty leave the
/// static value untouched.
pub fn resolved_transform_at(clip: &Clip, frame: i64) -> Transform {
    let mut t = clip.transform;

    if let Some(ref track) = clip.position_track {
        if !track.keyframes.is_empty() {
            let pos = sample_with_fallback(
                track,
                frame,
                AnimPair {
                    a: t.center_x,
                    b: t.center_y,
                },
            );
            t.center_x = pos.a;
            t.center_y = pos.b;
        }
    }

    if let Some(ref track) = clip.scale_track {
        if !track.keyframes.is_empty() {
            let s = sample_with_fallback(
                track,
                frame,
                AnimPair {
                    a: t.width,
                    b: t.height,
                },
            );
            t.width = s.a;
            t.height = s.b;
        }
    }

    if let Some(ref track) = clip.rotation_track {
        if !track.keyframes.is_empty() {
            t.rotation = sample_with_fallback(track, frame, t.rotation);
        }
    }

    t
}

/// INS-002 (crop track): Resolve the effective crop at a clip-relative frame.
///
/// When the crop keyframe track is non-empty the interpolated value
/// replaces the static crop.
pub fn resolved_crop_at(clip: &Clip, frame: i64) -> Crop {
    match clip.crop_track {
        Some(ref track) if !track.keyframes.is_empty() => {
            sample_with_fallback(track, frame, clip.crop)
        }
        _ => clip.crop,
    }
}

/// INS-002 (opacity track): Resolve the effective opacity at a clip-relative frame.
///
/// When the opacity keyframe track is non-empty the interpolated value
/// replaces the static opacity.
pub fn resolved_opacity_at(clip: &Clip, frame: i64) -> f64 {
    match clip.opacity_track {
        Some(ref track) if !track.keyframes.is_empty() => {
            sample_with_fallback(track, frame, clip.opacity)
        }
        _ => clip.opacity,
    }
}

/// Fade-in/out multiplier (`0..=1`) at clip-relative `frame`, from the clip's
/// `fade_in_frames` / `fade_out_frames`. Returns 1.0 where no fade applies.
/// Multiply this into `resolved_opacity_at` for the effective opacity.
///
/// Mirrors Swift `Timeline.fadeMultiplier(at:)` exactly: each ramp is `t` (or
/// `smoothstep(t)` for a `.smooth` interpolation), and the two ramps combine by
/// `min` — so overlapping head/tail fades on a short clip take the deeper of the
/// two rather than double-attenuating.
pub fn fade_multiplier_at(clip: &Clip, frame: i64) -> f64 {
    let dur = clip.duration_frames;
    if dur <= 0 {
        return 1.0;
    }
    if frame < 0 || frame > dur {
        return 0.0;
    }
    let in_mul = if clip.fade_in_frames > 0 {
        let t = (frame as f64 / clip.fade_in_frames as f64).min(1.0);
        if clip.fade_in_interpolation == Interpolation::Smooth {
            smoothstep(t)
        } else {
            t
        }
    } else {
        1.0
    };
    let out_rem = dur - frame;
    let out_mul = if clip.fade_out_frames > 0 {
        let t = (out_rem as f64 / clip.fade_out_frames as f64).min(1.0);
        if clip.fade_out_interpolation == Interpolation::Smooth {
            smoothstep(t)
        } else {
            t
        }
    } else {
        1.0
    };
    in_mul.min(out_mul)
}

/// `t*t*(3-2t)` easing, matching Swift `smoothstep` (Keyframe.swift).
fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// INS-002 (volume track): Resolve the effective volume at a clip-relative frame.
///
/// When the volume keyframe track is non-empty the interpolated value
/// replaces the static volume.
pub fn resolved_volume_at(clip: &Clip, frame: i64) -> f64 {
    match clip.volume_track {
        Some(ref track) if !track.keyframes.is_empty() => {
            sample_with_fallback(track, frame, clip.volume)
        }
        _ => clip.volume,
    }
}

/// INS-006: Rotate a crop-edge drag delta from screen space into crop space.
///
/// When the clip is rotated by `rotation_degrees`, pointer deltas need to be
/// un-rotated before being applied to crop inset values.
pub fn unrotate_crop_delta(delta: AnimPair, rotation_degrees: f64) -> AnimPair {
    let rad = rotation_degrees * PI / 180.0;
    let cos = rad.cos();
    let sin = rad.sin();
    AnimPair {
        a: delta.a * cos + delta.b * sin,
        b: -delta.a * sin + delta.b * cos,
    }
}

/// What aspect ratio constraint to apply during crop interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AspectConstraint {
    Free,
    Original,
    Preset(f64),
}

/// INS-007: Constrain crop insets to an aspect ratio.
///
/// `source_aspect` = source_width / source_height.
/// Adjusts both axes to match the target ratio while respecting canvas bounds.
pub fn constrain_crop_aspect(
    crop: &Crop,
    source_aspect: f64,
    constraint: AspectConstraint,
) -> Crop {
    let ratio = match constraint {
        AspectConstraint::Free => return *crop,
        AspectConstraint::Original => source_aspect,
        AspectConstraint::Preset(r) => r,
    };

    if ratio <= 0.0 {
        return *crop;
    }

    let visible_w = 1.0 - crop.left - crop.right;
    let visible_h = 1.0 - crop.top - crop.bottom;

    if visible_w <= 0.0 || visible_h <= 0.0 {
        return *crop;
    }

    let current_aspect = visible_w / visible_h;

    if (current_aspect - ratio).abs() < 1e-9 {
        return *crop;
    }

    // Try width-first: target_w = visible_h * ratio
    let target_w = visible_h * ratio;
    let max_w = 1.0 - crop.left;
    if target_w <= max_w {
        return Crop {
            left: crop.left,
            top: crop.top,
            right: 1.0 - crop.left - target_w,
            bottom: crop.bottom,
        };
    }

    // Width won't fit — adjust height instead: target_h = visible_w / ratio
    let target_h = visible_w / ratio;
    let max_h = 1.0 - crop.top;
    let target_h = target_h.clamp(0.0, max_h);
    Crop {
        left: crop.left,
        top: crop.top,
        right: crop.right,
        bottom: 1.0 - crop.top - target_h,
    }
}

/// INS-008: Enforce a minimum visible fraction on every crop axis.
///
/// `min_visible` is typically 0.05 (5 %).  Each inset is clamped so the
/// resulting visible width and height are at least `min_visible`.
pub fn clamp_crop_visibility(crop: &Crop, min_visible: f64) -> Crop {
    let min_v = min_visible.clamp(0.0, 1.0);

    let max_inset = 1.0 - min_v;

    Crop {
        left: crop.left.clamp(0.0, max_inset),
        top: crop.top.clamp(0.0, max_inset),
        right: crop.right.clamp(0.0, max_inset),
        bottom: crop.bottom.clamp(0.0, max_inset),
    }
}

/// INS-009: Resize a clip while preserving the source aspect ratio.
///
/// `source_aspect` = source_width / source_height.  The returned transform
/// fits within the requested `new_width × new_height` canvas box.
pub fn resize_preserving_aspect(
    transform: &Transform,
    new_width: f64,
    new_height: f64,
    source_aspect: f64,
) -> Transform {
    if source_aspect <= 0.0 || new_width <= 0.0 || new_height <= 0.0 {
        return *transform;
    }

    let target_aspect = new_width / new_height;
    let (fit_w, fit_h) = if target_aspect > source_aspect {
        // constrained by height
        let h = new_height;
        let w = h * source_aspect;
        (w, h)
    } else {
        // constrained by width
        let w = new_width;
        let h = w / source_aspect;
        (w, h)
    };

    Transform {
        center_x: transform.center_x,
        center_y: transform.center_y,
        width: fit_w / new_width,
        height: fit_h / new_height,
        rotation: transform.rotation,
        flip_horizontal: transform.flip_horizontal,
        flip_vertical: transform.flip_vertical,
    }
}

/// INS-010: Scale a text clip's font when its bounding box is resized.
///
/// Preserves the proportional relationship between font size and box size.
pub fn resize_text_font(
    style: &core_model::TextStyle,
    old_box_width: f64,
    old_box_height: f64,
    new_box_width: f64,
    new_box_height: f64,
) -> f64 {
    let old_area = old_box_width * old_box_height;
    let new_area = new_box_width * new_box_height;
    if old_area <= 0.0 {
        return style.font_scale;
    }
    let area_ratio = new_area / old_area;
    // Scale font_scale by the square root of area ratio (proportional to linear
    // dimension).
    style.font_scale * area_ratio.sqrt()
}

/// INS-011: Fit a text clip's transform to its rendered content.
///
/// Updates the clip's transform width/height to match the content size,
/// then adjusts horizontal anchor based on text alignment.
pub fn fit_text_clip_to_content(
    clip: &mut Clip,
    content_width: f64,
    content_height: f64,
    canvas_width: f64,
    canvas_height: f64,
) {
    if canvas_width <= 0.0 || canvas_height <= 0.0 || content_width <= 0.0 || content_height <= 0.0
    {
        return;
    }

    let new_w = content_width / canvas_width;
    let new_h = content_height / canvas_height;

    clip.transform.width = new_w;
    clip.transform.height = new_h;

    // Adjust horizontal anchor for text alignment.
    if let Some(ref style) = clip.text_style {
        let anchor_offset = match style.alignment {
            core_model::TextAlignment::Left => 0.0,
            core_model::TextAlignment::Center => (1.0 - new_w) / 2.0,
            core_model::TextAlignment::Right => 1.0 - new_w,
        };
        clip.transform.center_x = anchor_offset + new_w / 2.0;
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn sample_with_fallback<V>(track: &KeyframeTrack<V>, frame: i64, fallback: V) -> V
where
    V: KeyframeValue,
{
    if track.keyframes.is_empty() {
        return fallback;
    }
    // delegate to the shared sampler in keyframes module
    crate::sample_keyframe_track(track, frame, fallback)
}

// ---------------------------------------------------------------------------
// Metadata / value formatting (Swift: InspectorView helpers, VolumeScale)
// ---------------------------------------------------------------------------

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Reduced aspect ratio string, e.g. `format_aspect_ratio(1920, 1080) == "16:9"`.
/// Non-positive inputs yield `"0:0"`. Mirrors Swift `formatAspectRatio`.
pub fn format_aspect_ratio(width: i64, height: i64) -> String {
    if width <= 0 || height <= 0 {
        return "0:0".to_string();
    }
    let g = gcd(width, height);
    format!("{}:{}", width / g, height / g)
}

/// Duration as `h:mm:ss` (with hours) or `m:ss`. Mirrors Swift `formatDuration`.
pub fn format_duration(seconds: f64) -> String {
    let total = (seconds.round() as i64).max(0);
    let hours = total / 3600;
    let mins = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

/// Topmost clip whose transformed rect contains a point (upstream #191, preview
/// double-click select). `clips` are `(id, transform)` in top-to-bottom render
/// order; `point` is in normalized canvas coords (0..1). Hit-testing is done in
/// pixel space (aspect-correct) and accounts for each clip's rotation. Returns
/// the first (topmost) clip that contains the point.
pub fn clip_at_point(
    clips: &[(String, Transform)],
    point: (f64, f64),
    canvas_width: f64,
    canvas_height: f64,
) -> Option<String> {
    let px = point.0 * canvas_width;
    let py = point.1 * canvas_height;
    for (id, t) in clips {
        let cx = t.center_x * canvas_width;
        let cy = t.center_y * canvas_height;
        let hw = (t.width * canvas_width).abs() / 2.0;
        let hh = (t.height * canvas_height).abs() / 2.0;
        // Un-rotate the point into the clip's local (axis-aligned) frame.
        let rad = -t.rotation * PI / 180.0;
        let (s, c) = (rad.sin(), rad.cos());
        let dx = px - cx;
        let dy = py - cy;
        let lx = dx * c - dy * s;
        let ly = dx * s + dy * c;
        if lx.abs() <= hw && ly.abs() <= hh {
            return Some(id.clone());
        }
    }
    None
}

/// Volume-slider floor (hard mute, rendered as "-∞ dB"). Swift: `VolumeScale.floorDb`.
pub const VOLUME_FLOOR_DB: f64 = -60.0;
/// Volume-slider ceiling. Swift: `VolumeScale.ceilingDb`.
pub const VOLUME_CEILING_DB: f64 = 15.0;

/// Linear amplitude → dB, clamped to [floor, ceiling]; `<= 0` linear → floor.
/// Mirrors Swift `VolumeScale.dbFromLinear`.
pub fn db_from_linear(linear: f64) -> f64 {
    if linear <= 0.0 {
        return VOLUME_FLOOR_DB;
    }
    (20.0 * linear.log10()).clamp(VOLUME_FLOOR_DB, VOLUME_CEILING_DB)
}

/// dB → linear amplitude; at/below the floor → true 0 (hard mute).
/// Mirrors Swift `VolumeScale.linearFromDb`.
pub fn linear_from_db(db: f64) -> f64 {
    if db <= VOLUME_FLOOR_DB {
        return 0.0;
    }
    10f64.powf(db.min(VOLUME_CEILING_DB) / 20.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Transform};

    #[test]
    fn format_aspect_ratio_reduces() {
        assert_eq!(format_aspect_ratio(1920, 1080), "16:9");
        assert_eq!(format_aspect_ratio(1024, 1024), "1:1");
        assert_eq!(format_aspect_ratio(1080, 1920), "9:16");
        assert_eq!(format_aspect_ratio(0, 1080), "0:0");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(3661.0), "1:01:01");
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(9.4), "0:09");
        assert_eq!(format_duration(0.0), "0:00");
    }

    #[test]
    fn clip_at_point_topmost_and_rotation() {
        let full = Transform::from_top_left(0.0, 0.0, 1.0, 1.0); // covers whole canvas
        let quarter = Transform {
            center_x: 0.5,
            center_y: 0.5,
            width: 0.5,
            height: 0.5,
            rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        };
        // Topmost (first) clip containing the point wins.
        let clips = vec![("top".to_string(), quarter), ("bg".to_string(), full)];
        assert_eq!(
            clip_at_point(&clips, (0.5, 0.5), 1920.0, 1080.0).as_deref(),
            Some("top")
        );
        // A point outside the small clip falls through to the background.
        assert_eq!(
            clip_at_point(&clips, (0.05, 0.05), 1920.0, 1080.0).as_deref(),
            Some("bg")
        );
        // A point outside every clip → None.
        let only = vec![("q".to_string(), quarter)];
        assert_eq!(clip_at_point(&only, (0.1, 0.1), 1920.0, 1080.0), None);

        // Rotation: a wide clip rotated 90° becomes tall — a point above/below
        // center (in the rotated extent) hits it, one to the side does not.
        let wide = Transform {
            center_x: 0.5,
            center_y: 0.5,
            width: 0.8,
            height: 0.2,
            rotation: 90.0,
            flip_horizontal: false,
            flip_vertical: false,
        };
        let rot = vec![("r".to_string(), wide)];
        // On a square canvas the rotated rect spans tall; (0.5, 0.15) is inside.
        assert_eq!(
            clip_at_point(&rot, (0.5, 0.15), 1000.0, 1000.0).as_deref(),
            Some("r")
        );
        // A point far to the side is outside the (now narrow) rotated width.
        assert_eq!(clip_at_point(&rot, (0.85, 0.5), 1000.0, 1000.0), None);
    }

    #[test]
    fn volume_db_linear_round_trip_and_floor() {
        assert!((db_from_linear(1.0) - 0.0).abs() < 1e-9);
        assert_eq!(db_from_linear(0.0), VOLUME_FLOOR_DB);
        assert_eq!(linear_from_db(VOLUME_FLOOR_DB), 0.0);
        // -6 dB ≈ 0.501 linear; round-trip within tolerance.
        let l = linear_from_db(-6.0);
        assert!((db_from_linear(l) - (-6.0)).abs() < 1e-6);
        // Ceiling clamp.
        assert_eq!(db_from_linear(100.0), VOLUME_CEILING_DB);
    }

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
        }
    }

    // INS-001: Transform defaults are normalised canvas-space values.
    #[test]
    fn ins_001_default_transform_is_normalised() {
        let t = Transform::default();
        assert_eq!(t.center_x, 0.5);
        assert_eq!(t.center_y, 0.5);
        assert_eq!(t.width, 1.0);
        assert_eq!(t.height, 1.0);
        assert_eq!(t.rotation, 0.0);
        assert!(!t.flip_horizontal);
        assert!(!t.flip_vertical);
    }

    // INS-002: Static transform returned when no keyframe tracks.
    #[test]
    fn ins_002_resolved_transform_no_keyframes() {
        let clip = make_clip();
        let resolved = resolved_transform_at(&clip, 50);
        assert_eq!(resolved.center_x, 0.5);
        assert_eq!(resolved.center_y, 0.5);
        assert_eq!(resolved.width, 1.0);
        assert_eq!(resolved.height, 1.0);
        assert_eq!(resolved.rotation, 0.0);
    }

    // INS-002: Position keyframes override static centre.
    #[test]
    fn ins_002_resolved_transform_position_keyframe() {
        let mut clip = make_clip();
        clip.position_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: AnimPair { a: 0.1, b: 0.2 },
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: AnimPair { a: 0.9, b: 0.8 },
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        let resolved = resolved_transform_at(&clip, 50);
        assert!((resolved.center_x - 0.5).abs() < 1e-9);
        assert!((resolved.center_y - 0.5).abs() < 1e-9);
    }

    // INS-002: Scale keyframes override static size.
    #[test]
    fn ins_002_resolved_transform_scale_keyframe() {
        let mut clip = make_clip();
        clip.scale_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: AnimPair { a: 0.5, b: 0.5 },
                    interpolation_out: Interpolation::Hold,
                },
                Keyframe {
                    frame: 100,
                    value: AnimPair { a: 1.0, b: 1.0 },
                    interpolation_out: Interpolation::Hold,
                },
            ],
        });
        let resolved = resolved_transform_at(&clip, 0);
        assert!((resolved.width - 0.5).abs() < 1e-9);
        assert!((resolved.height - 0.5).abs() < 1e-9);
        let resolved2 = resolved_transform_at(&clip, 50);
        assert!((resolved2.width - 0.5).abs() < 1e-9); // hold
    }

    // INS-002 (opacity track): Static opacity returned when no track.
    #[test]
    fn ins_002_resolved_opacity_no_track() {
        let mut clip = make_clip();
        clip.opacity = 0.7;
        assert!((resolved_opacity_at(&clip, 50) - 0.7).abs() < 1e-9);
    }

    // INS-002 (opacity track): Exact-frame keyframe value overrides static.
    #[test]
    fn ins_002_resolved_opacity_exact_frame() {
        let mut clip = make_clip();
        clip.opacity = 1.0;
        clip.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.2,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: 0.8,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        assert!((resolved_opacity_at(&clip, 0) - 0.2).abs() < 1e-9);
        assert!((resolved_opacity_at(&clip, 100) - 0.8).abs() < 1e-9);
    }

    // INS-002 (opacity track): Linear interpolation at the midpoint.
    #[test]
    fn ins_002_resolved_opacity_interpolated_midpoint() {
        let mut clip = make_clip();
        clip.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.2,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: 0.8,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        assert!((resolved_opacity_at(&clip, 50) - 0.5).abs() < 1e-9);
    }

    // INS-002 (volume track): Static volume returned when no track.
    #[test]
    fn ins_002_resolved_volume_no_track() {
        let mut clip = make_clip();
        clip.volume = 0.6;
        assert!((resolved_volume_at(&clip, 50) - 0.6).abs() < 1e-9);
    }

    // INS-002 (volume track): Exact-frame keyframe value overrides static.
    #[test]
    fn ins_002_resolved_volume_exact_frame() {
        let mut clip = make_clip();
        clip.volume = 1.0;
        clip.volume_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.4,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: 1.0,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        assert!((resolved_volume_at(&clip, 0) - 0.4).abs() < 1e-9);
        assert!((resolved_volume_at(&clip, 100) - 1.0).abs() < 1e-9);
    }

    // INS-002 (volume track): Linear interpolation at the midpoint.
    #[test]
    fn ins_002_resolved_volume_interpolated_midpoint() {
        let mut clip = make_clip();
        clip.volume_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.4,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: 1.0,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        assert!((resolved_volume_at(&clip, 50) - 0.7).abs() < 1e-9);
    }

    // INS-003: Position keyframes store top-left via AnimPair.
    #[test]
    fn ins_003_position_keyframe_top_left() {
        let pos = AnimPair { a: 0.3, b: 0.4 };
        let track = KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: pos,
                interpolation_out: Interpolation::Linear,
            }],
        };
        assert!((track.keyframes[0].value.a - 0.3).abs() < 1e-9);
        assert!((track.keyframes[0].value.b - 0.4).abs() < 1e-9);
    }

    // INS-004: Scale keyframes store normalised w/h via AnimPair.
    #[test]
    fn ins_004_scale_keyframe_normalised() {
        let s = AnimPair { a: 0.8, b: 0.6 };
        let track = KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: s,
                interpolation_out: Interpolation::Linear,
            }],
        };
        assert!((track.keyframes[0].value.a - 0.8).abs() < 1e-9);
        assert!((track.keyframes[0].value.b - 0.6).abs() < 1e-9);
    }

    // INS-005: Crop defaults to zero (no crop).
    #[test]
    fn ins_005_crop_default_zero() {
        let c = Crop::default();
        assert_eq!(c.left, 0.0);
        assert_eq!(c.top, 0.0);
        assert_eq!(c.right, 0.0);
        assert_eq!(c.bottom, 0.0);
    }

    // INS-006: Un-rotating a pure-horizontal delta at 90° maps it vertical.
    #[test]
    fn ins_006_unrotate_crop_delta_90deg() {
        let delta = AnimPair { a: 10.0, b: 0.0 };
        let result = unrotate_crop_delta(delta, 90.0);
        assert!((result.a - 0.0).abs() < 1e-9);
        assert!((result.b - (-10.0)).abs() < 1e-9);
    }

    #[test]
    fn ins_006_unrotate_crop_delta_45deg() {
        let delta = AnimPair { a: 10.0, b: 0.0 };
        let result = unrotate_crop_delta(delta, 45.0);
        let expected = 10.0 * (45.0_f64 * PI / 180.0).cos();
        assert!((result.a - expected).abs() < 1e-9);
        assert!((result.b - (-expected)).abs() < 1e-9);
    }

    #[test]
    fn ins_006_unrotate_zero_rotation_passthrough() {
        let delta = AnimPair { a: 10.0, b: 5.0 };
        let result = unrotate_crop_delta(delta, 0.0);
        assert!((result.a - 10.0).abs() < 1e-9);
        assert!((result.b - 5.0).abs() < 1e-9);
    }

    // INS-007: Free constraint is passthrough.
    #[test]
    fn ins_007_aspect_free_passthrough() {
        let c = Crop {
            left: 0.1,
            top: 0.1,
            right: 0.2,
            bottom: 0.2,
        };
        let result = constrain_crop_aspect(&c, 16.0 / 9.0, AspectConstraint::Free);
        assert_eq!(result.left, c.left);
        assert_eq!(result.right, c.right);
    }

    // INS-007: Original aspect adjusts width to match source ratio.
    #[test]
    fn ins_007_aspect_original() {
        // source 16:9, crop 10 % all sides => visible 80 % × 80 % → aspect 1.0
        let c = Crop {
            left: 0.1,
            top: 0.1,
            right: 0.1,
            bottom: 0.1,
        };
        let result = constrain_crop_aspect(&c, 16.0 / 9.0, AspectConstraint::Original);
        let visible_w = 1.0 - result.left - result.right;
        let visible_h = 1.0 - result.top - result.bottom;
        let ratio = visible_w / visible_h;
        assert!((ratio - 16.0 / 9.0).abs() < 1e-6);
    }

    // INS-008: Minimum visibility clamps insets.
    #[test]
    fn ins_008_clamp_visibility() {
        let c = Crop {
            left: 0.0,
            top: 0.0,
            right: 0.98,
            bottom: 0.0,
        };
        let result = clamp_crop_visibility(&c, 0.05);
        assert!(result.right <= 0.95); // leaves at least 0.05 visible
        assert!((1.0 - result.left - result.right) >= 0.04); // floating tolerance
    }

    #[test]
    fn ins_008_clamp_visibility_fully_within_bounds() {
        let c = Crop {
            left: 0.05,
            top: 0.05,
            right: 0.05,
            bottom: 0.05,
        };
        let result = clamp_crop_visibility(&c, 0.05);
        assert_eq!(result.left, 0.05);
        assert_eq!(result.right, 0.05);
    }

    // INS-009: Resize preserving 16:9 into a 4:3 box.
    #[test]
    fn ins_009_resize_preserve_aspect_wide_into_square() {
        let t = Transform::default();
        let result = resize_preserving_aspect(&t, 100.0, 100.0, 16.0 / 9.0);
        // Height-constrained: fit_h = 100, fit_w = 100 * 16/9 ≈ 177.78
        // But in normalized canvas space: width = 177.78/100 ≈ 1.78, not valid for 0-1...
        // Wait, let me think about the semantics.

        // normalize w.r.t. the new box: fit_w / new_width, fit_h / new_height
        // For 16:9 into 100x100 square:
        //   constrained by height? target_aspect = 100/100 = 1.0, source_aspect = 1.78
        //   target_aspect (1.0) < source_aspect (1.78) → constrained by width
        //   w = 100, h = 100 / 1.78 ≈ 56.25
        //   normalized: width = 100/100 = 1.0, height = 56.25/100 = 0.5625

        assert!((result.width - 1.0).abs() < 1e-6);
        assert!((result.height - 0.5625).abs() < 1e-6);
    }

    #[test]
    fn ins_009_resize_preserve_aspect_square_into_wide() {
        let t = Transform::default();
        let result = resize_preserving_aspect(&t, 200.0, 100.0, 1.0);
        // source 1:1, target 2:1 → constrained by height
        // h = 100, w = 100
        // normalized: width = 100/200 = 0.5, height = 100/100 = 1.0
        assert!((result.width - 0.5).abs() < 1e-6);
        assert!((result.height - 1.0).abs() < 1e-6);
    }

    // INS-010: Font scale scales with sqrt of area ratio.
    #[test]
    fn ins_010_resize_text_font_doubled_area() {
        let style = core_model::TextStyle {
            font_scale: 1.0,
            ..core_model::TextStyle::default()
        };
        let result = resize_text_font(&style, 100.0, 100.0, 200.0, 200.0);
        // area ratio = 4, sqrt = 2
        assert!((result - 2.0).abs() < 1e-9);
    }

    #[test]
    fn ins_010_resize_text_font_half_area() {
        let style = core_model::TextStyle {
            font_scale: 1.0,
            ..core_model::TextStyle::default()
        };
        let result = resize_text_font(&style, 200.0, 200.0, 100.0, 100.0);
        // area ratio = 0.25, sqrt = 0.5
        assert!((result - 0.5).abs() < 1e-9);
    }

    // INS-011: Fit text clip to content updates transform and anchor.
    #[test]
    fn ins_011_fit_text_center_aligned() {
        let mut clip = make_clip();
        clip.text_content = Some("Hello".into());
        clip.text_style = Some(core_model::TextStyle {
            alignment: core_model::TextAlignment::Center,
            ..core_model::TextStyle::default()
        });
        clip.transform = Transform::default();

        fit_text_clip_to_content(&mut clip, 400.0, 100.0, 1920.0, 1080.0);
        assert!((clip.transform.width - 400.0 / 1920.0).abs() < 1e-9);
        assert!((clip.transform.height - 100.0 / 1080.0).abs() < 1e-9);
        // Center: anchor_offset = (1 - new_w) / 2
        let new_w = 400.0 / 1920.0;
        let expected_cx = (1.0 - new_w) / 2.0 + new_w / 2.0;
        assert!((clip.transform.center_x - expected_cx).abs() < 1e-9);
    }

    #[test]
    fn ins_011_fit_text_left_aligned() {
        let mut clip = make_clip();
        clip.text_content = Some("Hi".into());
        clip.text_style = Some(core_model::TextStyle {
            alignment: core_model::TextAlignment::Left,
            ..core_model::TextStyle::default()
        });
        clip.transform = Transform::default();

        fit_text_clip_to_content(&mut clip, 300.0, 80.0, 1920.0, 1080.0);
        let new_w = 300.0 / 1920.0;
        // Left: anchor_offset = 0, center_x = new_w / 2
        assert!((clip.transform.center_x - new_w / 2.0).abs() < 1e-9);
    }

    #[test]
    fn ins_011_fit_text_right_aligned() {
        let mut clip = make_clip();
        clip.text_content = Some("Right".into());
        clip.text_style = Some(core_model::TextStyle {
            alignment: core_model::TextAlignment::Right,
            ..core_model::TextStyle::default()
        });
        clip.transform = Transform::default();

        fit_text_clip_to_content(&mut clip, 500.0, 90.0, 1920.0, 1080.0);
        let new_w = 500.0 / 1920.0;
        // Right: anchor_offset = 1 - new_w, center_x = (1 - new_w) + new_w / 2 = 1 - new_w/2
        let expected_cx = 1.0 - new_w / 2.0;
        assert!((clip.transform.center_x - expected_cx).abs() < 1e-9);
    }

    #[test]
    fn ins_011_fit_text_zero_canvas_noop() {
        let mut clip = make_clip();
        clip.text_content = Some("Noop".into());
        let original = clip.clone();
        fit_text_clip_to_content(&mut clip, 100.0, 50.0, 0.0, 1080.0);
        assert_eq!(clip.transform.width, original.transform.width);
    }

    #[test]
    fn fade_multiplier_ramps_in_and_out() {
        let mut clip = make_clip();
        clip.duration_frames = 100;
        clip.fade_in_frames = 10;
        clip.fade_out_frames = 20;
        // Ramps up through the fade-in region.
        assert!(fade_multiplier_at(&clip, 0) < fade_multiplier_at(&clip, 5));
        assert!(fade_multiplier_at(&clip, 5) < fade_multiplier_at(&clip, 9));
        // Full opacity in the middle.
        assert!((fade_multiplier_at(&clip, 50) - 1.0).abs() < 1e-9);
        // Ramps down through the fade-out region.
        assert!(fade_multiplier_at(&clip, 85) > fade_multiplier_at(&clip, 95));
        assert!(fade_multiplier_at(&clip, 99) < 0.1);
    }

    #[test]
    fn fade_multiplier_is_unity_without_fades() {
        let mut clip = make_clip();
        clip.duration_frames = 50;
        assert_eq!(fade_multiplier_at(&clip, 0), 1.0);
        assert_eq!(fade_multiplier_at(&clip, 49), 1.0);
    }

    #[test]
    fn fade_multiplier_starts_and_ends_at_zero() {
        // No half-frame offset (Swift parity): the very first/last fade frame is
        // exactly 0, not ~0.05.
        let mut clip = make_clip();
        clip.duration_frames = 100;
        clip.fade_in_frames = 10;
        clip.fade_out_frames = 20;
        assert_eq!(fade_multiplier_at(&clip, 0), 0.0);
        assert_eq!(fade_multiplier_at(&clip, 100), 0.0);
        // Fully out of range clamps to 0.
        assert_eq!(fade_multiplier_at(&clip, 101), 0.0);
        assert_eq!(fade_multiplier_at(&clip, -1), 0.0);
    }

    #[test]
    fn fade_multiplier_smooth_curves_the_ramp() {
        // `.smooth` applies smoothstep: below linear in the first half of the
        // ramp, above it in the second half. Linear leaves it as `t`.
        let mut clip = make_clip();
        clip.duration_frames = 100;
        clip.fade_in_frames = 10;
        clip.fade_in_interpolation = Interpolation::Smooth;
        // t=0.2 → smoothstep 0.104 < linear 0.2.
        assert!(fade_multiplier_at(&clip, 2) < 0.2 - 1e-9);
        // t=0.8 → smoothstep 0.896 > linear 0.8.
        assert!(fade_multiplier_at(&clip, 8) > 0.8 + 1e-9);
        // Midpoint smoothstep(0.5) == 0.5, same as linear.
        assert!((fade_multiplier_at(&clip, 5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fade_multiplier_overlapping_fades_take_min_not_product() {
        // Short clip whose head and tail fades overlap: Swift takes the deeper of
        // the two ramps (min), not the product (which would double-attenuate).
        let mut clip = make_clip();
        clip.duration_frames = 10;
        clip.fade_in_frames = 8;
        clip.fade_out_frames = 8;
        // rel=5: in_mul = 5/8 = 0.625, out_mul = (10-5)/8 = 0.625.
        let m = fade_multiplier_at(&clip, 5);
        assert!((m - 0.625).abs() < 1e-9, "min ramp: {m}");
        assert!(m > 0.625 * 0.625 + 1e-9, "not the product");
    }
}
