//! FCPXML export (Final Cut Pro X / DaVinci Resolve interchange).
//! Upstream palmier-pro #193.
//!
//! v1 baseline: emits a valid FCPXML 1.10 document with a `<resources>` block
//! (one `<format>` plus deduped `<asset>` per media reference) and a `<spine>`
//! whose single full-length `<gap>` anchors every clip as a connected
//! `<asset-clip>` at its absolute project offset, stacked by lane (positive for
//! video tracks, negative for audio). Time is expressed in rational project
//! frames (`frames/fps s`).
//!
//! Deliberately out of scope for v1 (tracked as follow-ups):
//! - retime / compound `<ref-clip>` wrapping (#197)
//! - FCP/Resolve format naming + colorspace (#214)
//! - source timecode (#247)
//! - per-target transform/crop/blend calibration (#254)
//!
//! Done since v1: per-asset formats + A/V linked-audio collapse (#206).

use core_model::{Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Timeline};
use std::collections::BTreeMap;
use std::fmt::Write;

pub struct FcpxmlExport;

/// Which NLE the FCPXML is calibrated for (upstream #254). Resolve and Final Cut interpret
/// `<adjust-transform>`/`<adjust-crop>` values differently: Resolve wants values relative to the
/// aspect-fit source, FCP wants raw frame-relative values. Defaults to Resolve (Swift's default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FcpxmlTarget {
    #[default]
    Resolve,
    Fcp,
}

impl FcpxmlExport {
    /// Generate an FCPXML 1.10 document for `timeline` (Resolve calibration — Swift's default),
    /// resolving asset paths and source durations from `manifest`.
    pub fn export(timeline: &Timeline, manifest: &MediaManifest) -> String {
        Self::export_with_target(timeline, manifest, FcpxmlTarget::Resolve)
    }

    /// Generate an FCPXML 1.10 document calibrated for a specific NLE `target` (#254).
    pub fn export_with_target(
        timeline: &Timeline,
        manifest: &MediaManifest,
        target: FcpxmlTarget,
    ) -> String {
        Self::export_with_target_and_timelines(
            timeline,
            manifest,
            target,
            &std::collections::HashMap::new(),
        )
    }

    /// Like [`FcpxmlExport::export_with_target`] but resolves nested-timeline
    /// carriers (upstream #255) against the project's sibling timelines. v1
    /// flattens nests into their constituents (content-correct); emitting
    /// `<media><sequence>` compound resources + `<ref-clip>`s like Swift is a
    /// follow-up.
    pub fn export_with_target_and_timelines(
        timeline: &Timeline,
        manifest: &MediaManifest,
        target: FcpxmlTarget,
        timelines: &std::collections::HashMap<String, Timeline>,
    ) -> String {
        let flattened;
        let timeline = if timelines.is_empty() {
            timeline
        } else {
            flattened =
                timeline_core::flatten_nests(timeline, &|id: &str| timelines.get(id).cloned());
            &flattened
        };

        let fps = timeline.fps.max(1);
        let total = timeline_total_frames(timeline).max(1);

        // Assign one FCPXML lane per track: video tracks stack up (+), audio down (-).
        // Higher positive lane = further above (on top), so `tracks[0]` (the top
        // visual layer) must get the HIGHEST video lane, not lane 1.
        // Same-lane clips are the track's own non-overlapping clips.
        let num_video = timeline
            .tracks
            .iter()
            .filter(|t| t.r#type != ClipType::Audio)
            .count() as i64;
        let mut video_seen = 0i64;
        let mut audio_seen = 0i64;
        let mut lane_of_track: Vec<i64> = Vec::with_capacity(timeline.tracks.len());
        for track in &timeline.tracks {
            if track.r#type == ClipType::Audio {
                audio_seen += 1;
                lane_of_track.push(-audio_seen);
            } else {
                video_seen += 1;
                lane_of_track.push(num_video - video_seen + 1);
            }
        }

        let seq_w = timeline.width.max(1);
        let seq_h = timeline.height.max(1);

        // Dedup by media_ref (first-seen). r1 is the sequence format; each visual
        // asset gets its own <format> derived from its source dimensions
        // (upstream #206/#214). Audio assets carry no video format.
        let mut asset_ids: BTreeMap<String, String> = BTreeMap::new();
        // (media_ref, asset_id, format_id) in resource order.
        let mut resources: Vec<(String, String, Option<String>)> = Vec::new();
        let mut counter = 2usize;
        for track in &timeline.tracks {
            for clip in &track.clips {
                if clip.media_ref.is_empty()
                    || clip.media_type == ClipType::Text
                    || clip.media_type == ClipType::Shape
                {
                    continue;
                }
                if asset_ids.contains_key(&clip.media_ref) {
                    continue;
                }
                let entry = manifest.entry_for(&clip.media_ref);
                let media_type = entry
                    .map(|e| e.r#type.clone())
                    .unwrap_or_else(|| clip.media_type.clone());
                let has_video = media_type != ClipType::Audio;
                let has_dims = entry.and_then(|e| e.source_width).is_some()
                    && entry.and_then(|e| e.source_height).is_some();
                let format_id = if has_video && has_dims {
                    let fid = format!("r{counter}");
                    counter += 1;
                    Some(fid)
                } else {
                    None
                };
                let asset_id = format!("r{counter}");
                counter += 1;
                asset_ids.insert(clip.media_ref.clone(), asset_id.clone());
                resources.push((clip.media_ref.clone(), asset_id, format_id));
            }
        }

        // media_ref → its per-asset format id (None = audio-only or video without
        // dimensions). Lets each asset-clip reference its OWN format instead of
        // hardcoding the sequence format r1.
        let format_by_ref: BTreeMap<String, Option<String>> = resources
            .iter()
            .map(|(mref, _aid, fid)| (mref.clone(), fid.clone()))
            .collect();

        let mut xml = String::new();
        writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").ok();
        writeln!(xml, "<!DOCTYPE fcpxml>").ok();
        writeln!(xml, "<fcpxml version=\"1.10\">").ok();
        writeln!(xml, "  <resources>").ok();
        writeln!(
            xml,
            "    <format id=\"r1\" name=\"{}\" frameDuration=\"{}\" width=\"{seq_w}\" height=\"{seq_h}\" colorSpace=\"1-1-1 (Rec. 709)\"/>",
            sequence_format_name(seq_w, seq_h, fps as f64),
            frame_duration_str(fps as f64),
        )
        .ok();
        for (media_ref, asset_id, format_id) in &resources {
            let entry = manifest.entry_for(media_ref);
            if let Some(fid) = format_id {
                let w = entry.and_then(|e| e.source_width).unwrap_or(seq_w).max(1);
                let h = entry.and_then(|e| e.source_height).unwrap_or(seq_h).max(1);
                // Keep the source RESOLUTION, but the frameDuration must be the PROJECT
                // fps, NOT source_fps: the asset <duration> and every asset-clip
                // <start>/<duration> are emitted on the project-frame grid (the model
                // conforms sources to the project timebase, like XMEML). A source-fps
                // frameDuration would leave those times off the asset's own grid and
                // Final Cut would conform-snap them to the wrong frame.
                let f = fps as f64;
                writeln!(
                    xml,
                    "    <format id=\"{fid}\" name=\"{}\" frameDuration=\"{}\" width=\"{w}\" height=\"{h}\" colorSpace=\"1-1-1 (Rec. 709)\"/>",
                    video_format_name(w, h, f),
                    frame_duration_str(f),
                )
                .ok();
            }
            write_asset(&mut xml, asset_id, format_id.as_deref(), media_ref, manifest, fps);
        }
        // Title generator effect, emitted once when the timeline has any text overlay (#254).
        let has_titles = timeline.tracks.iter().flat_map(|t| &t.clips).any(|c| {
            c.media_type == ClipType::Text
                && c.text_content.as_ref().is_some_and(|s| !s.is_empty())
        });
        if has_titles {
            writeln!(
                xml,
                "    <effect id=\"titleBasic\" name=\"Basic Title\" uid=\".../Titles.localized/Bumper:Opener.localized/Basic Title.localized/Basic Title.moti\"/>"
            )
            .ok();
        }
        writeln!(xml, "  </resources>").ok();

        writeln!(xml, "  <library>").ok();
        writeln!(xml, "    <event name=\"Fronda\">").ok();
        writeln!(xml, "      <project name=\"Timeline\">").ok();
        writeln!(
            xml,
            "        <sequence format=\"r1\" duration=\"{}\" tcStart=\"0s\" tcFormat=\"NDF\">",
            time_str(total, fps)
        )
        .ok();
        writeln!(xml, "          <spine>").ok();
        writeln!(
            xml,
            "            <gap name=\"Gap\" offset=\"0s\" duration=\"{}\">",
            time_str(total, fps)
        )
        .ok();

        // A synced A/V pair (same source, timing, trim, speed) collapses into the
        // single video asset-clip — the asset already carries audio — so the audio
        // partner is dropped (upstream #206/#254).
        let (redundant_audio, collapsed_audio_vol) = redundant_audio_clip_ids(timeline);

        let mut title_style_id = 0usize;
        for (ti, track) in timeline.tracks.iter().enumerate() {
            let lane = lane_of_track[ti];
            // A hidden video track / muted audio track exports its clips disabled.
            let track_disabled = if track.r#type == ClipType::Audio {
                track.muted
            } else {
                track.hidden
            };
            for clip in &track.clips {
                // Text overlays become <title> generators; they have no backing asset.
                if clip.media_type == ClipType::Text {
                    if let Some(content) = clip.text_content.as_ref().filter(|c| !c.is_empty()) {
                        write_title(
                            &mut xml,
                            clip,
                            content,
                            lane,
                            fps,
                            seq_w,
                            seq_h,
                            track_disabled,
                            &mut title_style_id,
                        );
                    }
                    continue;
                }
                if clip.media_ref.is_empty()
                    || clip.media_type == ClipType::Shape
                    || redundant_audio.contains(&clip.id)
                {
                    continue;
                }
                let Some(ref_id) = asset_ids.get(&clip.media_ref) else {
                    continue;
                };
                // Reference the clip's OWN format: an audio-only asset-clip inherits
                // from its (video-format-less) asset — omit `format`; a video clip
                // uses its per-asset format, falling back to the sequence format r1
                // only when it has no dimensions. Hardcoding r1 mislabels a clip's
                // native size/rate and points audio clips at a video format.
                let is_audio = manifest
                    .entry_for(&clip.media_ref)
                    .map(|e| e.r#type == ClipType::Audio)
                    .unwrap_or(clip.media_type == ClipType::Audio);
                let format_attr = if is_audio {
                    String::new()
                } else {
                    let fid = format_by_ref
                        .get(&clip.media_ref)
                        .and_then(|o| o.as_deref())
                        .unwrap_or("r1");
                    format!(" format=\"{fid}\"")
                };
                let origin = start_timecode_frames(manifest.entry_for(&clip.media_ref), fps);
                // A hidden video track / muted audio track exports its clips disabled, so the
                // export mirrors what's actually visible/audible (Swift emits `enabled`).
                let enabled_attr = if track_disabled { " enabled=\"0\"" } else { "" };
                // #197: a retimed clip (speed != 1) carries a <timeMap> and its in-point is on the
                // retimed axis (trim/speed); a 1x clip's in-point is the source origin + trim (#247).
                let retimed = (clip.speed - 1.0).abs() > 0.001;
                let start_str = if retimed {
                    let (p, q) = rational_speed(clip.speed);
                    rational_time(clip.trim_start_frame.max(0) * q, fps * p)
                } else {
                    time_str(origin + clip.trim_start_frame.max(0), fps)
                };
                let time_map = if retimed {
                    let media_frames = manifest
                        .entry_for(&clip.media_ref)
                        .map(|e| (e.duration * fps as f64).round() as i64)
                        .unwrap_or(0);
                    build_time_map(clip.speed, origin, media_frames, fps)
                } else {
                    String::new()
                };
                let open = format!(
                    "              <asset-clip ref=\"{ref_id}\" lane=\"{lane}\" offset=\"{}\" name=\"{}\" duration=\"{}\" start=\"{start_str}\"{format_attr}{enabled_attr}",
                    time_str(clip.start_frame, fps),
                    xml_escape(&file_name(manifest, &clip.media_ref)),
                    time_str(clip.duration_frames.max(1), fps),
                );
                // Children in Swift's order: timeMap, then crop/conform/transform/blend/volume.
                let mut children = time_map;
                children.push_str(&clip_adjustments(
                    clip,
                    manifest,
                    is_audio,
                    seq_w,
                    seq_h,
                    fps,
                    collapsed_audio_vol.get(&clip.id).copied(),
                    target,
                ));
                if children.is_empty() {
                    writeln!(xml, "{open}/>").ok();
                } else {
                    writeln!(xml, "{open}>").ok();
                    xml.push_str(&children);
                    writeln!(xml, "              </asset-clip>").ok();
                }
            }
        }

        writeln!(xml, "            </gap>").ok();
        writeln!(xml, "          </spine>").ok();
        writeln!(xml, "        </sequence>").ok();
        writeln!(xml, "      </project>").ok();
        writeln!(xml, "    </event>").ok();
        writeln!(xml, "  </library>").ok();
        writeln!(xml, "</fcpxml>").ok();
        xml
    }
}

/// Emit an `<asset>`. `format_ref` is the asset's own per-asset format id when
/// it has one; visual assets without one fall back to the sequence format `r1`,
/// and audio-only assets carry no `format` attribute.
/// Ids of audio clips that are the redundant partner of a synced A/V pair — the
/// linked video's asset-clip already covers the audio, so the audio clip is dropped
/// on export (upstream #206/#254). A pair collapses only when its group holds exactly
/// one video/image and one audio that share source, placement, trim, speed, AND
/// enabled state. `enabled` derives from the TRACK (video/image → `!hidden`, audio →
/// `!muted`), so a MUTED audio partner is NOT collapsed — folding it into the video
/// asset-clip (which carries audio) would make the muted audio audible in the export.
/// Synced-A/V-pair collapse (#206/#254): the dropped audio clip ids, plus a map from the
/// surviving VIDEO clip id → the collapsed audio partner's volume (the audio clip carries the
/// gain; the video clip's own volume is often a default 1.0). Mirrors Swift's `linkedAudio ?? clip`
/// volume source for the collapsed asset-clip.
fn redundant_audio_clip_ids(
    timeline: &Timeline,
) -> (
    std::collections::HashSet<String>,
    std::collections::HashMap<String, f64>,
) {
    // (clip, enabled) grouped by link group.
    let mut by_group: std::collections::HashMap<&str, (Vec<(&Clip, bool)>, Vec<(&Clip, bool)>)> =
        std::collections::HashMap::new();
    for track in &timeline.tracks {
        for clip in &track.clips {
            let Some(group) = clip.link_group_id.as_deref() else {
                continue;
            };
            let bucket = by_group.entry(group).or_default();
            match clip.media_type {
                ClipType::Video | ClipType::Image => bucket.0.push((clip, !track.hidden)),
                ClipType::Audio => bucket.1.push((clip, !track.muted)),
                _ => {}
            }
        }
    }
    let mut redundant = std::collections::HashSet::new();
    let mut video_audio_volume = std::collections::HashMap::new();
    for (videos, audios) in by_group.into_values() {
        if videos.len() != 1 || audios.len() != 1 {
            continue;
        }
        let (v, v_enabled) = videos[0];
        let (a, a_enabled) = audios[0];
        if v.media_ref == a.media_ref
            && v_enabled == a_enabled
            && v.start_frame == a.start_frame
            && v.duration_frames == a.duration_frames
            && v.trim_start_frame == a.trim_start_frame
            && (v.speed - a.speed).abs() < 0.0001
        {
            redundant.insert(a.id.clone());
            video_audio_volume.insert(v.id.clone(), a.volume);
        }
    }
    (redundant, video_audio_volume)
}

fn write_asset(
    xml: &mut String,
    id: &str,
    format_ref: Option<&str>,
    media_ref: &str,
    manifest: &MediaManifest,
    fps: i64,
) {
    let entry = manifest.entry_for(media_ref);
    let name = file_name(manifest, media_ref);
    let (has_video, has_audio) = match entry.map(|e| &e.r#type) {
        Some(ClipType::Audio) => (false, true),
        Some(ClipType::Image) => (true, false),
        Some(_) | None => (true, entry.and_then(|e| e.has_audio).unwrap_or(false)),
    };
    let duration_frames = entry
        .map(|e| ((e.duration * fps as f64).round() as i64).max(1))
        .unwrap_or(1);
    let src = entry.map(|e| media_src(&e.source)).unwrap_or_default();
    let format_attr = if has_video {
        format!(" format=\"{}\"", format_ref.unwrap_or("r1"))
    } else {
        String::new()
    };
    // #247: the asset's `start` is its embedded source timecode — FCP/Resolve read it as the
    // asset's timecode origin, so a non-zero embedded timecode isn't flagged as a mismatch.
    let tc = time_str(start_timecode_frames(entry, fps), fps);

    writeln!(
        xml,
        "    <asset id=\"{id}\" name=\"{}\" start=\"{tc}\" duration=\"{}\" hasVideo=\"{}\" hasAudio=\"{}\"{format_attr}>",
        xml_escape(&name),
        time_str(duration_frames, fps),
        if has_video { 1 } else { 0 },
        if has_audio { 1 } else { 0 },
    )
    .ok();
    writeln!(
        xml,
        "      <media-rep kind=\"original-media\" src=\"{}\"/>",
        xml_escape(&src)
    )
    .ok();
    writeln!(xml, "    </asset>").ok();
}

/// The asset's embedded start timecode in project-frame units (upstream #247). Cameras often
/// embed a running timecode, so footage starts non-zero. `frames(atFPS) = round(frame/quanta*fps)`
/// (the tmcd track's `quanta` may differ from the project fps); 0 when no timecode is recorded.
fn start_timecode_frames(entry: Option<&MediaManifestEntry>, fps: i64) -> i64 {
    match entry.and_then(|e| e.source_timecode_frame.zip(e.source_timecode_quanta)) {
        Some((frame, quanta)) if quanta > 0 => {
            (frame as f64 / quanta as f64 * fps as f64).round() as i64
        }
        _ => 0,
    }
}

/// Clip adjustments as `<asset-clip>` children (upstream #254, Resolve target): geometry
/// (`<adjust-crop>` + `<adjust-conform type="fit">` + `<adjust-transform>`) then `<adjust-blend>`
/// (opacity) then `<adjust-volume>` (dB), in Swift's child order. Emitted only when non-default,
/// so a plain full-frame clip stays self-closing. `conform` is emitted with any geometry so the
/// fit-relative scale/position stay consistent. Deferred: keyframed transform/opacity animation,
/// the FCP target's alternate value encoding, and same-aspect/different-resolution auto-fit for
/// clips with NO explicit transform (they stay native rather than filling the frame).
#[allow(clippy::too_many_arguments)]
fn clip_adjustments(
    clip: &Clip,
    manifest: &MediaManifest,
    is_audio: bool,
    seq_w: i64,
    seq_h: i64,
    fps: i64,
    collapsed_audio_volume: Option<f64>,
    target: FcpxmlTarget,
) -> String {
    let mut out = String::new();
    if !is_audio {
        let t = &clip.transform;
        // Resolve wants scale/position relative to the aspect-fit source; FCP wants them raw.
        let fit = if target == FcpxmlTarget::Resolve {
            fit_fractions(clip, manifest, seq_w, seq_h)
        } else {
            (1.0, 1.0)
        };
        let base = scale_value(clip, t.width, t.height, fit);
        let moved = (t.center_x - 0.5).abs() > 0.0005 || (t.center_y - 0.5).abs() > 0.0005;
        let rotated = t.rotation.abs() > 0.005;
        let scaled = base != "1 1";
        let has_scale_kf = clip.scale_track.as_ref().is_some_and(|k| !k.keyframes.is_empty());
        let has_pos_kf = clip.position_track.as_ref().is_some_and(|k| !k.keyframes.is_empty());
        let has_rot_kf = clip.rotation_track.as_ref().is_some_and(|k| !k.keyframes.is_empty());
        let has_transform_kf = has_scale_kf || has_pos_kf || has_rot_kf;
        let transform_needed = moved || rotated || scaled || has_transform_kf;
        let crop_needed = !clip.crop.is_identity();

        if crop_needed {
            let c = &clip.crop;
            // Resolve encodes the trim-rect against source pixels fit into the sequence; FCP uses
            // plain 0..100 percentages of the frame.
            let (mut lr, mut tb) = (100.0, 100.0);
            if target == FcpxmlTarget::Resolve {
                if let Some((sw, sh)) = manifest
                    .entry_for(&clip.media_ref)
                    .and_then(|e| e.source_width.zip(e.source_height))
                {
                    if sw > 0 && sh > 0 {
                        let f = (seq_w as f64 / sw as f64).min(seq_h as f64 / sh as f64);
                        lr = sw as f64 * 100.0 / seq_h as f64;
                        tb = 100.0 / f;
                    }
                }
            }
            let _ = writeln!(out, "                <adjust-crop mode=\"trim\">");
            let _ = writeln!(
                out,
                "                  <trim-rect top=\"{}\" right=\"{}\" bottom=\"{}\" left=\"{}\"/>",
                format_number(c.top * tb),
                format_number(c.right * lr),
                format_number(c.bottom * tb),
                format_number(c.left * lr)
            );
            let _ = writeln!(out, "                </adjust-crop>");
        }
        // Swift emits <adjust-conform type="fit"> for EVERY visual clip, so a source whose
        // resolution/aspect differs from the timeline is fit into the frame (not shown at native
        // size). For a matching source it's a no-op. Must accompany any transform, since scale/
        // position are computed relative to the fit.
        out.push_str("                <adjust-conform type=\"fit\"/>\n");
        if transform_needed {
            let pos_base = position_value(t, seq_w, seq_h, fit);
            let rot_base = format_number(-t.rotation);
            let mut attrs = format!(" scale=\"{base}\"");
            if rotated || has_rot_kf {
                // FCP rotation is the negation of the model's clockwise rotation.
                attrs.push_str(&format!(" rotation=\"{rot_base}\""));
            }
            attrs.push_str(&format!(" anchor=\"0 0\" position=\"{pos_base}\""));
            if !has_transform_kf {
                let _ = writeln!(out, "                <adjust-transform{attrs}/>");
            } else {
                // Keyframed transform: each animated property is a <param>; values are sampled
                // through resolved_transform_at (which applies the top-left→centre + size
                // coupling) and encoded in FCP units. Keyframe time is on the (retiming-aware)
                // output axis via kf_rows.
                let _ = writeln!(out, "                <adjust-transform{attrs}>");
                if has_scale_kf {
                    let rows = kf_rows(
                        &clip.scale_track.as_ref().unwrap().keyframes,
                        clip,
                        fps,
                        |k| {
                            let rt = timeline_core::resolved_transform_at(clip, k.frame);
                            scale_value(clip, rt.width, rt.height, fit)
                        },
                    );
                    write_kf_param(&mut out, "scale", &base, &rows);
                }
                if has_pos_kf {
                    let rows = kf_rows(
                        &clip.position_track.as_ref().unwrap().keyframes,
                        clip,
                        fps,
                        |k| {
                            let rt = timeline_core::resolved_transform_at(clip, k.frame);
                            position_value(&rt, seq_w, seq_h, fit)
                        },
                    );
                    write_kf_param(&mut out, "position", &pos_base, &rows);
                }
                if has_rot_kf {
                    let rows = kf_rows(
                        &clip.rotation_track.as_ref().unwrap().keyframes,
                        clip,
                        fps,
                        |k| {
                            let rt = timeline_core::resolved_transform_at(clip, k.frame);
                            format_number(-rt.rotation)
                        },
                    );
                    write_kf_param(&mut out, "rotation", &rot_base, &rows);
                }
                let _ = writeln!(out, "                </adjust-transform>");
            }
        }

        append_opacity_blend(&mut out, clip, fps);
    }
    let asset_has_audio = manifest
        .entry_for(&clip.media_ref)
        .map(|e| e.r#type == ClipType::Audio || e.has_audio.unwrap_or(false))
        .unwrap_or(is_audio);
    // For a collapsed synced pair, the gain lives on the dropped audio partner, not the surviving
    // video clip (whose own volume is usually a default 1.0).
    let volume = collapsed_audio_volume.unwrap_or(clip.volume);
    if asset_has_audio && (volume - 1.0).abs() > 0.0005 {
        let db = if volume > 0.0 {
            20.0 * volume.log10()
        } else {
            -96.0
        };
        let _ = writeln!(
            out,
            "                <adjust-volume amount=\"{}\"/>",
            format_number(db)
        );
    }
    out
}

/// How the source fits (aspect-preserving) into the sequence frame, as (w, h) fractions of the
/// frame the fitted source occupies. `(1, 1)` when source dimensions are unknown. Mirrors Swift
/// `fitFractions`.
fn fit_fractions(clip: &Clip, manifest: &MediaManifest, seq_w: i64, seq_h: i64) -> (f64, f64) {
    match manifest
        .entry_for(&clip.media_ref)
        .and_then(|e| e.source_width.zip(e.source_height))
    {
        Some((sw, sh)) if sw > 0 && sh > 0 => {
            let source_aspect = sw as f64 / sh as f64;
            let frame_aspect = seq_w as f64 / seq_h as f64;
            if source_aspect >= frame_aspect {
                (1.0, frame_aspect / source_aspect)
            } else {
                (source_aspect / frame_aspect, 1.0)
            }
        }
        _ => (1.0, 1.0),
    }
}

/// `<adjust-transform scale>` value: the clip's normalized size divided by the fit fraction
/// (so a fit-letterboxed source scales back to its intended on-canvas size), sign-flipped per
/// axis for mirror. Mirrors Swift `scaleValue`.
fn scale_value(clip: &Clip, w: f64, h: f64, fit: (f64, f64)) -> String {
    let mut sx = w / fit.0;
    let mut sy = h / fit.1;
    if clip.transform.flip_horizontal {
        sx = -sx;
    }
    if clip.transform.flip_vertical {
        sy = -sy;
    }
    format!("{} {}", format_number(sx), format_number(sy))
}

/// `<adjust-transform position>` value in FCP points (1/100 of frame height per unit), measured
/// from centre, y-down negated to FCP's y-up, and fit-compensated. Mirrors Swift `positionValue`.
fn position_value(t: &core_model::Transform, seq_w: i64, seq_h: i64, fit: (f64, f64)) -> String {
    let unit = seq_h as f64 / 100.0;
    let x = (t.center_x - 0.5) * seq_w as f64 / unit / fit.0;
    let y = (0.5 - t.center_y) * seq_h as f64 / unit / fit.1;
    format!("{} {}", format_number(x), format_number(y))
}

/// FCPXML number formatting (mirrors Swift `formatNumber`): round to 4 places, drop a
/// trailing `.0`, and strip trailing zeros from a fractional value.
fn format_number(value: f64) -> String {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    if rounded == rounded.round() {
        return format!("{}", rounded as i64);
    }
    let mut s = format!("{rounded:.4}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Best rational approximation `p/q` of a playback speed (`q ≤ 1000`), mirroring Swift
/// `rationalSpeed`. FCPXML expresses retiming as a rational time scale.
fn rational_speed(speed: f64) -> (i64, i64) {
    let mut best = (1i64, 1i64);
    let mut best_err = f64::INFINITY;
    for q in 1..=1000i64 {
        let p = (speed * q as f64).round() as i64;
        if p <= 0 {
            continue;
        }
        let err = (speed - p as f64 / q as f64).abs();
        if err < best_err {
            best = (p, q);
            best_err = err;
            if err == 0.0 {
                break;
            }
        }
    }
    best
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.max(1)
}

/// Rational project time `num/den s`, reduced by gcd; `0s` for zero. Mirrors Swift `rationalTime`.
fn rational_time(num: i64, den: i64) -> String {
    if num == 0 {
        return "0s".to_string();
    }
    let g = gcd(num, den);
    let n = num / g;
    let d = den / g;
    if d == 1 {
        format!("{n}s")
    } else {
        format!("{n}/{d}s")
    }
}

/// A `<timeMap>` retiming a clip whose `speed != 1` (upstream #197): two `<timept>`s map the
/// clip's output span to `[origin, origin+mediaFrames)` of the source at the retimed rate.
/// Empty when the clip runs at 1× or the source length is unknown. Mirrors Swift `timeMapNode`.
fn build_time_map(speed: f64, origin: i64, media_frames: i64, fps: i64) -> String {
    if (speed - 1.0).abs() <= 0.001 || media_frames <= 0 {
        return String::new();
    }
    let (p, q) = rational_speed(speed);
    let mut s = String::new();
    let _ = writeln!(s, "                <timeMap frameSampling=\"floor\">");
    let _ = writeln!(
        s,
        "                  <timept time=\"0s\" value=\"{}\" interp=\"linear\"/>",
        time_str(origin, fps)
    );
    let _ = writeln!(
        s,
        "                  <timept time=\"{}\" value=\"{}\" interp=\"linear\"/>",
        rational_time(media_frames * q, fps * p),
        time_str(origin + media_frames, fps)
    );
    let _ = writeln!(s, "                </timeMap>");
    s
}

/// A keyframe's `<keyframe time>` on the clip's OUTPUT axis. At 1× it's the clip-relative frame;
/// under retiming it's `(trimStart*q + frame*p)/(fps*p)`. Mirrors Swift `keyframeTime`.
fn keyframe_time_str(frame: i64, clip: &Clip, fps: i64) -> String {
    if (clip.speed - 1.0).abs() <= 0.001 {
        time_str(frame, fps)
    } else {
        let (p, q) = rational_speed(clip.speed);
        rational_time(clip.trim_start_frame.max(0) * q + frame * p, fps * p)
    }
}

/// Append an `<adjust-blend>` for opacity — self-closing for static, or with a keyframed
/// `<param name="amount">` when the clip has an opacity track. Emits nothing at full opacity with
/// no keyframes. Shared by asset-clips and titles.
fn append_opacity_blend(out: &mut String, clip: &Clip, fps: i64) {
    let opacity_kf = clip
        .opacity_track
        .as_ref()
        .map(|t| t.keyframes.as_slice())
        .filter(|k| !k.is_empty());
    if clip.opacity >= 0.9995 && opacity_kf.is_none() {
        return;
    }
    let amount = format_number(clip.opacity);
    match opacity_kf {
        Some(kfs) => {
            let _ = writeln!(out, "                <adjust-blend amount=\"{amount}\">");
            let rows = kf_rows(kfs, clip, fps, |k| format_number(k.value));
            write_kf_param(out, "amount", &amount, &rows);
            let _ = writeln!(out, "                </adjust-blend>");
        }
        None => {
            let _ = writeln!(out, "                <adjust-blend amount=\"{amount}\"/>");
        }
    }
}

/// Emit a keyframed `<param>`: the base value on the param, then a `<keyframeAnimation>` with one
/// `<keyframe>` per row (`curve="linear"` for linear segments, else FCP's default smoothing).
/// Rows are `(time_string, interpolation_out, formatted_value)`, pre-sorted by the caller — the
/// `time` is precomputed so it can be on the retimed output axis (#197).
fn write_kf_param(
    out: &mut String,
    name: &str,
    base: &str,
    rows: &[(String, core_model::Interpolation, String)],
) {
    let _ = writeln!(out, "                  <param name=\"{name}\" value=\"{base}\">");
    let _ = writeln!(out, "                    <keyframeAnimation>");
    for (time, interp, value) in rows {
        let curve = if *interp == core_model::Interpolation::Linear {
            " curve=\"linear\""
        } else {
            ""
        };
        let _ = writeln!(
            out,
            "                      <keyframe time=\"{time}\"{curve} value=\"{value}\"/>"
        );
    }
    let _ = writeln!(out, "                    </keyframeAnimation>");
    let _ = writeln!(out, "                  </param>");
}

/// Build sorted keyframe rows `(time, interp, value)` from a track's keyframes, with the time on
/// the clip's output axis (retiming-aware) and the value produced by `value_of`.
fn kf_rows<V>(
    keyframes: &[core_model::Keyframe<V>],
    clip: &Clip,
    fps: i64,
    value_of: impl Fn(&core_model::Keyframe<V>) -> String,
) -> Vec<(String, core_model::Interpolation, String)> {
    let mut rows: Vec<(i64, String, core_model::Interpolation, String)> = keyframes
        .iter()
        .map(|k| {
            (
                k.frame,
                keyframe_time_str(k.frame, clip, fps),
                k.interpolation_out,
                value_of(k),
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    rows.into_iter().map(|(_, t, i, v)| (t, i, v)).collect()
}

/// Emit a text overlay as a `<title>` generator (#254). The title references the shared
/// `titleBasic` effect and carries a `<text>`/`<text-style-def>` pair (font family/face/size/
/// colour/alignment), a fit-conform + position transform, and static opacity. Font family is
/// the name's family part and face derives from weight (Rust has no NSFont resolution, so the
/// exact system face match Swift does is approximated); border stroke is not yet emitted.
#[allow(clippy::too_many_arguments)]
fn write_title(
    xml: &mut String,
    clip: &Clip,
    content: &str,
    lane: i64,
    fps: i64,
    seq_w: i64,
    seq_h: i64,
    disabled: bool,
    style_counter: &mut usize,
) {
    let style_id = format!("ts{}", *style_counter);
    *style_counter += 1;
    let style = clip.text_style.clone().unwrap_or_default();
    let enabled_attr = if disabled { " enabled=\"0\"" } else { "" };
    let family = font_family_fallback(&style.font_name);
    // Mirrors Swift `fontFaceFallback(isBold:isItalic:)`.
    let face = match (style.font_weight >= 700.0, style.is_italic) {
        (true, true) => "Bold Italic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        (false, false) => "Regular",
    };
    let font_size = style.font_size * style.font_scale;
    let color = color_string(&style.color);
    let align = match style.alignment {
        core_model::TextAlignment::Left => "left",
        core_model::TextAlignment::Center => "center",
        core_model::TextAlignment::Right => "right",
    };
    // Border → glyph stroke. Swift's glyphBorderStrokeWidth is -4 (a percent-of-font-size
    // convention), so strokeWidth = |−4|/100 * fontSize = 0.04 * fontSize.
    let stroke = if style.border.enabled {
        format!(
            " strokeColor=\"{}\" strokeWidth=\"{}\"",
            color_string(&style.border.color),
            format_number(0.04 * font_size)
        )
    } else {
        String::new()
    };
    let _ = writeln!(
        xml,
        "              <title ref=\"titleBasic\" name=\"{}\" lane=\"{lane}\" offset=\"{}\" start=\"0s\" duration=\"{}\"{enabled_attr}>",
        xml_escape(content),
        time_str(clip.start_frame, fps),
        time_str(clip.duration_frames.max(1), fps)
    );
    let _ = writeln!(xml, "                <text>");
    let _ = writeln!(
        xml,
        "                  <text-style ref=\"{style_id}\">{}</text-style>",
        xml_escape(content)
    );
    let _ = writeln!(xml, "                </text>");
    let _ = writeln!(xml, "                <text-style-def id=\"{style_id}\">");
    let _ = writeln!(
        xml,
        "                  <text-style font=\"{}\" fontFace=\"{face}\" fontSize=\"{}\" fontColor=\"{color}\" alignment=\"{align}\"{stroke}/>",
        xml_escape(&family),
        format_number(font_size)
    );
    let _ = writeln!(xml, "                </text-style-def>");
    let _ = writeln!(xml, "                <adjust-conform type=\"fit\"/>");
    let _ = writeln!(
        xml,
        "                <adjust-transform scale=\"1 1\" anchor=\"0 0\" position=\"{}\"/>",
        position_value(&clip.transform, seq_w, seq_h, (1.0, 1.0))
    );
    append_opacity_blend(xml, clip, fps);
    let _ = writeln!(xml, "              </title>");
}

/// The font family part of a font name (`"Poppins-Bold"` → `"Poppins"`). Mirrors Swift
/// `fontFamilyFallback`.
fn font_family_fallback(font_name: &str) -> String {
    font_name.split('-').next().unwrap_or(font_name).to_string()
}

/// FCP `fontColor` string: space-separated normalized r g b a. Mirrors Swift `colorString`.
fn color_string(c: &core_model::TextRgba) -> String {
    format!(
        "{} {} {} {}",
        format_number(c.r),
        format_number(c.g),
        format_number(c.b),
        format_number(c.a)
    )
}

fn media_src(source: &MediaSource) -> String {
    match source {
        MediaSource::External { absolute_path } => {
            let p = absolute_path.replace('\\', "/");
            if p.starts_with('/') {
                format!("file://{p}")
            } else {
                format!("file:///{p}")
            }
        }
        MediaSource::Project { relative_path } => relative_path.clone(),
    }
}

fn display_name(manifest: &MediaManifest, media_ref: &str) -> String {
    manifest
        .entry_for(media_ref)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| media_ref.to_string())
}

/// The on-disk filename (with extension) used for the `name` attribute (upstream #247).
/// Resolve relinks by matching a clip/asset `name` to the file on disk, so the extension must
/// be present — the source path's last component guarantees it. Falls back to the display name
/// (then media_ref) when the source path has no usable component.
fn file_name(manifest: &MediaManifest, media_ref: &str) -> String {
    let from_source = manifest.entry_for(media_ref).and_then(|e| {
        let path = match &e.source {
            MediaSource::External { absolute_path } => absolute_path.as_str(),
            MediaSource::Project { relative_path } => relative_path.as_str(),
        };
        let name = path.replace('\\', "/");
        name.rsplit('/').next().filter(|s| !s.is_empty()).map(String::from)
    });
    from_source.unwrap_or_else(|| display_name(manifest, media_ref))
}

/// Final Cut rate suffix for a format name (upstream #214). Integer rates → the
/// rounded value (`30`); NTSC rates → the hundredths of the NTSC rate (`2997`).
fn format_rate_suffix(fps: f64) -> String {
    let rounded = fps.round();
    let ntsc_rate = rounded * 1000.0 / 1001.0;
    if (fps - ntsc_rate).abs() < (fps - rounded).abs() {
        format!("{}", (ntsc_rate * 100.0).round() as i64)
    } else {
        format!("{}", rounded as i64)
    }
}

/// Final Cut's recognized preset name for standard resolutions, else `None`.
fn recognized_video_format_name(width: i64, height: i64, fps: f64) -> Option<String> {
    let rate = format_rate_suffix(fps);
    match (width, height) {
        (1280, 720) => Some(format!("FFVideoFormat720p{rate}")),
        (1920, 1080) => Some(format!("FFVideoFormat1080p{rate}")),
        (3840, 2160) => Some(format!("FFVideoFormat3840x2160p{rate}")),
        (4096, 2160) => Some(format!("FFVideoFormat4096x2160p{rate}")),
        _ => None,
    }
}

/// Per-asset `<format>` name (upstream #214): a recognized preset when the
/// source matches one, else Final Cut's generic `FFVideoFormat{w}x{h}p{rate}`.
fn video_format_name(width: i64, height: i64, fps: f64) -> String {
    recognized_video_format_name(width, height, fps)
        .unwrap_or_else(|| format!("FFVideoFormat{width}x{height}p{}", format_rate_suffix(fps)))
}

/// Sequence `<format>` name (upstream #214): a recognized preset when the canvas
/// matches one, else Final Cut's generic `FFVideoFormatRateUndefined`.
fn sequence_format_name(width: i64, height: i64, fps: f64) -> String {
    recognized_video_format_name(width, height, fps)
        .unwrap_or_else(|| "FFVideoFormatRateUndefined".to_string())
}

/// `frameDuration` for a rate, NTSC-aware (upstream #214). e.g. `1/30s`, or
/// `1001/30000s` for 29.97.
fn frame_duration_str(fps: f64) -> String {
    let rounded = fps.round() as i64;
    let ntsc_rate = rounded as f64 * 1000.0 / 1001.0;
    if (fps - ntsc_rate).abs() < (fps - rounded as f64).abs() {
        format!("1001/{}s", rounded * 1000)
    } else {
        format!("1/{rounded}s")
    }
}

/// Rational project time, e.g. `600/30s`. Zero collapses to `0s`.
fn time_str(frames: i64, fps: i64) -> String {
    if frames <= 0 {
        "0s".to_string()
    } else {
        format!("{frames}/{fps}s")
    }
}

fn timeline_total_frames(timeline: &Timeline) -> i64 {
    timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .map(|c| c.start_frame + c.duration_frames)
        .max()
        .unwrap_or(0)
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{
        Clip, Crop, Interpolation, MediaManifestEntry, MediaSource, Timeline, Track, Transform,
    };

    fn clip(id: &str, media_ref: &str, kind: ClipType, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.to_string(),
            media_ref: media_ref.to_string(),
            media_type: kind.clone(),
            source_clip_type: kind,
            start_frame: start,
            duration_frames: dur,
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
            text_animation: None,
            word_timings: None,
        }
    }

    fn entry(id: &str, name: &str, kind: ClipType, duration: f64, path: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: name.to_string(),
            r#type: kind,
            source: MediaSource::External {
                absolute_path: path.to_string(),
            },
            duration,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    fn track(kind: ClipType, clips: Vec<Clip>) -> Track {
        Track {
            id: format!("t-{kind:?}"),
            r#type: kind,
            muted: false,
            hidden: false,
            sync_locked: false,
            display_height: 50.0,
            clips,
        }
    }

    fn timeline(tracks: Vec<Track>) -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks,
            transcription_language: None,
            folder_id: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    fn sample() -> (Timeline, MediaManifest) {
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        manifest
            .entries
            .push(entry("a1", "music.wav", ClipType::Audio, 20.0, "/media/music.wav"));
        let tl = timeline(vec![
            track(ClipType::Video, vec![clip("c1", "v1", ClipType::Video, 0, 60)]),
            track(ClipType::Audio, vec![clip("c2", "a1", ClipType::Audio, 0, 120)]),
        ]);
        (tl, manifest)
    }

    #[test]
    fn fcpxml_export_flattens_compound_clip_to_nested_asset() {
        // A nest carrier (Swift #255) must export its child's assets, not an
        // empty ref. v1 flattens; native <ref-clip> emission is a follow-up.
        let inner = clip("inner", "v1", ClipType::Video, 0, 30);
        let mut nested = timeline(vec![track(ClipType::Video, vec![inner])]);
        nested.id = "n1".into();
        let mut carrier = clip("carrier", "n1", ClipType::Sequence, 0, 30);
        carrier.source_clip_type = ClipType::Sequence;
        let tl = timeline(vec![track(ClipType::Video, vec![carrier])]);
        let timelines = std::collections::HashMap::from([("n1".to_string(), nested)]);

        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));

        let xml = FcpxmlExport::export_with_target_and_timelines(
            &tl,
            &manifest,
            FcpxmlTarget::Resolve,
            &timelines,
        );
        assert!(xml.contains("shot.mp4"), "nested asset exported: {xml}");
    }

    #[test]
    fn fcpxml_header_and_version() {
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        assert!(xml.starts_with("<?xml version=\"1.0\""), "xml prolog");
        assert!(xml.contains("<!DOCTYPE fcpxml>"));
        assert!(xml.contains("<fcpxml version=\"1.10\">"));
        assert!(xml.trim_end().ends_with("</fcpxml>"));
    }

    #[test]
    fn fcpxml_no_timecode_asset_start_is_zero() {
        // #247: an asset without an embedded timecode keeps start="0s" and the in-point is
        // just the clip's trim (no origin offset).
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        assert!(xml.contains("<asset id=\"r3\" name=\"shot.mp4\" start=\"0s\""), "{xml}");
    }

    #[test]
    fn fcpxml_source_timecode_offsets_asset_and_clip_in_point() {
        // #247: a source with an embedded start timecode emits it as the asset `start`, and the
        // asset-clip in-point reads from that origin (origin + trim). Camera TC 90 @ quanta 30,
        // project fps 30 → origin 90 frames; trim 15 → in-point 105.
        let mut e = entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4");
        e.source_timecode_frame = Some(90);
        e.source_timecode_quanta = Some(30);
        let mut manifest = MediaManifest::default();
        manifest.entries.push(e);
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.trim_start_frame = 15;
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        // Asset carries its embedded timecode origin.
        assert!(
            xml.contains("<asset id=\"r3\" name=\"shot.mp4\" start=\"90/30s\""),
            "asset start = embedded timecode\n{xml}"
        );
        // Asset-clip in-point = origin (90) + trim (15) = 105.
        assert!(
            xml.contains("start=\"105/30s\""),
            "asset-clip in-point offset by origin\n{xml}"
        );
    }

    #[test]
    fn fcpxml_static_opacity_emits_adjust_blend() {
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.opacity = 0.5;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(
            xml.contains("<adjust-blend amount=\"0.5\"/>"),
            "opacity → adjust-blend\n{xml}"
        );
        assert!(xml.contains("</asset-clip>"), "open/close form when adjusted");
    }

    #[test]
    fn fcpxml_static_volume_emits_adjust_volume_db() {
        // Video asset carries audio (entry has_audio=true); clip volume 0.5 → dB.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.volume = 0.5;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(
            xml.contains("<adjust-volume amount=\"-6.0206\"/>"),
            "volume 0.5 → -6.0206 dB\n{xml}"
        );
    }

    #[test]
    fn fcpxml_default_visual_clip_has_only_conform() {
        // A default visual clip carries just <adjust-conform type="fit"> (Swift emits it for every
        // visual clip) — no blend/volume/transform/crop. Audio stays self-closing.
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        assert!(xml.contains("<adjust-conform type=\"fit\"/>"), "conform for visual\n{xml}");
        assert!(!xml.contains("<adjust-blend"), "no opacity for default");
        assert!(!xml.contains("<adjust-transform"), "no transform for default same-res");
        assert!(!xml.contains("<adjust-crop"), "no crop for default");
        // Audio clip (r4) has no conform and stays self-closing.
        let audio_line = xml
            .lines()
            .find(|l| l.contains("<asset-clip") && l.contains("ref=\"r4\""))
            .expect("audio asset-clip");
        assert!(audio_line.trim_end().ends_with("/>"), "audio self-closing: {audio_line}");
    }

    #[test]
    fn fcpxml_format_number_strips_trailing() {
        assert_eq!(format_number(0.5), "0.5");
        assert_eq!(format_number(1.0), "1");
        assert_eq!(format_number(-6.020599913), "-6.0206");
        assert_eq!(format_number(0.80000), "0.8");
    }

    #[test]
    fn fcpxml_rich_timeline_integration() {
        // Exercise many features at once and check the document is well-formed and carries each:
        // a transformed+cropped+opacity-keyframed video clip, a retimed clip, a volume audio clip,
        // a text title, and a hidden track — all in one export.
        let mut vid = clip("v-clip", "v1", ClipType::Video, 0, 60);
        vid.transform.center_x = 0.25;
        vid.transform.width = 0.5;
        vid.transform.height = 0.5;
        vid.crop.top = 0.1;
        vid.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe { frame: 0, value: 0.0, interpolation_out: core_model::Interpolation::Linear },
                core_model::Keyframe { frame: 30, value: 1.0, interpolation_out: core_model::Interpolation::Linear },
            ],
        });
        let mut slow = clip("slow-clip", "v2", ClipType::Video, 60, 30);
        slow.speed = 2.0;
        let mut aud = clip("a-clip", "a1", ClipType::Audio, 0, 120);
        aud.volume = 0.5;
        let mut txt = clip("t-clip", "", ClipType::Text, 0, 60);
        txt.media_type = ClipType::Text;
        txt.text_content = Some("Title".to_string());

        let mut manifest = MediaManifest::default();
        manifest.entries.push(entry("v1", "a.mp4", ClipType::Video, 10.0, "/m/a.mp4"));
        manifest.entries.push(entry("v2", "b.mp4", ClipType::Video, 10.0, "/m/b.mp4"));
        manifest.entries.push(entry("a1", "c.wav", ClipType::Audio, 10.0, "/m/c.wav"));
        let mut hidden = track(ClipType::Video, vec![clip("h-clip", "v1", ClipType::Video, 0, 30)]);
        hidden.hidden = true;
        let tl = timeline(vec![
            track(ClipType::Video, vec![vid, slow, txt]),
            track(ClipType::Audio, vec![aud]),
            hidden,
        ]);
        let xml = FcpxmlExport::export(&tl, &manifest);

        // Well-formed envelope.
        assert!(xml.starts_with("<?xml version=\"1.0\""));
        assert!(xml.trim_end().ends_with("</fcpxml>"));
        // Every always-paired container tag is balanced (these never self-close). Tags that CAN
        // self-close (asset-clip, adjust-transform, adjust-blend) are checked for presence below.
        for tag in ["fcpxml", "resources", "library", "event", "project", "sequence", "spine", "gap", "title", "timeMap", "adjust-crop", "keyframeAnimation"] {
            let open = xml.matches(&format!("<{tag} ")).count() + xml.matches(&format!("<{tag}>")).count();
            let close = xml.matches(&format!("</{tag}>")).count();
            assert_eq!(open, close, "tag <{tag}> is unbalanced: {open} open vs {close} close");
        }
        // Each feature is present.
        assert!(xml.contains("<effect id=\"titleBasic\""), "title effect resource");
        assert!(xml.contains("<title "), "title node");
        assert!(xml.contains("<timeMap "), "retimed clip timeMap");
        assert!(xml.contains("<adjust-crop "), "crop");
        assert!(xml.contains("<adjust-transform "), "transform");
        assert!(xml.contains("<param name=\"amount\""), "keyframed opacity");
        assert!(xml.contains("<adjust-volume "), "audio volume");
        assert!(xml.contains("enabled=\"0\""), "hidden track disabled");
        assert!(xml.contains("<adjust-conform type=\"fit\"/>"), "conform");
    }

    #[test]
    fn fcpxml_fcp_target_uses_raw_scale_and_crop() {
        // Square source (1080x1080) with a half-size transform + crop, in a 1920x1080 timeline.
        // Resolve fits against the source aspect; FCP uses raw frame-relative values.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.transform.width = 0.5;
        c.transform.height = 0.5;
        c.crop.top = 0.2;
        let mut manifest = MediaManifest::default();
        let mut e = entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4");
        e.source_width = Some(1080);
        e.source_height = Some(1080);
        manifest.entries.push(e);
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let resolve = FcpxmlExport::export(&tl, &manifest);
        let fcp = FcpxmlExport::export_with_target(&tl, &manifest, FcpxmlTarget::Fcp);
        // Resolve fit=(0.5625,1) → scale (0.5/0.5625, 0.5) = 0.8889 0.5; FCP fit=(1,1) → 0.5 0.5.
        assert!(resolve.contains("scale=\"0.8889 0.5\""), "resolve scale\n{resolve}");
        assert!(fcp.contains("scale=\"0.5 0.5\""), "fcp scale\n{fcp}");
        // Resolve crop tb = 100/min(1920/1080,1080/1080)=100 → top 20; FCP also 100 here (square).
        // Use a 4K source to separate them:
        let mut c2 = clip("c2", "v2", ClipType::Video, 0, 60);
        c2.crop.top = 0.2;
        let mut m2 = MediaManifest::default();
        let mut e2 = entry("v2", "uhd.mp4", ClipType::Video, 10.0, "/media/uhd.mp4");
        e2.source_width = Some(3840);
        e2.source_height = Some(2160);
        m2.entries.push(e2);
        let tl2 = timeline(vec![track(ClipType::Video, vec![c2])]);
        // Resolve tb = 100/min(1920/3840,1080/2160) = 100/0.5 = 200 → top 40; FCP → 20.
        assert!(FcpxmlExport::export(&tl2, &m2).contains("top=\"40\""), "resolve crop");
        assert!(
            FcpxmlExport::export_with_target(&tl2, &m2, FcpxmlTarget::Fcp).contains("top=\"20\""),
            "fcp crop"
        );
    }

    #[test]
    fn fcpxml_rational_speed_and_time() {
        assert_eq!(rational_speed(2.0), (2, 1));
        assert_eq!(rational_speed(1.5), (3, 2));
        assert_eq!(rational_speed(0.5), (1, 2));
        assert_eq!(rational_time(300, 60), "5s");
        assert_eq!(rational_time(15, 60), "1/4s");
        assert_eq!(rational_time(0, 60), "0s");
    }

    #[test]
    fn fcpxml_retimed_clip_emits_timemap() {
        // 2x speed, source 10s (300f @ 30fps). Output span = 300/(30*2) = 5s; the timeMap maps
        // it to the full source [0, 300/30s).
        let mut c = clip("c1", "v1", ClipType::Video, 0, 30);
        c.speed = 2.0;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<timeMap frameSampling=\"floor\">"), "timeMap\n{xml}");
        assert!(
            xml.contains("<timept time=\"0s\" value=\"0s\" interp=\"linear\"/>"),
            "timept 0\n{xml}"
        );
        assert!(
            xml.contains("<timept time=\"5s\" value=\"300/30s\" interp=\"linear\"/>"),
            "timept 1\n{xml}"
        );
    }

    #[test]
    fn fcpxml_1x_clip_has_no_timemap() {
        let (tl, m) = sample();
        assert!(!FcpxmlExport::export(&tl, &m).contains("<timeMap"), "1x → no timeMap");
    }

    #[test]
    fn fcpxml_retimed_clip_keyframe_time_on_output_axis() {
        // speed 2x, opacity kf at clip-relative frame 30 → keyframeTime = (0 + 30*2)/(30*2) = 1s
        // (rational-reduced), i.e. the output axis, not the raw 1x frame form.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 30);
        c.speed = 2.0;
        c.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: 1.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 30,
                    value: 0.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(
            xml.contains("<keyframe time=\"1s\" curve=\"linear\" value=\"0\"/>"),
            "retimed keyframe time on output axis\n{xml}"
        );
    }

    #[test]
    fn fcpxml_retimed_clip_start_on_retimed_axis() {
        // trim 15 at 2x → start = 15*q/(fps*p) = 15*1/(30*2) = 15/60 = 1/4s.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 30);
        c.speed = 2.0;
        c.trim_start_frame = 15;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("start=\"1/4s\""), "retimed in-point\n{xml}");
    }

    #[test]
    fn fcpxml_pip_transform_emits_conform_and_transform() {
        // Quarter-size PIP in the top-left quadrant: center (0.25,0.25), size 0.5x0.5.
        // Matching aspect (1920x1080 in 1920x1080) → fit (1,1). scale "0.5 0.5".
        // position x=(0.25-0.5)*1920/10.8 = -44.4444; y=(0.5-0.25)*1080/10.8 = 25.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.transform.center_x = 0.25;
        c.transform.center_y = 0.25;
        c.transform.width = 0.5;
        c.transform.height = 0.5;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<adjust-conform type=\"fit\"/>"), "conform\n{xml}");
        assert!(
            xml.contains("<adjust-transform scale=\"0.5 0.5\" anchor=\"0 0\" position=\"-44.4444 25\"/>"),
            "PIP transform\n{xml}"
        );
    }

    #[test]
    fn fcpxml_rotation_negated_for_fcp() {
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.transform.rotation = 90.0;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("rotation=\"-90\""), "FCP rotation negated\n{xml}");
    }

    #[test]
    fn fcpxml_crop_emits_trim_rect() {
        // crop top 0.1; source 1920x1080 in seq 1920x1080 → fit 1, lr=177.7778, tb=100.
        // trim-rect top = 0.1*100 = 10, others 0.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.crop.top = 0.1;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<adjust-crop mode=\"trim\">"), "crop\n{xml}");
        assert!(
            xml.contains("<trim-rect top=\"10\" right=\"0\" bottom=\"0\" left=\"0\"/>"),
            "trim-rect\n{xml}"
        );
        assert!(xml.contains("<adjust-conform type=\"fit\"/>"), "crop also emits conform");
    }

    #[test]
    fn fcpxml_flip_negates_scale_axis() {
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.transform.width = 0.5;
        c.transform.height = 0.5;
        c.transform.flip_horizontal = true;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("scale=\"-0.5 0.5\""), "h-flip negates sx\n{xml}");
    }

    #[test]
    fn fcpxml_keyframed_opacity_emits_keyframe_animation() {
        // Opacity ramp 1.0 → 0.0 over frames 0..30 (clip-relative). Emits a
        // <param>/<keyframeAnimation> inside adjust-blend.
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: 1.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 30,
                    value: 0.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<param name=\"amount\" value=\"1\">"), "param\n{xml}");
        assert!(xml.contains("<keyframeAnimation>"), "keyframeAnimation");
        assert!(
            xml.contains("<keyframe time=\"0s\" curve=\"linear\" value=\"1\"/>"),
            "first keyframe\n{xml}"
        );
        assert!(
            xml.contains("<keyframe time=\"30/30s\" curve=\"linear\" value=\"0\"/>"),
            "second keyframe\n{xml}"
        );
        assert!(xml.contains("</adjust-blend>"), "open/close blend");
    }

    #[test]
    fn fcpxml_keyframed_position_emits_transform_param() {
        // Half-size clip; position track animates top-left (0,0)→(0.25,0.25) over 0..30f.
        // resolved centre = top_left + size/2: frame0 (0.25,0.25) → pos "-44.4444 25";
        // frame30 (0.5,0.5) → pos "0 0". Base uses static centre (0.5,0.5) → "0 0".
        let mut c = clip("c1", "v1", ClipType::Video, 0, 60);
        c.transform.width = 0.5;
        c.transform.height = 0.5;
        c.position_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: core_model::AnimPair { a: 0.0, b: 0.0 },
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 30,
                    value: core_model::AnimPair { a: 0.25, b: 0.25 },
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(
            xml.contains("<adjust-transform scale=\"0.5 0.5\" anchor=\"0 0\" position=\"0 0\">"),
            "transform open tag\n{xml}"
        );
        assert!(xml.contains("<param name=\"position\" value=\"0 0\">"), "position param\n{xml}");
        assert!(
            xml.contains("<keyframe time=\"0s\" curve=\"linear\" value=\"-44.4444 25\"/>"),
            "frame 0 centre\n{xml}"
        );
        assert!(
            xml.contains("<keyframe time=\"30/30s\" curve=\"linear\" value=\"0 0\"/>"),
            "frame 30 centre\n{xml}"
        );
        assert!(xml.contains("</adjust-transform>"), "open/close transform");
    }

    #[test]
    fn fcpxml_text_clip_emits_title() {
        let mut c = clip("t1", "", ClipType::Text, 0, 60);
        c.media_type = ClipType::Text;
        c.source_clip_type = ClipType::Text;
        c.transform.center_x = 0.5;
        c.transform.center_y = 0.5;
        c.text_content = Some("Hello World".to_string());
        c.text_style = Some(core_model::TextStyle {
            font_name: "Poppins-Bold".to_string(),
            font_size: 48.0,
            font_weight: 700.0,
            alignment: core_model::TextAlignment::Center,
            color: core_model::TextRgba {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ..Default::default()
        });
        let manifest = MediaManifest::default();
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<effect id=\"titleBasic\""), "title effect resource\n{xml}");
        assert!(
            xml.contains("<title ref=\"titleBasic\" name=\"Hello World\""),
            "title node\n{xml}"
        );
        assert!(
            xml.contains("<text-style ref=\"ts0\">Hello World</text-style>"),
            "text content\n{xml}"
        );
        assert!(
            xml.contains("font=\"Poppins\" fontFace=\"Bold\" fontSize=\"48\" fontColor=\"1 0 0 1\" alignment=\"center\""),
            "text-style attrs\n{xml}"
        );
        assert!(xml.contains("</title>"), "title closed");
    }

    #[test]
    fn fcpxml_collapsed_pair_uses_audio_partner_volume() {
        // Linked video+audio of the same source (volume lives on the audio: 0.5). The pair
        // collapses into the video asset-clip, which must carry the audio's gain (-6.0206 dB),
        // not the video clip's own 1.0.
        let mut v = clip("v", "m1", ClipType::Video, 0, 60);
        v.link_group_id = Some("g1".into());
        let mut a = clip("a", "m1", ClipType::Audio, 0, 60);
        a.link_group_id = Some("g1".into());
        a.volume = 0.5;
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("m1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![
            track(ClipType::Video, vec![v]),
            track(ClipType::Audio, vec![a]),
        ]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(
            xml.contains("<adjust-volume amount=\"-6.0206\"/>"),
            "collapsed clip uses audio partner volume\n{xml}"
        );
        assert_eq!(
            xml.matches("<asset-clip").count(),
            1,
            "audio partner dropped\n{xml}"
        );
    }

    #[test]
    fn fcpxml_title_face_reflects_bold_and_italic() {
        // #65: fontFace mirrors Swift fontFaceFallback(isBold, isItalic).
        let cases = [
            (700.0, true, "Bold Italic"),
            (700.0, false, "Bold"),
            (400.0, true, "Italic"),
            (400.0, false, "Regular"),
        ];
        for (weight, italic, expected) in cases {
            let mut c = clip("t1", "", ClipType::Text, 0, 60);
            c.media_type = ClipType::Text;
            c.text_content = Some("T".to_string());
            c.text_style = Some(core_model::TextStyle {
                font_weight: weight,
                is_italic: italic,
                ..Default::default()
            });
            let tl = timeline(vec![track(ClipType::Video, vec![c])]);
            let xml = FcpxmlExport::export(&tl, &MediaManifest::default());
            assert!(
                xml.contains(&format!("fontFace=\"{expected}\"")),
                "weight {weight} italic {italic} → {expected}\n{xml}"
            );
        }
    }

    #[test]
    fn fcpxml_title_keyframed_opacity() {
        let mut c = clip("t1", "", ClipType::Text, 0, 60);
        c.media_type = ClipType::Text;
        c.text_content = Some("Fade".to_string());
        c.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: 0.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 15,
                    value: 1.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &MediaManifest::default());
        assert!(xml.contains("<title "), "title emitted\n{xml}");
        assert!(xml.contains("<param name=\"amount\""), "keyframed opacity param\n{xml}");
        assert!(
            xml.contains("<keyframe time=\"0s\" curve=\"linear\" value=\"0\"/>"),
            "kf 0\n{xml}"
        );
        assert!(
            xml.contains("<keyframe time=\"15/30s\" curve=\"linear\" value=\"1\"/>"),
            "kf 15\n{xml}"
        );
    }

    #[test]
    fn fcpxml_title_border_emits_stroke() {
        let mut c = clip("t1", "", ClipType::Text, 0, 60);
        c.media_type = ClipType::Text;
        c.text_content = Some("Outlined".to_string());
        c.text_style = Some(core_model::TextStyle {
            font_size: 50.0,
            border: core_model::TextFill {
                enabled: true,
                color: core_model::TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                padding: None,
                corner_radius: None,
            },
            ..Default::default()
        });
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &MediaManifest::default());
        // strokeWidth = 0.04 * 50 = 2.
        assert!(
            xml.contains("strokeColor=\"0 0 0 1\" strokeWidth=\"2\""),
            "border stroke\n{xml}"
        );
    }

    #[test]
    fn fcpxml_keyframe_time_is_clip_relative_at_nonzero_start() {
        // Regression: a keyframed clip that STARTS at frame 100 must still emit clip-relative
        // keyframe times (0s, 30/30s), not absolute (100/30s, 130/30s).
        let mut c = clip("c1", "v1", ClipType::Video, 100, 60);
        c.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: 1.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 30,
                    value: 0.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<keyframe time=\"0s\""), "clip-relative start\n{xml}");
        assert!(xml.contains("<keyframe time=\"30/30s\""), "clip-relative end\n{xml}");
        assert!(!xml.contains("time=\"100/30s\""), "not absolute");
        // The asset-clip offset IS absolute (timeline position 100).
        assert!(xml.contains("offset=\"100/30s\""), "clip offset is absolute\n{xml}");
    }

    #[test]
    fn fcpxml_hidden_track_clip_is_disabled() {
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let mut t = track(ClipType::Video, vec![clip("c1", "v1", ClipType::Video, 0, 60)]);
        t.hidden = true;
        let tl = timeline(vec![t]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("enabled=\"0\""), "hidden track → disabled\n{xml}");
        // A visible track carries no enabled attribute (FCP defaults to enabled).
        let (vis_tl, vis_m) = sample();
        assert!(
            !FcpxmlExport::export(&vis_tl, &vis_m).contains("enabled="),
            "visible tracks omit enabled"
        );
    }

    #[test]
    fn fcpxml_name_uses_on_disk_filename_for_relink() {
        // #247 relink: even when the asset's display name is a user label, the `name` attribute
        // is the on-disk filename (with extension) so Resolve can relink.
        let mut e = entry("v1", "My Interview", ClipType::Video, 10.0, "/media/C0012.MP4");
        e.name = "My Interview".to_string();
        let mut manifest = MediaManifest::default();
        manifest.entries.push(e);
        let tl = timeline(vec![track(
            ClipType::Video,
            vec![clip("c1", "v1", ClipType::Video, 0, 60)],
        )]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("<asset id=\"r3\" name=\"C0012.MP4\""), "asset name = filename\n{xml}");
        assert!(
            xml.contains("<asset-clip ref=\"r3\"") && xml.contains("name=\"C0012.MP4\""),
            "asset-clip name = filename\n{xml}"
        );
        assert!(!xml.contains("My Interview"), "display label not used for name\n{xml}");
    }

    #[test]
    fn fcpxml_source_timecode_rescales_quanta_to_project_fps() {
        // quanta differs from project fps: TC frame 48 @ quanta 24, project fps 30 →
        // round(48/24*30) = 60 frames origin.
        let mut e = entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4");
        e.source_timecode_frame = Some(48);
        e.source_timecode_quanta = Some(24);
        let mut manifest = MediaManifest::default();
        manifest.entries.push(e);
        let tl = timeline(vec![track(
            ClipType::Video,
            vec![clip("c1", "v1", ClipType::Video, 0, 60)],
        )]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert!(xml.contains("start=\"60/30s\""), "quanta rescaled to fps\n{xml}");
    }

    #[test]
    fn fcpxml_format_and_assets_present() {
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        assert!(xml.contains("<format id=\"r1\" name=\"FFVideoFormat1080p30\" frameDuration=\"1/30s\" width=\"1920\" height=\"1080\" colorSpace=\"1-1-1 (Rec. 709)\"/>"));
        // Video asset gets its own per-asset format (r2), then the asset (r3);
        // the audio asset (r4) carries no format attribute.
        assert!(xml.contains("<format id=\"r2\" name=\"FFVideoFormat1080p30\""), "per-asset video format");
        assert!(xml.contains("<asset id=\"r3\""), "video asset");
        assert!(xml.contains("<asset id=\"r4\""), "audio asset");
        assert!(xml.contains("src=\"file:///media/shot.mp4\""));
        assert!(xml.contains("hasAudio=\"1\""));
    }

    #[test]
    fn fcpxml_asset_format_uses_project_fps_grid_not_source_fps() {
        // A 24fps 4K source in a 30fps project: the per-asset <format> keeps the 4K
        // RESOLUTION but must declare the PROJECT frameDuration (1/30s) so the asset
        // <duration> and asset-clip <start> (both on the project grid) align to it.
        // A source-fps 1/24s grid would make Final Cut conform-snap those times.
        let mut e = entry("v24", "shot4k.mp4", ClipType::Video, 0.7, "/media/shot4k.mp4");
        e.source_width = Some(3840);
        e.source_height = Some(2160);
        e.source_fps = Some(24.0);
        let mut manifest = MediaManifest::default();
        manifest.entries.push(e);
        let mut c = clip("c1", "v24", ClipType::Video, 0, 21);
        c.trim_start_frame = 7;
        let tl = timeline(vec![track(ClipType::Video, vec![c])]);

        let xml = FcpxmlExport::export(&tl, &manifest);
        let fmt_line = xml
            .lines()
            .find(|l| l.contains("<format") && l.contains("width=\"3840\""))
            .expect("per-asset 4K format present");
        assert!(
            fmt_line.contains("frameDuration=\"1/30s\""),
            "asset format must use the project fps grid: {fmt_line}"
        );
        assert!(!xml.contains("1/24s"), "no source-fps grid anywhere:\n{xml}");
        assert!(xml.contains("start=\"7/30s\""), "in-point on the project grid:\n{xml}");
    }

    #[test]
    fn fcpxml_asset_clip_references_own_format_and_omits_for_audio() {
        // Video asset-clips reference their OWN per-asset format (not the sequence
        // r1); audio asset-clips omit `format` (their asset has no video format).
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        let video_line = xml
            .lines()
            .find(|l| l.contains("<asset-clip") && l.contains("ref=\"r3\""))
            .expect("video asset-clip present");
        assert!(
            video_line.contains("format=\"r2\""),
            "video clip references its own per-asset format: {video_line}"
        );
        let audio_line = xml
            .lines()
            .find(|l| l.contains("<asset-clip") && l.contains("ref=\"r4\""))
            .expect("audio asset-clip present");
        assert!(
            !audio_line.contains("format="),
            "audio clip omits the video format attribute: {audio_line}"
        );
    }

    #[test]
    fn fcpxml_collapses_synced_av_pair_dropping_the_audio_partner() {
        // A video and its linked audio from the SAME source, same timing/trim/speed,
        // collapse into the single video asset-clip; the audio partner is not emitted.
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let mut vclip = clip("cv", "v1", ClipType::Video, 0, 60);
        vclip.link_group_id = Some("g1".into());
        let mut aclip = clip("ca", "v1", ClipType::Audio, 0, 60);
        aclip.link_group_id = Some("g1".into());
        let tl = timeline(vec![
            track(ClipType::Video, vec![vclip]),
            track(ClipType::Audio, vec![aclip]),
        ]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert_eq!(
            xml.matches("<asset-clip").count(),
            1,
            "audio partner collapsed into the video asset-clip:\n{xml}"
        );
    }

    #[test]
    fn fcpxml_does_not_collapse_pair_from_different_sources() {
        // Linked, but from DIFFERENT sources → both asset-clips are emitted.
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        manifest
            .entries
            .push(entry("a1", "music.wav", ClipType::Audio, 10.0, "/media/music.wav"));
        let mut vclip = clip("cv", "v1", ClipType::Video, 0, 60);
        vclip.link_group_id = Some("g1".into());
        let mut aclip = clip("ca", "a1", ClipType::Audio, 0, 60);
        aclip.link_group_id = Some("g1".into());
        let tl = timeline(vec![
            track(ClipType::Video, vec![vclip]),
            track(ClipType::Audio, vec![aclip]),
        ]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert_eq!(
            xml.matches("<asset-clip").count(),
            2,
            "different sources are not collapsed:\n{xml}"
        );
    }

    #[test]
    fn fcpxml_does_not_collapse_when_audio_track_is_muted() {
        // A muted audio partner must NOT collapse — folding it into the video's
        // asset-clip (which carries audio) would make the muted audio audible.
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let mut vclip = clip("cv", "v1", ClipType::Video, 0, 60);
        vclip.link_group_id = Some("g1".into());
        let mut aclip = clip("ca", "v1", ClipType::Audio, 0, 60);
        aclip.link_group_id = Some("g1".into());
        let mut atrack = track(ClipType::Audio, vec![aclip]);
        atrack.muted = true;
        let tl = timeline(vec![track(ClipType::Video, vec![vclip]), atrack]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert_eq!(
            xml.matches("<asset-clip").count(),
            2,
            "muted audio (enabled diverges) is not collapsed:\n{xml}"
        );
    }

    #[test]
    fn fcpxml_asset_deduped_by_media_ref() {
        // Two clips of the same source → a single asset resource.
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let tl = timeline(vec![track(
            ClipType::Video,
            vec![
                clip("c1", "v1", ClipType::Video, 0, 30),
                clip("c2", "v1", ClipType::Video, 30, 30),
            ],
        )]);
        let xml = FcpxmlExport::export(&tl, &manifest);
        assert_eq!(xml.matches("<asset id=").count(), 1, "one deduped asset");
        assert_eq!(xml.matches("<asset-clip ").count(), 2, "two clips");
    }

    #[test]
    fn fcpxml_spine_places_clips_with_offset_and_lane() {
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        // Video clip on lane 1, audio on lane -1; both anchored to the gap.
        assert!(xml.contains("<gap name=\"Gap\" offset=\"0s\" duration=\"120/30s\">"));
        assert!(xml.contains("ref=\"r3\" lane=\"1\" offset=\"0s\" name=\"shot.mp4\" duration=\"60/30s\""));
        assert!(xml.contains("ref=\"r4\" lane=\"-1\" offset=\"0s\" name=\"music.wav\" duration=\"120/30s\""));
    }

    #[test]
    fn fcpxml_top_track_gets_the_highest_lane() {
        // Two video tracks: tracks[0] is the top layer → highest lane (2),
        // tracks[1] → lane 1. Higher positive lane renders above in FCP.
        let tl = timeline(vec![
            track(ClipType::Video, vec![clip("top", "a", ClipType::Video, 0, 30)]),
            track(ClipType::Video, vec![clip("bot", "b", ClipType::Video, 0, 30)]),
        ]);
        let mut m = MediaManifest::default();
        m.entries.push(entry("a", "top.mp4", ClipType::Video, 1.0, "/top.mp4"));
        m.entries.push(entry("b", "bot.mp4", ClipType::Video, 1.0, "/bot.mp4"));
        let xml = FcpxmlExport::export(&tl, &m);
        assert!(xml.contains("lane=\"2\" offset=\"0s\" name=\"top.mp4\""), "tracks[0] → lane 2");
        assert!(xml.contains("lane=\"1\" offset=\"0s\" name=\"bot.mp4\""), "tracks[1] → lane 1");
    }

    #[test]
    fn fcpxml_text_and_shape_clips_skipped() {
        let tl = timeline(vec![track(
            ClipType::Video,
            vec![
                clip("c1", "v1", ClipType::Video, 0, 30),
                clip("t1", "txt", ClipType::Text, 0, 30),
            ],
        )]);
        let mut m = MediaManifest::default();
        m.entries
            .push(entry("v1", "shot.mp4", ClipType::Video, 10.0, "/media/shot.mp4"));
        let xml = FcpxmlExport::export(&tl, &m);
        assert_eq!(xml.matches("<asset-clip ").count(), 1, "text clip skipped");
    }

    #[test]
    fn format_naming_recognized_and_generic() {
        assert_eq!(format_rate_suffix(30.0), "30");
        assert_eq!(format_rate_suffix(60.0), "60");
        assert_eq!(format_rate_suffix(29.97), "2997");
        assert_eq!(
            recognized_video_format_name(1920, 1080, 30.0).as_deref(),
            Some("FFVideoFormat1080p30")
        );
        assert_eq!(
            recognized_video_format_name(3840, 2160, 60.0).as_deref(),
            Some("FFVideoFormat3840x2160p60")
        );
        assert_eq!(recognized_video_format_name(1080, 1920, 30.0), None);
        // Custom (portrait) canvas falls back to Final Cut's generic preset.
        assert_eq!(sequence_format_name(1080, 1920, 30.0), "FFVideoFormatRateUndefined");
    }

    #[test]
    fn frame_duration_integer_and_ntsc() {
        assert_eq!(frame_duration_str(30.0), "1/30s");
        assert_eq!(frame_duration_str(24.0), "1/24s");
        assert_eq!(frame_duration_str(29.97), "1001/30000s");
    }

    #[test]
    fn fcpxml_audio_asset_has_no_video_format() {
        let (tl, m) = sample();
        let xml = FcpxmlExport::export(&tl, &m);
        // The audio asset (r4) references no format; only the video asset (r3) does.
        let audio_line = xml
            .lines()
            .find(|l| l.contains("<asset id=\"r4\""))
            .expect("audio asset present");
        assert!(!audio_line.contains("format="), "audio asset: {audio_line}");
        let video_line = xml
            .lines()
            .find(|l| l.contains("<asset id=\"r3\""))
            .expect("video asset present");
        assert!(video_line.contains("format=\"r2\""), "video asset: {video_line}");
    }

    #[test]
    fn fcpxml_custom_canvas_uses_generic_format_name() {
        let tl = Timeline {
            width: 1080,
            height: 1920,
            ..timeline(vec![])
        };
        let xml = FcpxmlExport::export(&tl, &MediaManifest::default());
        assert!(xml.contains(
            "<format id=\"r1\" name=\"FFVideoFormatRateUndefined\" frameDuration=\"1/30s\" width=\"1080\" height=\"1920\" colorSpace=\"1-1-1 (Rec. 709)\"/>"
        ));
    }

    #[test]
    fn fcpxml_empty_timeline_is_valid() {
        let tl = timeline(vec![]);
        let xml = FcpxmlExport::export(&tl, &MediaManifest::default());
        assert!(xml.contains("<fcpxml version=\"1.10\">"));
        assert!(xml.contains("<spine>"));
        assert!(xml.trim_end().ends_with("</fcpxml>"));
    }
}
