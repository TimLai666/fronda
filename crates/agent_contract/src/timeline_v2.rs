//! v2 relationship-first timeline view (tool-surface-v2 design C-5).
//!
//! Pure serializers shared by `get_timeline` v2 and the mutation envelope
//! (C-4): the clip shape (`frames: [start, end)`, default-stripping,
//! keyframe collapse, grade/effects vocabulary), track shape (labels, gaps,
//! A/V link-fold), and caption-group summaries.

use core_model::{BlendMode, Clip, ClipType, Crop, TextStyle, Timeline, Transform};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// C-4: mutation envelope lists at most this many changed clips.
pub const CHANGED_CLIPS_CAP: usize = 30;
/// C-4: pure shifts compress into a rule only at this group size.
pub const SHIFT_GROUP_MIN: usize = 3;
/// C-4: changed caption clips fold into a group summary at this count.
pub const CAPTION_FOLD_MIN: usize = 3;
/// C-5: captionDetail rows are capped per group.
pub const CAPTION_ROW_CAP: usize = 200;
/// C-5: textPreview length cap in characters.
pub const TEXT_PREVIEW_MAX: usize = 60;
/// C-5: keyframe constant-collapse tolerance.
pub const KF_TOLERANCE: f64 = 0.0005;

/// C-4/C-5: every float in agent output rounds to 3 decimals.
pub fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn num(v: f64) -> Value {
    json!(round3(v))
}

/// Hex color string from a TextRgba: `#RRGGBB`, `#RRGGBBAA` when alpha < 1.
pub fn hex_color(c: &core_model::TextRgba) -> String {
    let ch = |v: f64| ((v.clamp(0.0, 1.0) * 255.0).round() as u8);
    if c.a < 1.0 {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            ch(c.r),
            ch(c.g),
            ch(c.b),
            ch(c.a)
        )
    } else {
        format!("#{:02X}{:02X}{:02X}", ch(c.r), ch(c.g), ch(c.b))
    }
}

/// BlendMode wire name (serde camelCase raw value, e.g. `softLight`).
pub fn blend_mode_name(m: BlendMode) -> String {
    serde_json::to_value(m)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "normal".into())
}

/// A/V link-fold map (C-5): for every link group with exactly two members —
/// one on an audio track, one on a visual track — maps the VISUAL clip id to
/// the audio partner's `(track_index, clip_id)`. The folded audio clip is not
/// listed on its own track.
pub fn folded_audio_partners(timeline: &Timeline) -> HashMap<String, (usize, String)> {
    let mut groups: HashMap<&str, Vec<(usize, &Clip)>> = HashMap::new();
    for (ti, track) in timeline.tracks.iter().enumerate() {
        for clip in &track.clips {
            if let Some(lg) = clip.link_group_id.as_deref() {
                groups.entry(lg).or_default().push((ti, clip));
            }
        }
    }
    let mut fold = HashMap::new();
    for members in groups.values() {
        if members.len() != 2 {
            continue;
        }
        let audio = members
            .iter()
            .find(|(ti, _)| timeline.tracks[*ti].r#type == ClipType::Audio);
        let visual = members
            .iter()
            .find(|(ti, _)| timeline.tracks[*ti].r#type != ClipType::Audio);
        if let (Some((ati, a)), Some((_, v))) = (audio, visual) {
            fold.insert(v.id.clone(), (*ati, a.id.clone()));
        }
    }
    fold
}

/// Rows for one keyframe track: `(frame, values, interp)` in tool vocabulary.
fn kf_rows_scalar(track: &core_model::KeyframeTrack<f64>) -> Vec<(i64, Vec<f64>, &'static str)> {
    track
        .keyframes
        .iter()
        .map(|k| (k.frame, vec![k.value], interp_name(k.interpolation_out)))
        .collect()
}

fn interp_name(i: core_model::Interpolation) -> &'static str {
    match i {
        core_model::Interpolation::Linear => "linear",
        core_model::Interpolation::Hold => "hold",
        core_model::Interpolation::Smooth => "smooth",
    }
}

fn kf_row_json(frame: i64, values: &[f64], interp: &str) -> Value {
    let mut row: Vec<Value> = vec![json!(frame)];
    row.extend(values.iter().map(|v| num(*v)));
    if interp != "smooth" {
        row.push(json!(interp));
    }
    Value::Array(row)
}

/// Whether every row holds the same values within [`KF_TOLERANCE`].
fn is_constant(rows: &[(i64, Vec<f64>, &'static str)]) -> bool {
    match rows.first() {
        None => true,
        Some((_, first, _)) => rows.iter().all(|(_, v, _)| {
            v.iter()
                .zip(first.iter())
                .all(|(a, b)| (a - b).abs() <= KF_TOLERANCE)
        }),
    }
}

fn values_close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= KF_TOLERANCE)
}

/// The collapsed keyframe view of one clip (C-5): animating tracks as
/// `keyframes.<prop>` rows, non-identity constant tracks folded back into the
/// static fields, identity constant tracks dropped.
struct CollapsedKeyframes {
    keyframes: Map<String, Value>,
    static_opacity: Option<f64>,
    static_volume: Option<f64>,
    static_rotation: Option<f64>,
    static_position: Option<(f64, f64)>,
    static_scale: Option<(f64, f64)>,
    static_crop: Option<Crop>,
}

fn collapse_keyframes(clip: &Clip) -> CollapsedKeyframes {
    let mut out = CollapsedKeyframes {
        keyframes: Map::new(),
        static_opacity: None,
        static_volume: None,
        static_rotation: None,
        static_position: None,
        static_scale: None,
        static_crop: None,
    };

    let mut handle =
        |prop: &str, rows: Vec<(i64, Vec<f64>, &'static str)>, identity: &[f64]| -> Option<Vec<f64>> {
            if rows.is_empty() {
                return None;
            }
            if is_constant(&rows) {
                let v = rows[0].1.clone();
                if values_close(&v, identity) {
                    return None; // identity constant track: dropped
                }
                return Some(v); // non-identity constant: collapse to static
            }
            let json_rows: Vec<Value> = rows
                .iter()
                .map(|(f, v, i)| kf_row_json(*f, v, i))
                .collect();
            out.keyframes.insert(prop.to_string(), json!(json_rows));
            None
        };

    if let Some(t) = &clip.opacity_track {
        if let Some(v) = handle("opacity", kf_rows_scalar(t), &[1.0]) {
            out.static_opacity = Some(v[0]);
        }
    }
    if let Some(t) = &clip.volume_track {
        if let Some(v) = handle("volume", kf_rows_scalar(t), &[1.0]) {
            out.static_volume = Some(v[0]);
        }
    }
    if let Some(t) = &clip.rotation_track {
        if let Some(v) = handle("rotation", kf_rows_scalar(t), &[0.0]) {
            out.static_rotation = Some(v[0]);
        }
    }
    if let Some(t) = &clip.position_track {
        let rows: Vec<_> = t
            .keyframes
            .iter()
            .map(|k| {
                (
                    k.frame,
                    vec![k.value.a, k.value.b],
                    interp_name(k.interpolation_out),
                )
            })
            .collect();
        if let Some(v) = handle("position", rows, &[0.0, 0.0]) {
            out.static_position = Some((v[0], v[1]));
        }
    }
    if let Some(t) = &clip.scale_track {
        let rows: Vec<_> = t
            .keyframes
            .iter()
            .map(|k| {
                (
                    k.frame,
                    vec![k.value.a, k.value.b],
                    interp_name(k.interpolation_out),
                )
            })
            .collect();
        if let Some(v) = handle("scale", rows, &[1.0, 1.0]) {
            out.static_scale = Some((v[0], v[1]));
        }
    }
    if let Some(t) = &clip.crop_track {
        let rows: Vec<_> = t
            .keyframes
            .iter()
            .map(|k| {
                (
                    k.frame,
                    vec![k.value.top, k.value.right, k.value.bottom, k.value.left],
                    interp_name(k.interpolation_out),
                )
            })
            .collect();
        if let Some(v) = handle("crop", rows, &[0.0, 0.0, 0.0, 0.0]) {
            out.static_crop = Some(Crop {
                top: v[0],
                right: v[1],
                bottom: v[2],
                left: v[3],
            });
        }
    }
    out
}

/// The clip's grade in apply_color vocabulary (C-5 `color`), rebuilt from its
/// `color.*` effects. `None` when ungraded.
pub fn color_object(clip: &Clip) -> Option<Value> {
    let effects = clip.effects.as_ref()?;
    let mut grade = Map::new();
    for e in effects.iter().filter(|e| e.r#type.starts_with("color.")) {
        match e.r#type.as_str() {
            // Legacy single-effect grade (retired set_color_grade): flatten.
            "color.grade" => {
                for (k, p) in &e.params {
                    if let Some(v) = p.value {
                        grade.insert(k.clone(), num(v));
                    }
                }
            }
            "color.curves" => {
                for (k, p) in &e.params {
                    if let Some(s) = &p.string {
                        if let Ok(v) = serde_json::from_str::<Value>(s) {
                            grade.insert(format!("{k}Curve"), v);
                        }
                    }
                }
            }
            "color.hueCurves" => {
                if let Some(s) = e.params.get("targets").and_then(|p| p.string.as_ref()) {
                    if let Ok(v) = serde_json::from_str::<Value>(s) {
                        grade.insert("hueCurves".into(), json!({ "targets": v }));
                    }
                }
            }
            "color.lut" => {
                let mut lut = Map::new();
                if let Some(p) = e.params.get("path").and_then(|p| p.string.as_ref()) {
                    lut.insert("path".into(), json!(p));
                }
                if let Some(s) = e.params.get("strength").and_then(|p| p.value) {
                    lut.insert("strength".into(), num(s));
                }
                if !lut.is_empty() {
                    grade.insert("lut".into(), Value::Object(lut));
                }
            }
            other => {
                let knob = other.trim_start_matches("color.");
                if let Some(v) = e.params.values().find_map(|p| p.value) {
                    grade.insert(knob.to_string(), num(v));
                }
            }
        }
    }
    if grade.is_empty() {
        None
    } else {
        Some(Value::Object(grade))
    }
}

/// Non-color effects in apply_effect vocabulary (C-5): `[{type, params
/// (flattened), enabled (only when false)}]`, including a synthesized
/// `key.chroma` entry when the clip carries a chroma key. `None` when empty.
pub fn effects_list(clip: &Clip) -> Option<Value> {
    let mut list: Vec<Value> = Vec::new();
    // apply_effect's key.chroma keeps a real effect entry (richer params) AND
    // mirrors into chroma_key for the compositor — synthesize from the mirror
    // only when no entry exists (legacy projects).
    let has_chroma_entry = clip
        .effects
        .as_ref()
        .is_some_and(|es| es.iter().any(|e| e.r#type == "key.chroma"));
    if let Some(ck) = clip.chroma_key.as_ref().filter(|_| !has_chroma_entry) {
        let (h, _s, _v) = rgb_to_hue(ck.key_r, ck.key_g, ck.key_b);
        let mut e = Map::new();
        e.insert("type".into(), json!("key.chroma"));
        e.insert(
            "params".into(),
            json!({
                "keyHue": round3(h),
                "tolerance": round3(ck.tolerance),
                "spill": round3(ck.spill_suppression),
            }),
        );
        if !ck.enabled {
            e.insert("enabled".into(), json!(false));
        }
        list.push(Value::Object(e));
    }
    if let Some(effects) = &clip.effects {
        for e in effects.iter().filter(|e| !e.r#type.starts_with("color.")) {
            let mut params = Map::new();
            let mut keys: Vec<&String> = e.params.keys().collect();
            keys.sort();
            for k in keys {
                let p = &e.params[k];
                if let Some(v) = p.value {
                    params.insert(k.clone(), num(v));
                } else if let Some(s) = &p.string {
                    params.insert(k.clone(), json!(s));
                }
            }
            let mut obj = Map::new();
            obj.insert("type".into(), json!(e.r#type));
            if !params.is_empty() {
                obj.insert("params".into(), Value::Object(params));
            }
            if !e.enabled {
                obj.insert("enabled".into(), json!(false));
            }
            list.push(Value::Object(obj));
        }
    }
    if list.is_empty() {
        None
    } else {
        Some(json!(list))
    }
}

/// Normalized hue (0–1) of an RGB color, matching the chroma-key convention.
fn rgb_to_hue(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d.abs() < f64::EPSILON {
        0.0
    } else if (max - r).abs() < f64::EPSILON {
        (((g - b) / d).rem_euclid(6.0)) / 6.0
    } else if (max - g).abs() < f64::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    let s = if max.abs() < f64::EPSILON { 0.0 } else { d / max };
    (h, s, max)
}

/// Non-default text style fields in the add_texts/update_text vocabulary.
fn text_style_v2(style: &TextStyle) -> Map<String, Value> {
    let d = TextStyle::default();
    let mut out = Map::new();
    if style.font_name != d.font_name {
        out.insert("fontName".into(), json!(style.font_name));
    }
    if (style.font_size - d.font_size).abs() > f64::EPSILON {
        out.insert("fontSize".into(), num(style.font_size));
    }
    if style.font_weight >= 700.0 {
        out.insert("isBold".into(), json!(true));
    }
    if style.is_italic {
        out.insert("isItalic".into(), json!(true));
    }
    if style.color != d.color {
        out.insert("color".into(), json!(hex_color(&style.color)));
    }
    if style.alignment != d.alignment {
        let name = match style.alignment {
            core_model::TextAlignment::Left => "left",
            core_model::TextAlignment::Center => "center",
            core_model::TextAlignment::Right => "right",
        };
        out.insert("alignment".into(), json!(name));
    }
    if style.border.enabled {
        out.insert("borderColor".into(), json!(hex_color(&style.border.color)));
    }
    if style.background.enabled {
        out.insert(
            "backgroundColor".into(),
            json!(hex_color(&style.background.color)),
        );
    }
    out
}

/// The v2 clip JSON shape (C-5), without A/V folding (see [`clip_v2_folded`]).
pub fn clip_v2(clip: &Clip) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("id".into(), json!(clip.id));
    if !clip.media_ref.is_empty() {
        out.insert("mediaRef".into(), json!(clip.media_ref));
    }
    out.insert(
        "frames".into(),
        json!([clip.start_frame, clip.start_frame + clip.duration_frames]),
    );
    if clip.media_type != ClipType::Video {
        out.insert("mediaType".into(), json!(clip.media_type.name()));
    }
    if clip.source_clip_type != clip.media_type {
        out.insert(
            "sourceClipType".into(),
            json!(clip.source_clip_type.name()),
        );
    }
    if (clip.speed - 1.0).abs() > f64::EPSILON {
        out.insert("speed".into(), num(clip.speed));
    }

    let collapsed = collapse_keyframes(clip);
    let is_text = clip.media_type == ClipType::Text;

    let volume = collapsed.static_volume.unwrap_or(clip.volume);
    if (volume - 1.0).abs() > f64::EPSILON {
        out.insert("volume".into(), num(volume));
    }
    let opacity = collapsed.static_opacity.unwrap_or(clip.opacity);
    if (opacity - 1.0).abs() > f64::EPSILON {
        out.insert("opacity".into(), num(opacity));
    }
    if !is_text {
        if clip.trim_start_frame != 0 {
            out.insert("trimStartFrame".into(), json!(clip.trim_start_frame));
        }
        if clip.trim_end_frame != 0 {
            out.insert("trimEndFrame".into(), json!(clip.trim_end_frame));
        }
    }
    if clip.fade_in_frames != 0 {
        out.insert("fadeInFrames".into(), json!(clip.fade_in_frames));
    }
    if clip.fade_out_frames != 0 {
        out.insert("fadeOutFrames".into(), json!(clip.fade_out_frames));
    }

    // Transform: static merged with collapsed constant tracks.
    let mut t = clip.transform;
    if let Some((w, h)) = collapsed.static_scale {
        t.width = w;
        t.height = h;
    }
    if let Some((x, y)) = collapsed.static_position {
        // position keyframes store the normalized TOP-LEFT (spec INS-003).
        t.center_x = x + t.width / 2.0;
        t.center_y = y + t.height / 2.0;
    }
    if let Some(r) = collapsed.static_rotation {
        t.rotation = r;
    }
    if t != Transform::default() {
        let mut tj = Map::new();
        tj.insert("centerX".into(), num(t.center_x));
        tj.insert("centerY".into(), num(t.center_y));
        tj.insert("width".into(), num(t.width));
        tj.insert("height".into(), num(t.height));
        if t.rotation.abs() > f64::EPSILON {
            tj.insert("rotation".into(), num(t.rotation));
        }
        if t.flip_horizontal {
            tj.insert("flipHorizontal".into(), json!(true));
        }
        if t.flip_vertical {
            tj.insert("flipVertical".into(), json!(true));
        }
        out.insert("transform".into(), Value::Object(tj));
    }

    // Crop: static merged with a collapsed constant crop track; non-zero
    // insets only.
    let crop = collapsed.static_crop.unwrap_or(clip.crop);
    let mut cj = Map::new();
    for (k, v) in [
        ("top", crop.top),
        ("right", crop.right),
        ("bottom", crop.bottom),
        ("left", crop.left),
    ] {
        if v.abs() > f64::EPSILON {
            cj.insert(k.into(), num(v));
        }
    }
    if !cj.is_empty() {
        out.insert("crop".into(), Value::Object(cj));
    }

    if clip.blend_mode != BlendMode::Normal {
        out.insert("blendMode".into(), json!(blend_mode_name(clip.blend_mode)));
    }

    if is_text {
        if let Some(content) = &clip.text_content {
            out.insert("content".into(), json!(content));
        }
        if let Some(style) = &clip.text_style {
            let sj = text_style_v2(style);
            if !sj.is_empty() {
                out.insert("textStyle".into(), Value::Object(sj));
            }
        }
        if let Some(anim) = &clip.text_animation {
            if anim.preset != core_model::TextAnimationPreset::None {
                if let Ok(Value::String(name)) = serde_json::to_value(anim.preset) {
                    out.insert("animation".into(), json!(name));
                }
                if let Some(h) = &anim.highlight {
                    out.insert("highlightColor".into(), json!(hex_color(h)));
                }
            }
        }
    }

    if let Some(lg) = &clip.link_group_id {
        out.insert("linkGroupId".into(), json!(lg));
    }
    if let Some(cg) = &clip.caption_group_id {
        out.insert("captionGroupId".into(), json!(cg));
    }

    if let Some(color) = color_object(clip) {
        out.insert("color".into(), color);
    }
    if let Some(effects) = effects_list(clip) {
        out.insert("effects".into(), effects);
    }
    if !collapsed.keyframes.is_empty() {
        out.insert("keyframes".into(), Value::Object(collapsed.keyframes));
    }
    out
}

/// The v2 clip shape with its linked audio partner folded in (C-5): the
/// audio object carries `id`, `track`, and only the fields that deviate from
/// the visual clip.
pub fn clip_v2_folded(clip: &Clip, audio_track: usize, audio: &Clip) -> Map<String, Value> {
    let mut out = clip_v2(clip);
    out.remove("linkGroupId");
    let mut a = Map::new();
    a.insert("id".into(), json!(audio.id));
    a.insert("track".into(), json!(audio_track));
    if audio.start_frame != clip.start_frame || audio.duration_frames != clip.duration_frames {
        a.insert(
            "frames".into(),
            json!([audio.start_frame, audio.start_frame + audio.duration_frames]),
        );
    }
    if audio.trim_start_frame != clip.trim_start_frame {
        a.insert("trimStartFrame".into(), json!(audio.trim_start_frame));
    }
    if audio.trim_end_frame != clip.trim_end_frame {
        a.insert("trimEndFrame".into(), json!(audio.trim_end_frame));
    }
    if (audio.speed - clip.speed).abs() > f64::EPSILON {
        a.insert("speed".into(), num(audio.speed));
    }
    let audio_collapsed = collapse_keyframes(audio);
    let audio_volume = audio_collapsed.static_volume.unwrap_or(audio.volume);
    if (audio_volume - clip.volume).abs() > f64::EPSILON {
        a.insert("volume".into(), num(audio_volume));
    }
    if audio.fade_in_frames != clip.fade_in_frames {
        a.insert("fadeInFrames".into(), json!(audio.fade_in_frames));
    }
    if audio.fade_out_frames != clip.fade_out_frames {
        a.insert("fadeOutFrames".into(), json!(audio.fade_out_frames));
    }
    if !audio_collapsed.keyframes.is_empty() {
        a.insert("keyframes".into(), Value::Object(audio_collapsed.keyframes));
    }
    if let Some(effects) = effects_list(audio) {
        a.insert("effects".into(), effects);
    }
    out.insert("audio".into(), Value::Object(a));
    out
}

/// Empty `[start, end)` spans between consecutive non-caption clips on one
/// track (C-5). Head and tail gaps are not reported.
pub fn track_gaps(clips: &[&Clip]) -> Vec<[i64; 2]> {
    let mut sorted: Vec<&&Clip> = clips.iter().collect();
    sorted.sort_by_key(|c| c.start_frame);
    let mut gaps = Vec::new();
    for pair in sorted.windows(2) {
        let prev_end = pair[0].start_frame + pair[0].duration_frames;
        let next_start = pair[1].start_frame;
        if next_start > prev_end {
            gaps.push([prev_end, next_start]);
        }
    }
    gaps
}

/// One caption group's fold on a track (C-5).
pub struct CaptionGroupView {
    pub summary: Value,
    /// Clips whose residual properties deviate from the group mode — these
    /// are also listed individually in `clips`.
    pub deviant_ids: Vec<String>,
}

/// Residual (style) properties of a caption clip: everything except identity,
/// timing, and text content. Transform width/height are excluded (auto-fit).
fn caption_residual(clip: &Clip) -> Value {
    let mut m = clip_v2(clip);
    for k in [
        "id",
        "mediaRef",
        "frames",
        "content",
        "captionGroupId",
        "linkGroupId",
        "keyframes",
    ] {
        m.remove(k);
    }
    if let Some(Value::Object(t)) = m.get_mut("transform") {
        t.remove("width");
        t.remove("height");
        if t.is_empty() {
            m.remove("transform");
        }
    }
    Value::Object(m)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// Folds one track's caption clips (same `captionGroupId`) into summaries.
/// `detail` expands per-clip rows (capped at [`CAPTION_ROW_CAP`]).
pub fn caption_groups_v2(clips: &[&Clip], detail: bool) -> Vec<(CaptionGroupView, String)> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<&Clip>> = HashMap::new();
    for clip in clips {
        if let Some(cg) = &clip.caption_group_id {
            if !groups.contains_key(cg) {
                order.push(cg.clone());
            }
            groups.entry(cg.clone()).or_default().push(clip);
        }
    }
    let mut out = Vec::new();
    for cg in order {
        let mut members = groups.remove(&cg).unwrap_or_default();
        members.sort_by_key(|c| c.start_frame);
        let first = members.first().map(|c| c.start_frame).unwrap_or(0);
        let last = members
            .iter()
            .map(|c| c.start_frame + c.duration_frames)
            .max()
            .unwrap_or(0);

        // Modal residual property set.
        let mut counts: HashMap<String, (usize, Value)> = HashMap::new();
        for c in &members {
            let r = caption_residual(c);
            let key = r.to_string();
            counts.entry(key).or_insert((0, r)).0 += 1;
        }
        let shared = counts
            .values()
            .max_by_key(|(n, _)| *n)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| json!({}));
        let deviant_ids: Vec<String> = members
            .iter()
            .filter(|c| caption_residual(c) != shared)
            .map(|c| c.id.clone())
            .collect();

        let first_text = members
            .first()
            .and_then(|c| c.text_content.as_deref())
            .unwrap_or("");
        let last_text = members
            .last()
            .and_then(|c| c.text_content.as_deref())
            .unwrap_or("");
        let preview = if members.len() == 1 {
            truncate_chars(first_text, TEXT_PREVIEW_MAX)
        } else {
            truncate_chars(&format!("{first_text} … {last_text}"), TEXT_PREVIEW_MAX)
        };

        let mut summary = Map::new();
        summary.insert("captionGroupId".into(), json!(cg));
        summary.insert("clipCount".into(), json!(members.len()));
        summary.insert("frameRange".into(), json!([first, last]));
        summary.insert("shared".into(), shared);
        summary.insert("textPreview".into(), json!(preview));
        if detail {
            summary.insert(
                "clipFormat".into(),
                json!(["clipId", "startFrame", "endFrame", "text"]),
            );
            let rows: Vec<Value> = members
                .iter()
                .take(CAPTION_ROW_CAP)
                .map(|c| {
                    json!([
                        c.id,
                        c.start_frame,
                        c.start_frame + c.duration_frames,
                        c.text_content.as_deref().unwrap_or("")
                    ])
                })
                .collect();
            if members.len() > CAPTION_ROW_CAP {
                summary.insert(
                    "clipsNote".into(),
                    json!(format!(
                        "Showing {} of {} caption clips — window with startFrame/endFrame for the rest.",
                        CAPTION_ROW_CAP,
                        members.len()
                    )),
                );
            }
            summary.insert("clips".into(), json!(rows));
        }
        out.push((
            CaptionGroupView {
                summary: Value::Object(summary),
                deviant_ids,
            },
            cg,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{AnimPair, Interpolation, Keyframe, KeyframeTrack, TextRgba, Track};

    fn clip(id: &str, start: i64, dur: i64) -> Clip {
        let mut c = crate::test_helpers::make_clip(start, dur);
        c.id = id.into();
        c.media_ref = "media-1".into();
        c
    }

    fn track(kind: ClipType, clips: Vec<Clip>) -> Track {
        Track {
            id: uuid::Uuid::new_v4().to_string(),
            r#type: kind,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips,
        }
    }

    #[test]
    fn round3_rounds_to_three_decimals() {
        assert_eq!(round3(0.123456), 0.123);
        assert_eq!(round3(0.9995), 1.0);
    }

    #[test]
    fn clip_v2_strips_defaults_and_uses_frames_pair() {
        let c = clip("c1", 120, 180);
        let j = Value::Object(clip_v2(&c));
        assert_eq!(j["frames"], json!([120, 300]));
        assert_eq!(j["id"], json!("c1"));
        assert_eq!(j["mediaRef"], json!("media-1"));
        for k in [
            "mediaType",
            "sourceClipType",
            "speed",
            "volume",
            "opacity",
            "trimStartFrame",
            "trimEndFrame",
            "fadeInFrames",
            "fadeOutFrames",
            "transform",
            "crop",
            "blendMode",
            "keyframes",
            "effects",
            "color",
            "startFrame",
            "durationFrames",
        ] {
            assert!(j.get(k).is_none(), "default field '{k}' must be stripped");
        }
    }

    #[test]
    fn clip_v2_reports_non_defaults() {
        let mut c = clip("c1", 0, 100);
        c.speed = 2.0;
        c.volume = 0.5;
        c.opacity = 0.75;
        c.trim_start_frame = 10;
        c.blend_mode = BlendMode::SoftLight;
        c.crop.left = 0.25;
        let j = Value::Object(clip_v2(&c));
        assert_eq!(j["speed"], json!(2.0));
        assert_eq!(j["volume"], json!(0.5));
        assert_eq!(j["opacity"], json!(0.75));
        assert_eq!(j["trimStartFrame"], json!(10));
        assert_eq!(j["blendMode"], json!("softLight"));
        assert_eq!(j["crop"], json!({"left": 0.25}));
    }

    #[test]
    fn text_clip_never_reports_trims() {
        let mut c = clip("t1", 0, 100);
        c.media_type = ClipType::Text;
        c.source_clip_type = ClipType::Text;
        c.trim_start_frame = 5;
        c.text_content = Some("Hello".into());
        let j = Value::Object(clip_v2(&c));
        assert!(j.get("trimStartFrame").is_none());
        assert_eq!(j["content"], json!("Hello"));
        assert_eq!(j["mediaType"], json!("text"));
        assert!(j.get("sourceClipType").is_none(), "== mediaType stripped");
    }

    #[test]
    fn identity_constant_keyframe_track_is_dropped() {
        let mut c = clip("c1", 0, 100);
        c.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 1.0,
                    interpolation_out: Interpolation::Smooth,
                },
                Keyframe {
                    frame: 50,
                    value: 1.0002,
                    interpolation_out: Interpolation::Smooth,
                },
            ],
        });
        let j = Value::Object(clip_v2(&c));
        assert!(j.get("keyframes").is_none());
        assert!(j.get("opacity").is_none());
    }

    #[test]
    fn non_identity_constant_track_collapses_to_static_field() {
        let mut c = clip("c1", 0, 100);
        c.crop_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: Crop {
                        left: 0.31,
                        ..Crop::default()
                    },
                    interpolation_out: Interpolation::Smooth,
                },
                Keyframe {
                    frame: 80,
                    value: Crop {
                        left: 0.3102,
                        ..Crop::default()
                    },
                    interpolation_out: Interpolation::Smooth,
                },
            ],
        });
        let j = Value::Object(clip_v2(&c));
        assert!(j.get("keyframes").is_none());
        assert_eq!(j["crop"]["left"], json!(0.31));
    }

    #[test]
    fn animating_track_emits_rows_with_non_smooth_interp_only() {
        let mut c = clip("c1", 0, 100);
        c.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.0,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 30,
                    value: 1.0,
                    interpolation_out: Interpolation::Smooth,
                },
            ],
        });
        let j = Value::Object(clip_v2(&c));
        assert_eq!(
            j["keyframes"]["opacity"],
            json!([[0, 0.0, "linear"], [30, 1.0]])
        );
    }

    #[test]
    fn constant_position_track_collapses_into_transform_center() {
        let mut c = clip("c1", 0, 100);
        // Top-left (0.25, 0.25) with default full-canvas size → centre (0.75, 0.75).
        c.transform.width = 1.0;
        c.transform.height = 1.0;
        c.position_track = Some(KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: AnimPair { a: 0.25, b: 0.25 },
                interpolation_out: Interpolation::Smooth,
            }],
        });
        let j = Value::Object(clip_v2(&c));
        assert!(j.get("keyframes").is_none());
        assert_eq!(j["transform"]["centerX"], json!(0.75));
        assert_eq!(j["transform"]["centerY"], json!(0.75));
    }

    #[test]
    fn av_fold_maps_two_member_link_groups() {
        let mut timeline = Timeline::default();
        timeline.tracks = vec![
            track(ClipType::Video, vec![{
                let mut c = clip("v1", 0, 100);
                c.link_group_id = Some("lg1".into());
                c
            }]),
            track(ClipType::Audio, vec![{
                let mut c = clip("a1", 0, 100);
                c.media_type = ClipType::Audio;
                c.link_group_id = Some("lg1".into());
                c
            }]),
        ];
        let fold = folded_audio_partners(&timeline);
        assert_eq!(fold.get("v1"), Some(&(1usize, "a1".to_string())));
        assert!(!fold.contains_key("a1"));
    }

    #[test]
    fn folded_audio_carries_only_deviations() {
        let mut v = clip("v1", 0, 100);
        v.link_group_id = Some("lg1".into());
        let mut a = clip("a1", 0, 100);
        a.media_type = ClipType::Audio;
        a.link_group_id = Some("lg1".into());
        a.volume = 0.4;
        let j = Value::Object(clip_v2_folded(&v, 1, &a));
        assert!(j.get("linkGroupId").is_none(), "folded visual drops linkGroupId");
        let audio = &j["audio"];
        assert_eq!(audio["id"], json!("a1"));
        assert_eq!(audio["track"], json!(1));
        assert_eq!(audio["volume"], json!(0.4));
        assert!(audio.get("frames").is_none(), "aligned frames omitted");
        assert!(audio.get("speed").is_none());
    }

    #[test]
    fn folded_audio_reports_offset_frames() {
        let mut v = clip("v1", 0, 100);
        v.link_group_id = Some("lg1".into());
        let mut a = clip("a1", 12, 100);
        a.media_type = ClipType::Audio;
        a.link_group_id = Some("lg1".into());
        let j = Value::Object(clip_v2_folded(&v, 1, &a));
        assert_eq!(j["audio"]["frames"], json!([12, 112]));
    }

    #[test]
    fn gaps_report_interior_spans_only() {
        let a = clip("a", 0, 100);
        let b = clip("b", 150, 50);
        let c = clip("c", 200, 40);
        let clips: Vec<&Clip> = vec![&a, &b, &c];
        assert_eq!(track_gaps(&clips), vec![[100, 150]]);
    }

    #[test]
    fn caption_groups_fold_with_preview_and_deviants() {
        let mut clips: Vec<Clip> = Vec::new();
        for (i, text) in ["First words", "middle", "the last words"].iter().enumerate() {
            let mut c = clip(&format!("cap{i}"), i as i64 * 60, 60);
            c.media_type = ClipType::Text;
            c.source_clip_type = ClipType::Text;
            c.caption_group_id = Some("cg1".into());
            c.text_content = Some(text.to_string());
            clips.push(c);
        }
        // One deviant: different opacity.
        clips[1].opacity = 0.5;
        let refs: Vec<&Clip> = clips.iter().collect();
        let groups = caption_groups_v2(&refs, false);
        assert_eq!(groups.len(), 1);
        let (view, cg) = &groups[0];
        assert_eq!(cg, "cg1");
        assert_eq!(view.summary["clipCount"], json!(3));
        assert_eq!(view.summary["frameRange"], json!([0, 180]));
        assert_eq!(
            view.summary["textPreview"],
            json!("First words … the last words")
        );
        assert_eq!(view.deviant_ids, vec!["cap1".to_string()]);
        assert!(view.summary.get("clips").is_none(), "no detail rows");
    }

    #[test]
    fn caption_detail_emits_capped_rows() {
        let mut clips: Vec<Clip> = Vec::new();
        for i in 0..(CAPTION_ROW_CAP + 5) {
            let mut c = clip(&format!("cap{i}"), i as i64 * 10, 10);
            c.media_type = ClipType::Text;
            c.caption_group_id = Some("cg1".into());
            c.text_content = Some(format!("w{i}"));
            clips.push(c);
        }
        let refs: Vec<&Clip> = clips.iter().collect();
        let groups = caption_groups_v2(&refs, true);
        let (view, _) = &groups[0];
        assert_eq!(
            view.summary["clipFormat"],
            json!(["clipId", "startFrame", "endFrame", "text"])
        );
        assert_eq!(
            view.summary["clips"].as_array().unwrap().len(),
            CAPTION_ROW_CAP
        );
        assert!(view.summary["clipsNote"]
            .as_str()
            .unwrap()
            .contains("Showing 200 of 205"));
        assert_eq!(view.summary["clips"][0], json!(["cap0", 0, 10, "w0"]));
    }

    #[test]
    fn text_preview_truncates_at_60_chars() {
        let long = "x".repeat(80);
        let mut c = clip("cap0", 0, 10);
        c.media_type = ClipType::Text;
        c.caption_group_id = Some("cg1".into());
        c.text_content = Some(long);
        let clips = vec![&c];
        let groups = caption_groups_v2(&clips, false);
        let preview = groups[0].0.summary["textPreview"].as_str().unwrap();
        assert!(preview.chars().count() <= TEXT_PREVIEW_MAX);
    }

    #[test]
    fn color_object_rebuilds_grade_from_effects() {
        let mut c = clip("c1", 0, 100);
        c.effects = Some(vec![
            core_model::Effect {
                id: "e1".into(),
                r#type: "color.exposure".into(),
                enabled: true,
                params: [("ev".to_string(), core_model::EffectParam::value(0.5))]
                    .into_iter()
                    .collect(),
            },
            core_model::Effect {
                id: "e2".into(),
                r#type: "blur.gaussian".into(),
                enabled: true,
                params: [("radius".to_string(), core_model::EffectParam::value(8.0))]
                    .into_iter()
                    .collect(),
            },
        ]);
        let j = Value::Object(clip_v2(&c));
        assert_eq!(j["color"], json!({"exposure": 0.5}));
        assert_eq!(
            j["effects"],
            json!([{"type": "blur.gaussian", "params": {"radius": 8.0}}])
        );
    }

    #[test]
    fn chroma_key_synthesizes_key_chroma_effect() {
        let mut c = clip("c1", 0, 100);
        c.chroma_key = Some(core_model::ChromaKey {
            enabled: true,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.4,
            spill_suppression: 0.5,
        });
        let j = Value::Object(clip_v2(&c));
        let e = &j["effects"][0];
        assert_eq!(e["type"], json!("key.chroma"));
        assert_eq!(e["params"]["keyHue"], json!(0.333));
        assert_eq!(e["params"]["tolerance"], json!(0.4));
    }

    #[test]
    fn chroma_effect_entry_suppresses_the_mirror_synth() {
        // apply_effect key.chroma stores an entry AND mirrors chroma_key —
        // the effects list must not show the key twice.
        let mut c = clip("c1", 0, 100);
        c.chroma_key = Some(core_model::ChromaKey {
            enabled: true,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.4,
            spill_suppression: 0.5,
        });
        c.effects = Some(vec![core_model::Effect {
            id: "e1".into(),
            r#type: "key.chroma".into(),
            enabled: true,
            params: [
                ("keyHue".to_string(), core_model::EffectParam::value(0.333)),
                ("softness".to_string(), core_model::EffectParam::value(0.7)),
            ]
            .into_iter()
            .collect(),
        }]);
        let j = Value::Object(clip_v2(&c));
        let effects = j["effects"].as_array().unwrap();
        assert_eq!(effects.len(), 1, "no duplicate key.chroma: {j}");
        assert_eq!(effects[0]["params"]["softness"], json!(0.7), "entry wins (richer)");
    }

    #[test]
    fn text_style_reports_only_non_defaults() {
        let mut style = TextStyle::default();
        style.font_weight = 700.0;
        style.color = TextRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let j = Value::Object(text_style_v2(&style));
        assert_eq!(j["isBold"], json!(true));
        assert_eq!(j["color"], json!("#FF0000"));
        assert!(j.get("fontName").is_none());
        assert!(j.get("fontSize").is_none());
        assert!(j.get("alignment").is_none());
    }
}
