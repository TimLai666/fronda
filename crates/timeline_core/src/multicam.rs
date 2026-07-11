//! Multicam engine (upstream #283): pure port of Swift `MulticamEngine.swift`
//! plus the timeline-math parts of `EditorViewModel+Multicam.swift`.
//!
//! A group's members live on one shared clock (per-member `sync.offsetSeconds`
//! from the group zero). Clips are ordinary stamped timeline clips
//! (`Clip.multicam_group_id`); switching an angle rewrites `media_ref` +
//! `trim_start_frame` so the same real moment shows on the new camera.

use crate::edit::split_values;
use crate::{ClipMathExt, TimelineMathExt};
use core_model::video_layout::{layout_placement, LayoutFit, LayoutRect};
use core_model::{
    Clip, ClipType, Crop, Interpolation, MulticamMember, MulticamMemberKind, MulticamSource,
    MulticamSyncMap, Timeline, Track, Transform, VideoLayout,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

/// Per-member asset facts the engine needs; the host resolves them from the
/// media manifest (Swift reads `mediaAssets` directly).
#[derive(Debug, Clone)]
pub struct MulticamAsset {
    pub name: String,
    pub clip_type: ClipType,
    pub duration: f64,
    pub has_audio: bool,
    pub source_width: Option<i64>,
    pub source_height: Option<i64>,
}

/// Member request for group creation (Swift `MulticamMemberSpec`).
#[derive(Debug, Clone)]
pub struct MulticamMemberSpec {
    pub media_ref: String,
    pub kind: MulticamMemberKind,
    pub angle_label: Option<String>,
    pub pinned_offset_seconds: Option<f64>,
}

/// One angle-switch request (Swift `AngleSwitchRequest`): full-frame
/// (`layout == Full`, one angle) or a multi-angle layout.
#[derive(Debug, Clone)]
pub struct AngleSwitchRequest {
    pub range: Range<i64>,
    pub layout: VideoLayout,
    pub angles: Vec<String>,
}

impl AngleSwitchRequest {
    pub fn full(range: Range<i64>, angle: &str) -> Self {
        Self {
            range,
            layout: VideoLayout::Full,
            angles: vec![angle.to_string()],
        }
    }

    pub fn with_layout(range: Range<i64>, layout: VideoLayout, angles: Vec<String>) -> Self {
        Self {
            range,
            layout,
            angles,
        }
    }
}

/// Engine entry (Swift `MulticamEngine.Entry`): resolved members fill the
/// layout's slots in order; `slots[0]` is the program angle.
#[derive(Debug, Clone)]
pub struct MulticamEntry {
    pub range: Range<i64>,
    pub slots: Vec<MulticamMember>,
    pub layout: VideoLayout,
}

/// A range that had to shrink because the target angle wasn't recording.
#[derive(Debug, Clone, PartialEq)]
pub struct MulticamClamp {
    pub requested: Range<i64>,
    pub applied: Range<i64>,
    pub culprit: String,
}

/// Switch outcome (Swift `MulticamEngine.Outcome`).
#[derive(Debug, Clone, Default)]
pub struct MulticamOutcome {
    pub switched: usize,
    pub merged: usize,
    pub applied: Vec<Range<i64>>,
    pub clamped: Vec<MulticamClamp>,
    pub skipped: Vec<(Range<i64>, String)>,
    pub overlay_clip_ids: Vec<String>,
}

/// Swift `Clip.sourceFramesConsumed`: source frames a clip plays through.
pub fn source_frames_consumed(clip: &Clip) -> i64 {
    (clip.duration_frames as f64 * clip.speed).round() as i64
}

/// Aspect-fit transform (Swift `EditorViewModel.fitTransform`): letterbox the
/// source into the canvas; near-matching aspects (±0.02) stay full-frame.
pub fn fit_transform(
    source_width: i64,
    source_height: i64,
    canvas_width: i64,
    canvas_height: i64,
) -> Transform {
    const ASPECT_TOLERANCE: f64 = 0.02;
    if source_width <= 0 || source_height <= 0 || canvas_width <= 0 || canvas_height <= 0 {
        return Transform::default();
    }
    let canvas_aspect = canvas_width as f64 / canvas_height as f64;
    let relative_aspect = (source_width as f64 / source_height as f64) / canvas_aspect;
    let source_aspect = relative_aspect * canvas_aspect;
    if (canvas_aspect - source_aspect).abs() < ASPECT_TOLERANCE {
        return Transform::default();
    }
    if relative_aspect > 1.0 {
        Transform {
            width: 1.0,
            height: 1.0 / relative_aspect,
            ..Transform::default()
        }
    } else {
        Transform {
            width: relative_aspect,
            height: 1.0,
            ..Transform::default()
        }
    }
}

/// Audio-sync lag clamp (Swift `MulticamEngine.maxLagHops`): a lag may not
/// exceed half the shorter envelope, so thin overlaps can't fake a peak.
pub fn max_lag_hops(
    window_seconds: f64,
    hop_seconds: f64,
    reference_count: usize,
    target_count: usize,
) -> usize {
    let window_hops = (window_seconds / hop_seconds).round() as usize;
    window_hops
        .min(reference_count.min(target_count) / 2)
        .max(1)
}

// ── Lookup ───────────────────────────────────────────────────────────────

/// All clips stamped with `group_id`, as `(track_index, clip_index)`.
pub fn multicam_clip_locations(timeline: &Timeline, group_id: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (ti, track) in timeline.tracks.iter().enumerate() {
        for (ci, clip) in track.clips.iter().enumerate() {
            if clip.multicam_group_id.as_deref() == Some(group_id) {
                out.push((ti, ci));
            }
        }
    }
    out
}

pub fn multicam_track_indexes(timeline: &Timeline, group_id: &str) -> BTreeSet<usize> {
    multicam_clip_locations(timeline, group_id)
        .into_iter()
        .map(|(ti, _)| ti)
        .collect()
}

/// Group ids referenced by any clip across `timelines`.
pub fn referenced_group_ids<'a>(
    timelines: impl IntoIterator<Item = &'a Timeline>,
) -> HashSet<String> {
    timelines
        .into_iter()
        .flat_map(|t| t.tracks.iter())
        .flat_map(|tr| tr.clips.iter())
        .filter_map(|c| c.multicam_group_id.clone())
        .collect()
}

/// Groups worth persisting (Swift `savedMulticamGroups`): only those a
/// timeline still references.
pub fn live_groups<'a>(
    groups: &[MulticamSource],
    timelines: impl IntoIterator<Item = &'a Timeline>,
) -> Vec<MulticamSource> {
    let referenced = referenced_group_ids(timelines);
    groups
        .iter()
        .filter(|g| referenced.contains(&g.id))
        .cloned()
        .collect()
}

fn clip_overlaps(clip: &Clip, range: &Range<i64>) -> bool {
    clip.start_frame < range.end && clip.end_frame() > range.start
}

fn has_keyframes(clip: &Clip) -> bool {
    clip.opacity_track.is_some()
        || clip.position_track.is_some()
        || clip.scale_track.is_some()
        || clip.rotation_track.is_some()
        || clip.crop_track.is_some()
        || clip.volume_track.is_some()
}

/// Swift `Range.clamped(to:)`.
fn clamp_range(r: &Range<i64>, bounds: &Range<i64>) -> Range<i64> {
    let lo = r.start.clamp(bounds.start, bounds.end);
    let hi = r.end.clamp(bounds.start, bounds.end);
    lo..hi.max(lo)
}

fn is_program_fragment(clip: &Clip, group: &MulticamSource) -> bool {
    clip.multicam_group_id.as_deref() == Some(group.id.as_str())
        && clip.media_type != ClipType::Audio
}

/// The BOTTOM-most video track holding a program fragment overlapping `range`
/// (tracks[0] is the top visual layer; Swift `tracks.last`).
fn program_track_id(
    timeline: &Timeline,
    group: &MulticamSource,
    range: &Range<i64>,
) -> Option<String> {
    timeline
        .tracks
        .iter()
        .rev()
        .find(|t| {
            t.r#type == ClipType::Video
                && t.clips
                    .iter()
                    .any(|c| is_program_fragment(c, group) && clip_overlaps(c, range))
        })
        .map(|t| t.id.clone())
}

fn track_index_by_id(timeline: &Timeline, id: &str) -> Option<usize> {
    timeline.tracks.iter().position(|t| t.id == id)
}

/// Swift `Clip(mediaRef:startFrame:durationFrames:)` defaults.
fn base_clip(media_ref: &str, start_frame: i64, duration_frames: i64) -> Clip {
    Clip {
        id: uuid::Uuid::new_v4().to_string(),
        media_ref: media_ref.to_string(),
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
        blend_mode: Default::default(),
        chroma_key: None,
        multicam_group_id: None,
        text_animation: None,
        word_timings: None,
    }
}

fn new_video_track() -> Track {
    Track {
        id: uuid::Uuid::new_v4().to_string(),
        r#type: ClipType::Video,
        muted: false,
        hidden: false,
        sync_locked: true,
        display_height: 50.0,
        clips: Vec::new(),
    }
}

// ── Clip surgery ─────────────────────────────────────────────────────────

/// Rewrite `clip` to show `member`'s source at the same real moment
/// (Swift `MulticamEngine.rewrite`).
pub fn rewrite(
    clip: &mut Clip,
    group: &MulticamSource,
    member: &MulticamMember,
    source_durations: &HashMap<String, f64>,
    fps: i64,
) {
    if clip.media_ref == member.media_ref {
        return;
    }
    let Some(current) = group.member_by_media_ref(&clip.media_ref) else {
        return;
    };
    let delta =
        ((current.sync.offset_seconds - member.sync.offset_seconds) * fps as f64).round() as i64;
    clip.media_ref = member.media_ref.clone();
    clip.trim_start_frame += delta;
    if let Some(duration) = source_durations.get(&member.media_ref) {
        let source_len = (duration * fps as f64).round() as i64;
        clip.trim_end_frame =
            (source_len - clip.trim_start_frame - source_frames_consumed(clip)).max(0);
    } else {
        clip.trim_end_frame = 0;
    }
}

/// Track-local split at `frame`; `only_group` restricts to stamped clips.
fn split_in_track(track: &mut Track, frame: i64, only_group: Option<&str>) -> bool {
    let Some(i) = track.clips.iter().position(|c| {
        frame > c.start_frame
            && frame < c.end_frame()
            && only_group.is_none_or(|g| c.multicam_group_id.as_deref() == Some(g))
    }) else {
        return false;
    };
    let Some((left, right)) = split_values(&track.clips[i], frame) else {
        return false;
    };
    track.clips[i] = left;
    track.clips.insert(i + 1, right);
    true
}

fn is_through_edit(a: &Clip, b: &Clip) -> bool {
    a.media_ref == b.media_ref
        && a.media_type == b.media_type
        && a.multicam_group_id == b.multicam_group_id
        && b.start_frame == a.end_frame()
        && b.trim_start_frame == a.trim_start_frame + source_frames_consumed(a)
        && a.speed == b.speed
        && a.volume == b.volume
        && a.opacity == b.opacity
        && a.transform == b.transform
        && a.crop == b.crop
        && a.effects == b.effects
        && a.blend_mode == b.blend_mode
        && a.fade_out_frames == 0
        && b.fade_in_frames == 0
        && !has_keyframes(a)
        && !has_keyframes(b)
}

fn join_through_edits(track: &mut Track, ranges: &[Range<i64>], group_id: &str) -> usize {
    if ranges.is_empty() {
        return 0;
    }
    let mut merged = 0usize;
    let mut clips = track.clips.clone();
    clips.sort_by_key(|c| c.start_frame);
    let mut i = 0usize;
    while i + 1 < clips.len() {
        let seam = clips[i].end_frame();
        if clips[i].multicam_group_id.as_deref() == Some(group_id)
            && ranges.iter().any(|r| r.start <= seam && seam <= r.end)
            && is_through_edit(&clips[i], &clips[i + 1])
        {
            clips[i].duration_frames += clips[i + 1].duration_frames;
            clips[i].trim_end_frame = clips[i + 1].trim_end_frame;
            clips[i].fade_out_frames = clips[i + 1].fade_out_frames;
            clips.remove(i + 1);
            merged += 1;
        } else {
            i += 1;
        }
    }
    track.clips = clips;
    merged
}

fn clear_overlays(
    timeline: &mut Timeline,
    range: &Range<i64>,
    program_track_id: &str,
    group_id: &str,
) -> usize {
    let Some(program_idx) = track_index_by_id(timeline, program_track_id) else {
        return 0;
    };
    let mut removed = 0usize;
    for ti in 0..program_idx {
        let track = &mut timeline.tracks[ti];
        split_in_track(track, range.start, Some(group_id));
        split_in_track(track, range.end, Some(group_id));
        let before = track.clips.len();
        track.clips.retain(|c| {
            !(c.multicam_group_id.as_deref() == Some(group_id)
                && c.start_frame >= range.start
                && c.end_frame() <= range.end)
        });
        removed += before - track.clips.len();
    }
    removed
}

#[allow(clippy::too_many_arguments)]
fn place_overlay(
    timeline: &mut Timeline,
    member: &MulticamMember,
    range: &Range<i64>,
    anchor: &Clip,
    anchor_member: &MulticamMember,
    program_track_id: &str,
    group: &MulticamSource,
    source_durations: &HashMap<String, f64>,
    fps: i64,
    style: &dyn Fn(&Clip) -> (Transform, Crop),
) -> Option<String> {
    let mut clip = base_clip(&member.media_ref, range.start, range.end - range.start);
    clip.multicam_group_id = Some(group.id.clone());
    let group_frame = range.start - anchor_member.anchor_frame(anchor, fps);
    clip.trim_start_frame = group_frame - member.offset_frames(fps);
    if let Some(duration) = source_durations.get(&member.media_ref) {
        let source_len = (duration * fps as f64).round() as i64;
        clip.trim_end_frame =
            (source_len - clip.trim_start_frame - source_frames_consumed(&clip)).max(0);
    }
    let (transform, crop) = style(&clip);
    clip.transform = transform;
    clip.crop = crop;

    let program_idx = track_index_by_id(timeline, program_track_id)?;
    let free = timeline.tracks[..program_idx].iter().rposition(|t| {
        t.r#type == ClipType::Video && !t.clips.iter().any(|c| clip_overlaps(c, range))
    });
    let idx = match free {
        Some(i) => i,
        None => {
            timeline.tracks.insert(program_idx, new_video_track());
            program_idx
        }
    };
    let id = clip.id.clone();
    timeline.tracks[idx].clips.push(clip);
    timeline.tracks[idx].clips.sort_by_key(|c| c.start_frame);
    Some(id)
}

fn clamp_to_coverage(
    wanted: &Range<i64>,
    fragment: &Clip,
    current: &MulticamMember,
    target: &MulticamMember,
    source_durations: &HashMap<String, f64>,
    fps: i64,
) -> (Range<i64>, Option<String>) {
    let Some(duration) = source_durations.get(&target.media_ref) else {
        return (wanted.clone(), None);
    };
    let group_start = fragment.trim_start_frame as f64 / fps as f64 + current.sync.offset_seconds;
    let project_frame = |group_seconds: f64| -> i64 {
        fragment.start_frame + ((group_seconds - group_start) * fps as f64).round() as i64
    };
    let coverage = project_frame(target.sync.offset_seconds)
        ..project_frame(target.sync.offset_seconds + duration);
    let clamped = clamp_range(wanted, &coverage);
    let culprit = (clamped != *wanted).then(|| target.angle_label.clone());
    (clamped, culprit)
}

// ── Engine core ──────────────────────────────────────────────────────────

/// Apply angle switches (Swift `MulticamEngine.apply`). `fit_transform_for`
/// resolves a clip's default aspect-fit; `placement` frames a clip into a
/// layout slot rect.
pub fn apply(
    entries: &[MulticamEntry],
    timeline: &mut Timeline,
    group: &MulticamSource,
    source_durations: &HashMap<String, f64>,
    fit_transform_for: &dyn Fn(&Clip) -> Transform,
    placement: &dyn Fn(&Clip, LayoutRect) -> (Transform, Crop),
) -> MulticamOutcome {
    let mut outcome = MulticamOutcome::default();
    let fps = timeline.fps;

    for entry in entries.iter().filter(|e| !e.range.is_empty()) {
        let Some(program_id) = program_track_id(timeline, group, &entry.range) else {
            outcome
                .skipped
                .push((entry.range.clone(), "no multicam clip in this range".into()));
            continue;
        };
        let fragment_ids: Vec<String> = {
            let ti = track_index_by_id(timeline, &program_id).expect("track just found");
            timeline.tracks[ti]
                .clips
                .iter()
                .filter(|c| is_program_fragment(c, group) && clip_overlaps(c, &entry.range))
                .map(|c| c.id.clone())
                .collect()
        };

        for fragment_id in fragment_ids {
            let Some(fragment) = track_index_by_id(timeline, &program_id).and_then(|ti| {
                timeline.tracks[ti]
                    .clips
                    .iter()
                    .find(|c| c.id == fragment_id)
                    .cloned()
            }) else {
                continue;
            };
            let Some(member) = group.member_by_media_ref(&fragment.media_ref) else {
                continue;
            };

            let wanted = clamp_range(&entry.range, &(fragment.start_frame..fragment.end_frame()));
            let (target, culprit) = clamp_to_coverage(
                &wanted,
                &fragment,
                member,
                &entry.slots[0],
                source_durations,
                fps,
            );
            if target.is_empty() {
                outcome.skipped.push((
                    wanted,
                    format!(
                        "{} wasn't recording here",
                        culprit.as_deref().unwrap_or("an angle")
                    ),
                ));
                continue;
            }
            if let Some(culprit) = culprit {
                outcome.clamped.push(MulticamClamp {
                    requested: wanted.clone(),
                    applied: target.clone(),
                    culprit,
                });
            }

            let had_layout = clear_overlays(timeline, &target, &program_id, &group.id) > 0;
            let program_rect = entry.layout.slots().first().map(|s| s.rect);
            let layout_slots = entry.layout.slots();
            for (slot, slot_member) in layout_slots.iter().skip(1).zip(entry.slots.iter().skip(1)) {
                let rect = slot.rect;
                if let Some(id) = place_overlay(
                    timeline,
                    slot_member,
                    &target,
                    &fragment,
                    member,
                    &program_id,
                    group,
                    source_durations,
                    fps,
                    &|clip| placement(clip, rect),
                ) {
                    outcome.overlay_clip_ids.push(id);
                }
            }

            if let Some(ti) = track_index_by_id(timeline, &program_id) {
                let track = &mut timeline.tracks[ti];
                split_in_track(track, target.start, None);
                split_in_track(track, target.end, None);
                for i in 0..track.clips.len() {
                    let c = track.clips[i].clone();
                    if !(is_program_fragment(&c, group)
                        && c.start_frame >= target.start
                        && c.end_frame() <= target.end)
                    {
                        continue;
                    }
                    let was_default_fit =
                        c.transform == fit_transform_for(&c) && c.crop == Crop::default();
                    rewrite(
                        &mut track.clips[i],
                        group,
                        &entry.slots[0],
                        source_durations,
                        fps,
                    );
                    if entry.layout != VideoLayout::Full {
                        if let Some(rect) = program_rect {
                            let placed = placement(&track.clips[i], rect);
                            track.clips[i].transform = placed.0;
                            track.clips[i].crop = placed.1;
                        }
                    } else if was_default_fit || had_layout {
                        track.clips[i].transform = fit_transform_for(&track.clips[i]);
                        if had_layout {
                            track.clips[i].crop = Crop::default();
                        }
                    }
                    outcome.switched += 1;
                }
                outcome.merged +=
                    join_through_edits(track, std::slice::from_ref(&target), &group.id);
            }
            outcome.applied.push(target);
        }
    }
    outcome
}

// ── Validation / member resolution ───────────────────────────────────────

/// Usable members that put audio in the mix (Swift `multicamAudioBearers`):
/// audio providers plus cameras whose file carries audio.
pub fn multicam_audio_bearers<'a>(
    group: &'a MulticamSource,
    assets: &HashMap<String, MulticamAsset>,
) -> Vec<&'a MulticamMember> {
    group
        .members
        .iter()
        .filter(|m| {
            m.usable()
                && (m.provides_audio() || assets.get(&m.media_ref).is_some_and(|a| a.has_audio))
        })
        .collect()
}

/// Swift `resolveMember`: an angleLabel to a usable member, video or audio role.
pub fn resolve_member<'a>(
    label: &str,
    group: &'a MulticamSource,
    audio: bool,
    assets: &HashMap<String, MulticamAsset>,
) -> Result<&'a MulticamMember, String> {
    let noun = if audio { "mic" } else { "angle" };
    let candidates: Vec<&MulticamMember> = if audio {
        multicam_audio_bearers(group, assets)
    } else {
        group.angles()
    };
    let member = group.member_labeled(label).filter(|m| {
        if audio {
            candidates.iter().any(|c| c.id == m.id)
        } else {
            m.provides_video()
        }
    });
    let Some(member) = member else {
        let mut noun_cap = noun.to_string();
        noun_cap[..1].make_ascii_uppercase();
        return Err(format!(
            "Unknown {noun} '{label}'. {noun_cap}s: {}.",
            candidates
                .iter()
                .map(|m| m.angle_label.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    if !member.usable() {
        let mut noun_cap = noun.to_string();
        noun_cap[..1].make_ascii_uppercase();
        return Err(format!(
            "{noun_cap} '{label}' isn't synced — pin an offset or recreate the group."
        ));
    }
    Ok(member)
}

/// Validate requests + run the engine (Swift `switchMulticamAngles`).
pub fn switch_angles(
    timeline: &mut Timeline,
    group: &MulticamSource,
    requests: &[AngleSwitchRequest],
    assets: &HashMap<String, MulticamAsset>,
) -> Result<MulticamOutcome, String> {
    if multicam_clip_locations(timeline, &group.id).is_empty() {
        return Err("The group has no clips on the active timeline.".to_string());
    }
    let mut entries = Vec::with_capacity(requests.len());
    for request in requests {
        let slot_count = request.layout.slots().len();
        if request.angles.is_empty() || request.angles.len() > slot_count {
            return Err(format!(
                "Layout {} takes at most {slot_count} angle(s): {}.",
                request.layout.as_str(),
                request
                    .layout
                    .slots()
                    .iter()
                    .map(|s| s.id)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let mut slots = Vec::with_capacity(request.angles.len());
        for angle in &request.angles {
            slots.push(resolve_member(angle, group, false, assets)?.clone());
        }
        entries.push(MulticamEntry {
            range: request.range.clone(),
            slots,
            layout: request.layout,
        });
    }

    let durations = source_durations(group, assets);
    let (canvas_w, canvas_h) = (timeline.width, timeline.height);
    let dims_for = |clip: &Clip, assets: &HashMap<String, MulticamAsset>| -> (i64, i64) {
        assets
            .get(&clip.media_ref)
            .and_then(|a| a.source_width.zip(a.source_height))
            .unwrap_or((0, 0))
    };
    let assets_fit = assets.clone();
    let fit = move |clip: &Clip| -> Transform {
        let (sw, sh) = dims_for(clip, &assets_fit);
        fit_transform(sw, sh, canvas_w, canvas_h)
    };
    let assets_place = assets.clone();
    let place = move |clip: &Clip, rect: LayoutRect| -> (Transform, Crop) {
        let (sw, sh) = dims_for(clip, &assets_place);
        layout_placement(rect, LayoutFit::Fill, sw, sh, canvas_w, canvas_h, 0.5, 0.5)
    };
    Ok(apply(&entries, timeline, group, &durations, &fit, &place))
}

/// Source durations by mediaRef (Swift `multicamSourceDurations`).
pub fn source_durations(
    group: &MulticamSource,
    assets: &HashMap<String, MulticamAsset>,
) -> HashMap<String, f64> {
    group
        .members
        .iter()
        .filter_map(|m| {
            assets
                .get(&m.media_ref)
                .map(|a| (m.media_ref.clone(), a.duration))
        })
        .collect()
}

/// Manual per-clip switch (Swift `switchMulticamSegment`): mics and overlay
/// clips rewrite in place; a program fragment routes through `switch_angles`.
pub fn switch_segment(
    timeline: &mut Timeline,
    group: &MulticamSource,
    clip_id: &str,
    angle: &str,
    assets: &HashMap<String, MulticamAsset>,
) -> Result<(), String> {
    let Some(loc) = crate::edit::find_clip(timeline, clip_id) else {
        return Err(format!("Clip not found: {clip_id}"));
    };
    let clip = timeline.tracks[loc.track_index].clips[loc.clip_index].clone();
    let program_track = multicam_clip_locations(timeline, &group.id)
        .into_iter()
        .filter(|(ti, ci)| {
            timeline.tracks[*ti].r#type == ClipType::Video
                && timeline.tracks[*ti].clips[*ci].media_type != ClipType::Audio
        })
        .map(|(ti, _)| ti)
        .max();
    if clip.media_type == ClipType::Audio || Some(loc.track_index) != program_track {
        let member =
            resolve_member(angle, group, clip.media_type == ClipType::Audio, assets)?.clone();
        let durations = source_durations(group, assets);
        rewrite(
            &mut timeline.tracks[loc.track_index].clips[loc.clip_index],
            group,
            &member,
            &durations,
            timeline.fps,
        );
        return Ok(());
    }
    switch_angles(
        timeline,
        group,
        &[AngleSwitchRequest::full(
            clip.start_frame..clip.end_frame(),
            angle,
        )],
        assets,
    )
    .map(|_| ())
}

// ── Creation ─────────────────────────────────────────────────────────────

fn unique_angle_label(raw: &str, used: &mut HashSet<String>) -> String {
    let mut base = String::new();
    for c in raw.to_lowercase().chars() {
        let mapped = if c.is_alphanumeric() { c } else { '-' };
        if !(base.ends_with('-') && mapped == '-') {
            base.push(mapped);
        }
    }
    let mut base = base.trim_matches('-').to_string();
    if base.is_empty() {
        base = "angle".to_string();
    }
    let mut label = base.clone();
    let mut n = 2;
    while !used.insert(label.clone()) {
        label = format!("{base}-{n}");
        n += 1;
    }
    label
}

fn unique_group_name(existing: &[String]) -> String {
    let mut n = 1;
    loop {
        let candidate = format!("Multicam {n}");
        if !existing.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn make_member_clip(
    member: &MulticamMember,
    group_range: &Range<i64>,
    media_type: ClipType,
    group_id: &str,
    group_origin: i64,
    fps: i64,
    source_duration: Option<f64>,
    asset: Option<&MulticamAsset>,
    canvas: (i64, i64),
) -> Option<Clip> {
    let start = group_origin + group_range.start;
    let clamped_start = start.max(0);
    let clamped_end = group_origin + group_range.end;
    if clamped_end <= clamped_start {
        return None;
    }
    let head_cut = clamped_start - start;

    let mut clip = base_clip(
        &member.media_ref,
        clamped_start,
        clamped_end - clamped_start,
    );
    clip.media_type = media_type;
    clip.source_clip_type = asset.map(|a| a.clip_type).unwrap_or(media_type);
    clip.multicam_group_id = Some(group_id.to_string());
    clip.trim_start_frame = member.trim_frame(group_range.start, fps) + head_cut;
    if let Some(duration) = source_duration {
        let source_len = (duration * fps as f64).round() as i64;
        clip.trim_end_frame =
            (source_len - clip.trim_start_frame - source_frames_consumed(&clip)).max(0);
    }
    if media_type == ClipType::Video {
        if let Some((sw, sh)) = asset.and_then(|a| a.source_width.zip(a.source_height)) {
            clip.transform = fit_transform(sw, sh, canvas.0, canvas.1);
        }
    }
    Some(clip)
}

/// Build + place a multicam group (Swift `createMulticamGroup`): one program
/// video track whose spans hole-fill across covering angles, one audio track
/// per mic. Returns the group metadata (caller stores it) and the clip ids.
#[allow(clippy::too_many_arguments)]
pub fn create_group(
    timeline: &mut Timeline,
    specs: &[MulticamMemberSpec],
    sync_maps: &HashMap<String, MulticamSyncMap>,
    master_ref: &str,
    name: Option<&str>,
    existing_group_names: &[String],
    assets: &HashMap<String, MulticamAsset>,
    start_frame: Option<i64>,
) -> Result<(MulticamSource, Vec<String>), String> {
    let mut members: Vec<MulticamMember> = Vec::with_capacity(specs.len());
    let mut used_labels: HashSet<String> = HashSet::new();
    for spec in specs {
        let raw_label = spec
            .angle_label
            .clone()
            .or_else(|| assets.get(&spec.media_ref).map(|a| a.name.clone()))
            .unwrap_or_else(|| spec.media_ref.clone());
        members.push(MulticamMember {
            id: uuid::Uuid::new_v4().to_string(),
            media_ref: spec.media_ref.clone(),
            kind: spec.kind,
            angle_label: unique_angle_label(&raw_label, &mut used_labels),
            sync: sync_maps.get(&spec.media_ref).cloned().unwrap_or_default(),
        });
    }
    let Some(master) = members.iter().find(|m| m.media_ref == master_ref) else {
        return Err("Master member not found among members.".to_string());
    };

    let group = MulticamSource {
        id: uuid::Uuid::new_v4().to_string(),
        name: name
            .map(str::to_string)
            .unwrap_or_else(|| unique_group_name(existing_group_names)),
        master_member_id: master.id.clone(),
        members,
    };
    let durations = source_durations(&group, assets);
    let fps = timeline.fps;
    let at = start_frame.unwrap_or_else(|| timeline.total_frames());

    let angles = group.angles();
    let angle_ranges: Vec<Range<i64>> = angles
        .iter()
        .filter_map(|a| durations.get(&a.media_ref).map(|d| a.coverage(*d, fps)))
        .collect();
    let video_start = angle_ranges.iter().map(|r| r.start).min();
    let video_end = angle_ranges.iter().map(|r| r.end).max();
    let seed = angles
        .iter()
        .find(|a| durations.contains_key(&a.media_ref))
        .copied();
    let (Some(video_start), Some(video_end), Some(seed)) = (video_start, video_end, seed) else {
        return Err("No synced camera has picture — nothing to place.".to_string());
    };
    if video_start >= video_end {
        return Err("No synced camera has picture — nothing to place.".to_string());
    }

    let mut clip_ids: Vec<String> = Vec::new();
    let group_origin = at - video_start;

    // Hole-fill: the seed angle first, then remaining angles fill uncovered spans.
    let mut program_spans: Vec<(MulticamMember, Range<i64>)> = Vec::new();
    let mut holes: Vec<Range<i64>> = vec![video_start..video_end];
    let ordered: Vec<&MulticamMember> = std::iter::once(seed)
        .chain(angles.iter().copied().filter(|a| a.id != seed.id))
        .collect();
    for angle in ordered {
        let Some(duration) = durations.get(&angle.media_ref) else {
            continue;
        };
        let coverage = angle.coverage(*duration, fps);
        let mut remaining: Vec<Range<i64>> = Vec::new();
        for hole in &holes {
            let filled = clamp_range(hole, &coverage);
            if filled.is_empty() {
                remaining.push(hole.clone());
                continue;
            }
            program_spans.push((angle.clone(), filled.clone()));
            if hole.start < filled.start {
                remaining.push(hole.start..filled.start);
            }
            if filled.end < hole.end {
                remaining.push(filled.end..hole.end);
            }
        }
        holes = remaining;
        if holes.is_empty() {
            break;
        }
    }
    program_spans.sort_by_key(|(_, r)| r.start);

    let canvas = (timeline.width, timeline.height);
    let video_idx = crate::track_ops::insert_track_at(timeline, 0, ClipType::Video)
        .map_err(|e| format!("Could not create the multicam program track: {e:?}"))?;
    for (member, span) in &program_spans {
        if let Some(clip) = make_member_clip(
            member,
            span,
            ClipType::Video,
            &group.id,
            group_origin,
            fps,
            durations.get(&member.media_ref).copied(),
            assets.get(&member.media_ref),
            canvas,
        ) {
            clip_ids.push(clip.id.clone());
            timeline.tracks[video_idx].clips.push(clip);
        }
    }

    for mic in group.mics() {
        let Some(duration) = durations.get(&mic.media_ref) else {
            continue;
        };
        let Some(clip) = make_member_clip(
            mic,
            &mic.coverage(*duration, fps),
            ClipType::Audio,
            &group.id,
            group_origin,
            fps,
            Some(*duration),
            assets.get(&mic.media_ref),
            canvas,
        ) else {
            continue;
        };
        let idx =
            crate::track_ops::insert_track_at(timeline, timeline.tracks.len(), ClipType::Audio)
                .map_err(|e| format!("Could not create a mic track: {e:?}"))?;
        clip_ids.push(clip.id.clone());
        timeline.tracks[idx].clips.push(clip);
    }

    if clip_ids.is_empty() {
        return Err("Could not place the multicam on the timeline.".to_string());
    }
    Ok((group, clip_ids))
}

/// Strip the group's stamps on this timeline; clips stay put as ordinary
/// clips (Swift `ungroupMulticam`'s timeline half — metadata removal is the
/// caller's, since the group store lives with the host).
pub fn strip_group_stamps(timeline: &mut Timeline, group_id: &str) {
    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            if clip.multicam_group_id.as_deref() == Some(group_id) {
                clip.multicam_group_id = None;
            }
        }
    }
}

// ── Program read ─────────────────────────────────────────────────────────

/// Run-length program rows `[angleLabel, startFrame, endFrame)` on the
/// group's program track (Swift `multicamProgramRows`).
pub fn program_rows(
    timeline: &Timeline,
    group: &MulticamSource,
    window: Option<Range<i64>>,
) -> Vec<(String, i64, i64)> {
    let program_locs: Vec<(usize, usize)> = multicam_clip_locations(timeline, &group.id)
        .into_iter()
        .filter(|(ti, ci)| {
            timeline.tracks[*ti].r#type == ClipType::Video
                && timeline.tracks[*ti].clips[*ci].media_type != ClipType::Audio
        })
        .collect();
    let Some(program_track) = program_locs.iter().map(|(ti, _)| *ti).max() else {
        return Vec::new();
    };

    let mut clips: Vec<&Clip> = program_locs
        .iter()
        .filter(|(ti, _)| *ti == program_track)
        .map(|(ti, ci)| &timeline.tracks[*ti].clips[*ci])
        .collect();
    clips.sort_by_key(|c| c.start_frame);

    let mut rows: Vec<(String, i64, i64)> = Vec::new();
    for clip in clips {
        let mut r = clip.start_frame..clip.end_frame();
        if let Some(w) = &window {
            r = clamp_range(&r, w);
        }
        if r.is_empty() {
            continue;
        }
        let label = group
            .member_by_media_ref(&clip.media_ref)
            .map(|m| m.angle_label.clone())
            .unwrap_or_default();
        match rows.last_mut() {
            Some(last) if last.0 == label && last.2 == r.start => last.2 = r.end,
            _ => rows.push((label, r.start, r.end)),
        }
    }
    rows
}

// ── Guardrails ───────────────────────────────────────────────────────────

/// Track display label mirroring the agent surface: visual tracks V-numbered
/// bottom-up, audio tracks A-numbered top-down within the audio zone.
fn track_display_label(timeline: &Timeline, index: usize) -> String {
    if timeline.tracks[index].r#type == ClipType::Audio {
        let n = timeline.tracks[..=index]
            .iter()
            .filter(|t| t.r#type == ClipType::Audio)
            .count();
        format!("A{n}")
    } else {
        let n = timeline.tracks[index..]
            .iter()
            .filter(|t| t.r#type != ClipType::Audio)
            .count();
        format!("V{n}")
    }
}

fn group_name(groups: &[MulticamSource], group_id: &str) -> String {
    groups
        .iter()
        .find(|g| g.id == group_id)
        .map(|g| g.name.clone())
        .unwrap_or_else(|| "Multicam".to_string())
}

/// Swift `multicamMoveViolation`: moves are `(clip_id, dest_track, dest_frame)`
/// with defaults already resolved. Horizontal shifts of a group subset and
/// camera lane changes are refused; whole-group moves pass.
pub fn move_violation(
    timeline: &Timeline,
    groups: &[MulticamSource],
    moves: &[(String, usize, i64)],
) -> Option<String> {
    struct Info {
        id: String,
        group_id: Option<String>,
        is_audio: bool,
        current_track: usize,
        start_frame: i64,
        to_track: usize,
        to_frame: i64,
    }
    let infos: Vec<Info> = moves
        .iter()
        .filter_map(|(id, to_track, to_frame)| {
            let loc = crate::edit::find_clip(timeline, id)?;
            let clip = &timeline.tracks[loc.track_index].clips[loc.clip_index];
            Some(Info {
                id: id.clone(),
                group_id: clip.multicam_group_id.clone(),
                is_audio: clip.media_type == ClipType::Audio,
                current_track: loc.track_index,
                start_frame: clip.start_frame,
                to_track: *to_track,
                to_frame: *to_frame,
            })
        })
        .collect();
    let moved_ids: HashSet<&str> = infos.iter().map(|i| i.id.as_str()).collect();
    let horizontal = infos.iter().any(|i| i.start_frame != i.to_frame);
    let lane_change = infos
        .iter()
        .any(|i| i.group_id.is_some() && !i.is_audio && i.current_track != i.to_track);
    if !horizontal && !lane_change {
        return None;
    }
    if lane_change {
        return Some(
            "Can't move a multicam camera clip to another track — the group's program track stays fixed."
                .to_string(),
        );
    }
    let group_ids: HashSet<String> = infos.iter().filter_map(|i| i.group_id.clone()).collect();
    for gid in group_ids {
        let left_behind = multicam_clip_locations(timeline, &gid)
            .into_iter()
            .any(|(ti, ci)| !moved_ids.contains(timeline.tracks[ti].clips[ci].id.as_str()));
        if left_behind {
            return Some(format!(
                "Can't move part of multicam group \"{}\" — its clips stay in sync and move together.",
                group_name(groups, &gid)
            ));
        }
    }
    None
}

/// Swift `multicamAtomicityViolation`: shifting only SOME of a group's tracks
/// would desync the members left behind.
pub fn atomicity_violation(
    timeline: &Timeline,
    groups: &[MulticamSource],
    shifting_track_indices: &HashSet<usize>,
) -> Option<String> {
    let mut group_tracks: HashMap<String, BTreeSet<usize>> = HashMap::new();
    for (ti, track) in timeline.tracks.iter().enumerate() {
        for gid in track
            .clips
            .iter()
            .filter_map(|c| c.multicam_group_id.as_deref())
            .collect::<HashSet<_>>()
        {
            group_tracks.entry(gid.to_string()).or_default().insert(ti);
        }
    }
    for (gid, track_set) in &group_tracks {
        let moving: BTreeSet<usize> = track_set
            .iter()
            .copied()
            .filter(|ti| shifting_track_indices.contains(ti))
            .collect();
        if moving.is_empty() || moving == *track_set {
            continue;
        }
        let stranded: Vec<String> = track_set
            .iter()
            .filter(|ti| !shifting_track_indices.contains(ti))
            .map(|ti| track_display_label(timeline, *ti))
            .collect();
        return Some(format!(
            "Can't shift part of multicam group \"{}\" — {} would stay behind and desync.",
            group_name(groups, gid),
            stranded.join(", ")
        ));
    }
    None
}

/// Swift `multicamManualRippleViolation`: atomicity plus "no ripple through
/// the middle of a group clip" for manual (non-range) ripples.
pub fn manual_ripple_violation(
    timeline: &Timeline,
    groups: &[MulticamSource],
    shifting_track_indices: &HashSet<usize>,
    at_frame: i64,
) -> Option<String> {
    if let Some(reason) = atomicity_violation(timeline, groups, shifting_track_indices) {
        return Some(reason);
    }
    for ti in shifting_track_indices {
        let Some(track) = timeline.tracks.get(*ti) else {
            continue;
        };
        if let Some(clip) = track.clips.iter().find(|c| {
            c.multicam_group_id.is_some() && c.start_frame < at_frame && c.end_frame() > at_frame
        }) {
            let gid = clip.multicam_group_id.as_deref().unwrap_or_default();
            return Some(format!(
                "Can't ripple through multicam group \"{}\" — split its clips at the edit point, or remove silence/words to cut time.",
                group_name(groups, gid)
            ));
        }
    }
    None
}

/// Swift `multicamTrimBounds`: how far a stamped clip may trim outward before
/// hitting a differently-anchored neighbour (a ripple seam) — the caller also
/// caps at source coverage via the returned trim values.
pub fn trim_bounds(
    timeline: &Timeline,
    group: &MulticamSource,
    clip_id: &str,
) -> Option<(i64, i64)> {
    let loc = crate::edit::find_clip(timeline, clip_id)?;
    let clip = &timeline.tracks[loc.track_index].clips[loc.clip_index];
    let member = group.member_by_media_ref(&clip.media_ref)?;
    let fps = timeline.fps;
    let own = member.anchor_frame(clip, fps);
    let mut left = clip.trim_start_frame;
    let mut right = clip.trim_end_frame;
    for (ti, ci) in multicam_clip_locations(timeline, &group.id) {
        let other = &timeline.tracks[ti].clips[ci];
        if other.id == clip.id {
            continue;
        }
        let Some(m) = group.member_by_media_ref(&other.media_ref) else {
            continue;
        };
        if m.anchor_frame(other, fps) == own {
            continue;
        }
        if other.end_frame() <= clip.start_frame {
            left = left.min(clip.start_frame - other.end_frame());
        }
        if other.start_frame >= clip.end_frame() {
            right = right.min(other.start_frame - clip.end_frame());
        }
    }
    Some((left.max(0), right.max(0)))
}

/// Out-of-sync offsets for stamped clips (the multicam half of Swift
/// `linkGroupOffsets`): clips clustered by overlapping ranges within a group
/// should share one anchor; deviants map to `clip_id → offset`.
pub fn multicam_group_offsets(
    timeline: &Timeline,
    groups: &[MulticamSource],
) -> HashMap<String, i64> {
    let fps = timeline.fps;
    let members_by_group: HashMap<&str, HashMap<&str, &MulticamMember>> = groups
        .iter()
        .map(|g| {
            (
                g.id.as_str(),
                g.members
                    .iter()
                    .map(|m| (m.media_ref.as_str(), m))
                    .collect(),
            )
        })
        .collect();

    let mut by_group: HashMap<&str, Vec<(String, i64, Range<i64>)>> = HashMap::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            let Some(gid) = clip.multicam_group_id.as_deref() else {
                continue;
            };
            let Some(member) = members_by_group
                .get(gid)
                .and_then(|m| m.get(clip.media_ref.as_str()))
            else {
                continue;
            };
            by_group.entry(gid).or_default().push((
                clip.id.clone(),
                member.anchor_frame(clip, fps),
                clip.start_frame..clip.end_frame(),
            ));
        }
    }

    let mut offsets: HashMap<String, i64> = HashMap::new();
    for (_, mut entries) in by_group {
        if entries.len() < 2 {
            continue;
        }
        entries.sort_by_key(|(_, _, r)| r.start);
        let mut cluster: Vec<&(String, i64, Range<i64>)> = Vec::new();
        let mut cluster_end = i64::MIN;
        let flush = |cluster: &[&(String, i64, Range<i64>)], offsets: &mut HashMap<String, i64>| {
            if cluster.len() < 2 {
                return;
            }
            let anchor_ref = cluster.iter().map(|(_, a, _)| *a).min().unwrap();
            for (id, anchor, _) in cluster {
                if *anchor != anchor_ref {
                    offsets.insert(id.clone(), anchor - anchor_ref);
                }
            }
        };
        for entry in &entries {
            if entry.2.start >= cluster_end {
                flush(&cluster, &mut offsets);
                cluster.clear();
            }
            cluster_end = cluster_end.max(entry.2.end);
            cluster.push(entry);
        }
        flush(&cluster, &mut offsets);
    }
    offsets
}
