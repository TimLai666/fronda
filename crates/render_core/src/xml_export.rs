use core_model::{Clip, ClipType, MediaManifest, MediaSource, Timeline, Track};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// A source's start timecode: frame number in the timecode track's own quanta rate,
/// plus its drop-frame flag. Upstream PR #136.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceTimecode {
    pub frame: i64,
    pub quanta: i64,
    pub drop_frame: bool,
}

/// Frame count → SMPTE string; drop-frame (29.97/59.94) uses `;` separators.
pub fn format_timecode(frame: i64, fps: i64, drop_frame: bool) -> String {
    if fps <= 0 {
        return "00:00:00:00".to_string();
    }
    let mut f = frame;
    if drop_frame {
        let drop = ((fps as f64) * 0.066666).round() as i64; // 2 @ 30, 4 @ 60
        // Drop-adjusted divisors: a real minute holds fps*60 - drop frame labels
        // (2 dropped) and a 10-minute block fps*600 - 9*drop (9 of 10 minutes drop).
        // Using the nominal fps*60 / fps*600 here under-counts the added labels and
        // shifts the SMPTE frame field (e.g. frame 1800@30 → ;00 instead of ;02).
        let fpm = fps * 60 - drop;
        let fp10m = fps * 600 - 9 * drop;
        let d = f / fp10m;
        let m = f % fp10m;
        f += drop * 9 * d
            + if m > drop {
                drop * ((m - drop) / fpm)
            } else {
                0
            };
    }
    let sep = if drop_frame { ";" } else { ":" };
    let ff = f % fps;
    let ss = (f / fps) % 60;
    let mm = (f / (fps * 60)) % 60;
    let hh = f / (fps * 3600);
    format!("{:02}{}{:02}{}{:02}{}{:02}", hh, sep, mm, sep, ss, sep, ff)
}

/// Compute timecode values for a file's `<timecode>` block.
/// When `source` is provided, uses the tmcd track's own quanta and drop-frame flag;
/// otherwise falls back to video rate with NTSC-based drop-frame guess.
pub fn timecode_tags(
    source: Option<SourceTimecode>,
    video_timebase: i64,
    video_ntsc: bool,
) -> (i64, bool, i64, bool, String) {
    let base = source.map_or(video_timebase, |s| s.quanta);
    let drop_frame = source.map_or(video_ntsc && video_timebase % 30 == 0, |s| s.drop_frame);
    let ntsc = if drop_frame { true } else { video_ntsc };
    let frame = source.map_or(0, |s| s.frame);
    let string = format_timecode(frame, base, drop_frame);
    (base, ntsc, frame, drop_frame, string)
}

/// XMEML 4 / Final Cut Pro 7 XML export (XML-001).
/// Pure string generation with no platform dependencies.
pub struct XmlExport;

impl XmlExport {
    /// Generate XMEML 4 XML for a given timeline.
    /// `media_timecodes` maps media_ref → SourceTimecode for files with a tmcd track.
    /// XML-001: XMEML 4, not FCPXML.
    /// XML-010: Visual track order is reversed.
    /// XML-013: Text overlays are not preserved.
    /// XML-014: Flip state is not preserved.
    /// XML-015: Keyframe easing curves not preserved.
    pub fn export(timeline: &Timeline) -> String {
        Self::export_with_timecodes(timeline, None, None)
    }

    /// Like [`XmlExport::export`] but resolves real media file paths (as
    /// `file://localhost//…` pathurls) and dedupes repeated files from `manifest`.
    pub fn export_with_manifest(timeline: &Timeline, manifest: &MediaManifest) -> String {
        Self::export_with_timecodes(timeline, None, Some(manifest))
    }

    /// Like [`XmlExport::export_with_manifest`] but resolves nested-timeline
    /// carriers (upstream #255) against the project's sibling timelines,
    /// emitting real nested `<sequence>` nodes like Swift: the first use of a
    /// child inlines its full `<sequence id="sequence-N">`, later uses emit a
    /// self-closing reference (the Premiere convention).
    pub fn export_with_manifest_and_timelines(
        timeline: &Timeline,
        manifest: &MediaManifest,
        timelines: &HashMap<String, Timeline>,
    ) -> String {
        Self::export_with_timecodes_and_timelines(timeline, None, Some(manifest), timelines)
    }

    fn export_with_timecodes(
        timeline: &Timeline,
        media_timecodes: Option<&HashMap<String, SourceTimecode>>,
        manifest: Option<&MediaManifest>,
    ) -> String {
        Self::export_with_timecodes_and_timelines(
            timeline,
            media_timecodes,
            manifest,
            &HashMap::new(),
        )
    }

    fn export_with_timecodes_and_timelines(
        timeline: &Timeline,
        media_timecodes: Option<&HashMap<String, SourceTimecode>>,
        manifest: Option<&MediaManifest>,
        timelines: &HashMap<String, Timeline>,
    ) -> String {
        // XML-011 dedup: a full <file> is emitted once per (media_ref, is_audio);
        // later uses become self-closing <file id="…"/> references.
        let mut emitted: HashSet<String> = HashSet::new();
        let mut nest = NestCtx {
            timelines,
            sequence_ids: HashMap::new(),
            emitted_sequences: HashSet::new(),
        };
        let mut xml = String::new();
        writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").ok();
        writeln!(xml, "<!DOCTYPE xmeml>").ok();
        writeln!(xml, "<xmeml version=\"4\">").ok();
        write_sequence(
            &mut xml,
            None,
            "Timeline",
            timeline,
            media_timecodes,
            manifest,
            &mut emitted,
            &mut nest,
            0,
        );
        writeln!(xml, "</xmeml>").ok();
        xml
    }
}

/// Nested-sequence emission state (upstream #255): child timeline id → XMEML
/// sequence id, plus which children already emitted their full node.
struct NestCtx<'a> {
    timelines: &'a HashMap<String, Timeline>,
    sequence_ids: HashMap<String, String>,
    emitted_sequences: HashSet<String>,
}

/// One timeline as a `<sequence>`; used for the root and recursively for
/// nested timelines (mirrors Swift `sequenceNode`).
#[allow(clippy::too_many_arguments)]
fn write_sequence(
    xml: &mut String,
    id_attr: Option<&str>,
    name: &str,
    timeline: &Timeline,
    media_timecodes: Option<&HashMap<String, SourceTimecode>>,
    manifest: Option<&MediaManifest>,
    emitted: &mut HashSet<String>,
    nest: &mut NestCtx,
    depth: usize,
) {
    match id_attr {
        Some(id) => writeln!(xml, "  <sequence id=\"{}\">", xml_escape(id)).ok(),
        None => writeln!(xml, "  <sequence>").ok(),
    };
    writeln!(xml, "    <name>{}</name>", xml_escape(name)).ok();
    writeln!(
        xml,
        "    <duration>{}</duration>",
        timeline_total_frames(timeline)
    )
    .ok();
    writeln!(xml, "    <rate>").ok();
    writeln!(xml, "      <timebase>{}</timebase>", timeline.fps).ok();
    writeln!(xml, "      <ntsc>FALSE</ntsc>").ok();
    writeln!(xml, "    </rate>").ok();
    writeln!(xml, "    <media>").ok();

    // Video tracks (XML-010: reversed order)
    let video_tracks: Vec<&Track> = timeline
        .tracks
        .iter()
        .filter(|t| t.r#type != ClipType::Audio)
        .collect();
    writeln!(xml, "      <video>").ok();
    for (i, track) in video_tracks.iter().rev().enumerate() {
        writeln!(xml, "        <track>").ok();
        writeln!(xml, "          <trackindex>{}</trackindex>", i + 1).ok();
        writeln!(xml, "          <videotrack>").ok();
        writeln!(
            xml,
            "            <enabled>{}</enabled>",
            if track.hidden { "FALSE" } else { "TRUE" }
        )
        .ok();
        writeln!(xml, "            <locked>FALSE</locked>").ok();

        for clip in &track.clips {
            if clip.media_type == ClipType::Text || clip.media_type == ClipType::Shape {
                // XML-013: Text/shape overlays not preserved
                continue;
            }
            if clip.source_clip_type == ClipType::Sequence {
                write_fade_transition(xml, clip, timeline.fps, true, false);
                write_nest_clipitem(
                    xml,
                    clip,
                    false,
                    timeline.fps,
                    media_timecodes,
                    manifest,
                    emitted,
                    nest,
                    depth,
                );
                write_fade_transition(xml, clip, timeline.fps, false, false);
                continue;
            }
            let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
            write_fade_transition(xml, clip, timeline.fps, true, false);
            write_clip(xml, clip, timeline.fps, tc, manifest, false, emitted);
            write_fade_transition(xml, clip, timeline.fps, false, false);
        }
        writeln!(xml, "          </videotrack>").ok();
        writeln!(xml, "        </track>").ok();
    }
    writeln!(xml, "      </video>").ok();

    // Audio tracks
    let audio_tracks: Vec<&Track> = timeline
        .tracks
        .iter()
        .filter(|t| t.r#type == ClipType::Audio)
        .collect();
    writeln!(xml, "      <audio>").ok();
    for (i, track) in audio_tracks.iter().enumerate() {
        writeln!(xml, "        <track>").ok();
        writeln!(xml, "          <trackindex>{}</trackindex>", i + 1).ok();
        writeln!(xml, "          <audiotrack>").ok();
        writeln!(
            xml,
            "            <enabled>{}</enabled>",
            if track.muted { "FALSE" } else { "TRUE" }
        )
        .ok();
        writeln!(xml, "            <locked>FALSE</locked>").ok();
        for clip in &track.clips {
            if clip.source_clip_type == ClipType::Sequence {
                write_fade_transition(xml, clip, timeline.fps, true, true);
                write_nest_clipitem(
                    xml,
                    clip,
                    true,
                    timeline.fps,
                    media_timecodes,
                    manifest,
                    emitted,
                    nest,
                    depth,
                );
                write_fade_transition(xml, clip, timeline.fps, false, true);
                continue;
            }
            let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
            write_fade_transition(xml, clip, timeline.fps, true, true);
            write_clip(xml, clip, timeline.fps, tc, manifest, true, emitted);
            write_fade_transition(xml, clip, timeline.fps, false, true);
        }
        writeln!(xml, "          </audiotrack>").ok();
        writeln!(xml, "        </track>").ok();
    }
    writeln!(xml, "      </audio>").ok();

    writeln!(xml, "    </media>").ok();
    writeln!(xml, "  </sequence>").ok();
}

/// A nest carrier as a `<clipitem>` embedding (or referencing) the child
/// timeline's `<sequence>` (mirrors Swift `nestClipItemNode`): the full node on
/// first use, a self-closing `<sequence id="…"/>` reference afterwards. A
/// frozen carrier trimmed past the child's length is skipped (out < in).
#[allow(clippy::too_many_arguments)]
fn write_nest_clipitem(
    xml: &mut String,
    clip: &Clip,
    // Carrier volume/motion filters (Swift emits them on nest clipitems too)
    // are a follow-up; the carrier body is identical for video/audio today.
    _is_audio: bool,
    fps: i64,
    media_timecodes: Option<&HashMap<String, SourceTimecode>>,
    manifest: Option<&MediaManifest>,
    emitted: &mut HashSet<String>,
    nest: &mut NestCtx,
    depth: usize,
) {
    if depth >= timeline_core::NEST_MAX_DEPTH {
        return;
    }
    let Some(child) = nest.timelines.get(&clip.media_ref).cloned() else {
        return; // missing child exports nothing
    };
    let child_total = timeline_total_frames(&child);
    let in_point = clip.trim_start_frame;
    let out_point = (in_point + clip.duration_frames).min(child_total);
    if out_point <= in_point {
        return; // frozen carrier trimmed past the child's current length
    }

    let seq_id = match nest.sequence_ids.get(&clip.media_ref) {
        Some(id) => id.clone(),
        None => {
            let id = format!("sequence-{}", nest.sequence_ids.len() + 1);
            nest.sequence_ids.insert(clip.media_ref.clone(), id.clone());
            id
        }
    };

    writeln!(xml, "            <clipitem id=\"{}\">", xml_escape(&clip.id)).ok();
    writeln!(xml, "              <name>{}</name>", xml_escape(&child.name)).ok();
    writeln!(xml, "              <enabled>TRUE</enabled>").ok();
    writeln!(xml, "              <duration>{child_total}</duration>").ok();
    writeln!(xml, "              <rate>").ok();
    writeln!(xml, "                <timebase>{fps}</timebase>").ok();
    writeln!(xml, "                <ntsc>FALSE</ntsc>").ok();
    writeln!(xml, "              </rate>").ok();
    writeln!(xml, "              <start>{}</start>", clip.start_frame).ok();
    writeln!(
        xml,
        "              <end>{}</end>",
        clip.start_frame + (out_point - in_point)
    )
    .ok();
    writeln!(xml, "              <in>{in_point}</in>").ok();
    writeln!(xml, "              <out>{out_point}</out>").ok();

    if nest.emitted_sequences.insert(clip.media_ref.clone()) {
        let child_name = child.name.clone();
        write_sequence(
            xml,
            Some(&seq_id),
            &child_name,
            &child,
            media_timecodes,
            manifest,
            emitted,
            nest,
            depth + 1,
        );
    } else {
        writeln!(xml, "  <sequence id=\"{}\"/>", xml_escape(&seq_id)).ok();
    }
    writeln!(xml, "            </clipitem>").ok();
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

/// Emit a fade as a single-sided `<transitionitem>` — the form Premiere reads (XML-007), a
/// dissolve to black (video) / silence (audio). `is_left` = the fade-in edge; the right edge is
/// the fade-out. Sibling of the clipitem at the track level. Mirrors Swift `fadeTransition`.
fn write_fade_transition(xml: &mut String, clip: &Clip, fps: i64, is_left: bool, is_audio: bool) {
    let frames = if is_left {
        clip.fade_in_frames
    } else {
        clip.fade_out_frames
    };
    if frames <= 0 || clip.media_ref.is_empty() {
        return;
    }
    let end_frame = clip.start_frame + clip.duration_frames;
    let (start, end, alignment, cut_frames) = if is_left {
        (clip.start_frame, clip.start_frame + frames, "start-black", 0)
    } else {
        (end_frame - frames, end_frame, "end-black", frames)
    };
    writeln!(xml, "            <transitionitem>").ok();
    writeln!(xml, "              <start>{start}</start>").ok();
    writeln!(xml, "              <end>{end}</end>").ok();
    writeln!(xml, "              <alignment>{alignment}</alignment>").ok();
    if !is_audio {
        // Premiere's private cut-point in ticks (254016000000/sec): 0 for a fade-in, the full
        // length for a fade-out.
        let cut_point_ticks = cut_frames as i64 * (254_016_000_000i64 / fps.max(1));
        writeln!(xml, "              <cutPointTicks>{cut_point_ticks}</cutPointTicks>").ok();
    }
    writeln!(xml, "              <rate>").ok();
    writeln!(xml, "                <timebase>{fps}</timebase>").ok();
    writeln!(xml, "                <ntsc>FALSE</ntsc>").ok();
    writeln!(xml, "              </rate>").ok();
    writeln!(xml, "              <effect>").ok();
    if is_audio {
        writeln!(xml, "                <name>Cross Fade ( 0dB)</name>").ok();
        writeln!(xml, "                <effectid>KGAudioTransCrossFade0dB</effectid>").ok();
        writeln!(xml, "                <effecttype>transition</effecttype>").ok();
        writeln!(xml, "                <mediatype>audio</mediatype>").ok();
    } else {
        writeln!(xml, "                <name>Cross Dissolve</name>").ok();
        writeln!(xml, "                <effectid>Cross Dissolve</effectid>").ok();
        writeln!(xml, "                <effectcategory>Dissolve</effectcategory>").ok();
        writeln!(xml, "                <effecttype>transition</effecttype>").ok();
        writeln!(xml, "                <mediatype>video</mediatype>").ok();
        writeln!(xml, "                <wipecode>0</wipecode>").ok();
        writeln!(xml, "                <wipeaccuracy>100</wipeaccuracy>").ok();
        writeln!(xml, "                <startratio>0</startratio>").ok();
        writeln!(xml, "                <endratio>1</endratio>").ok();
        writeln!(xml, "                <reverse>FALSE</reverse>").ok();
    }
    writeln!(xml, "              </effect>").ok();
    writeln!(xml, "            </transitionitem>").ok();
}

/// Write a single clip element.
/// Emit a Basic Motion `<parameter>`. With keyframes it writes one `<keyframe>`
/// per entry (`when` = clip-relative frame); otherwise the single static value.
/// XML-012 (keyframed params). NLE round-trip is not machine-verified here; the
/// tests assert the emitted XMEML structure faithfully mirrors the track.
fn write_motion_param(xml: &mut String, id: &str, static_value: String, keyframes: &[(i64, String)]) {
    writeln!(xml, "                  <parameter>").ok();
    writeln!(xml, "                    <parameterid>{id}</parameterid>").ok();
    if keyframes.is_empty() {
        writeln!(xml, "                    <value>{static_value}</value>").ok();
    } else {
        for (when, value) in keyframes {
            writeln!(
                xml,
                "                    <keyframe><when>{when}</when><value>{value}</value></keyframe>"
            )
            .ok();
        }
    }
    writeln!(xml, "                  </parameter>").ok();
}

/// XML-002~008: preserves clip placement, trims, speed, volume, opacity, transform, crop, fades.
/// XML-011: a full `<file>` is emitted once per (media_ref, is_audio); repeats
///          become self-closing `<file id="…"/>` references. When `manifest`
///          resolves the media, the pathurl is the real `file://localhost//…` path.
/// XML-012: clips without media_ref are skipped.
/// `timecode` is the optional SourceTimecode from the tmcd track. Upstream PR #136.
fn write_clip(
    xml: &mut String,
    clip: &Clip,
    fps: i64,
    timecode: Option<SourceTimecode>,
    manifest: Option<&MediaManifest>,
    is_audio: bool,
    emitted: &mut HashSet<String>,
) {
    if clip.media_ref.is_empty() {
        // XML-012: skip unresolved media
        return;
    }

    // Human-readable clip/file name (Premiere shows this in the timeline) — the resolved asset
    // name, not the raw media_ref id. Mirrors Swift `resolver.displayName(for:)`.
    let display_name = manifest
        .and_then(|m| m.entry_for(&clip.media_ref))
        .map(|e| e.name.clone())
        .unwrap_or_else(|| clip.media_ref.clone());

    writeln!(xml, "            <clipitem id=\"{}\">", xml_escape(&clip.id)).ok();
    writeln!(xml, "              <name>{}</name>", xml_escape(&display_name)).ok();
    writeln!(
        xml,
        "              <duration>{}</duration>",
        clip.duration_frames
    )
    .ok();
    writeln!(xml, "              <rate>").ok();
    writeln!(xml, "                <timebase>{}</timebase>", fps).ok();
    writeln!(xml, "                <ntsc>FALSE</ntsc>").ok();
    writeln!(xml, "              </rate>").ok();

    // XML-002: clip placement
    writeln!(xml, "              <start>{}</start>", clip.start_frame).ok();
    // XML-003: source trims
    writeln!(xml, "              <in>{}</in>", clip.trim_start_frame).ok();
    writeln!(xml, "              <out>{}</out>", clip.trim_end_frame).ok();
    // XML-004: speed changes
    if (clip.speed - 1.0).abs() > f64::EPSILON {
        writeln!(xml, "              <speed>").ok();
        writeln!(xml, "                <value>{:.3}</value>", clip.speed).ok();
        writeln!(xml, "                <timebase>{}</timebase>", fps).ok();
        writeln!(xml, "                <ntsc>FALSE</ntsc>").ok();
        writeln!(xml, "              </speed>").ok();
    }
    // XML-005: volume
    writeln!(xml, "              <volume>").ok();
    writeln!(xml, "                <value>{:.6}</value>", clip.volume).ok();
    writeln!(xml, "              </volume>").ok();
    // XML-005: opacity
    writeln!(xml, "              <opacity>{:.6}</opacity>", clip.opacity).ok();
    // XML-007: fades are emitted as sibling <transitionitem>s at the track level (Premiere
    // reads those, not <fadein>/<fadeout>), so they're written around the clipitem, not here.
    // XML-006: transform and crop
    writeln!(xml, "              <filter>").ok();
    writeln!(xml, "                <effect>").ok();
    writeln!(xml, "                  <name>Basic Motion</name>").ok();
    writeln!(
        xml,
        "                  <effectcategory>motion</effectcategory>"
    )
    .ok();
    writeln!(xml, "                  <effecttype>motion</effecttype>").ok();
    // XML-012: keyframed motion params when a track is present, else static.
    // Scale is uniform in Basic Motion; follow the static convention and take
    // the AnimPair's first component (matches transform.width above).
    let scale_kf: Vec<(i64, String)> = clip
        .scale_track
        .as_ref()
        .map(|t| {
            t.keyframes
                .iter()
                .map(|k| (k.frame, format!("{:.6}", k.value.a)))
                .collect()
        })
        .unwrap_or_default();
    write_motion_param(xml, "scale", format!("{:.6}", clip.transform.width), &scale_kf);

    let rotation_kf: Vec<(i64, String)> = clip
        .rotation_track
        .as_ref()
        .map(|t| {
            t.keyframes
                .iter()
                .map(|k| (k.frame, format!("{:.6}", k.value)))
                .collect()
        })
        .unwrap_or_default();
    write_motion_param(
        xml,
        "rotation",
        format!("{:.6}", clip.transform.rotation),
        &rotation_kf,
    );

    // Center keyframes: position_track stores TOP-LEFT, but the "center" param is
    // the resolved centre (top_left + size/2). Because centre depends on both
    // position and size, sample the resolved transform at the union of position +
    // scale keyframe frames (matches Swift XMLExporter.motionFilter). Only emitted
    // when position is animated — a static position keeps centre fixed even under
    // an animated scale, which scales around the centre.
    let position_active = clip
        .position_track
        .as_ref()
        .is_some_and(|t| !t.keyframes.is_empty());
    let center_kf: Vec<(i64, String)> = if position_active {
        let mut frames: Vec<i64> = Vec::new();
        if let Some(t) = clip.position_track.as_ref() {
            frames.extend(t.keyframes.iter().map(|k| k.frame));
        }
        if let Some(t) = clip.scale_track.as_ref() {
            frames.extend(t.keyframes.iter().map(|k| k.frame));
        }
        frames.sort_unstable();
        frames.dedup();
        frames
            .into_iter()
            .map(|f| {
                let tr = timeline_core::resolved_transform_at(clip, f);
                (f, format!("{:.6} {:.6}", tr.center_x, tr.center_y))
            })
            .collect()
    } else {
        Vec::new()
    };
    write_motion_param(
        xml,
        "center",
        format!("{:.6} {:.6}", clip.transform.center_x, clip.transform.center_y),
        &center_kf,
    );

    let crop_kf: Vec<(i64, String)> = clip
        .crop_track
        .as_ref()
        .map(|t| {
            t.keyframes
                .iter()
                .map(|k| {
                    (
                        k.frame,
                        format!(
                            "{:.6} {:.6} {:.6} {:.6}",
                            k.value.left, k.value.top, k.value.right, k.value.bottom
                        ),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    write_motion_param(
        xml,
        "crop",
        format!(
            "{:.6} {:.6} {:.6} {:.6}",
            clip.crop.left, clip.crop.top, clip.crop.right, clip.crop.bottom
        ),
        &crop_kf,
    );
    writeln!(xml, "                </effect>").ok();
    writeln!(xml, "              </filter>").ok();

    // XML-012: keyframed opacity becomes a dedicated Opacity filter so the
    // animation survives export; static opacity stays on the clipitem above.
    if let Some(track) = clip.opacity_track.as_ref() {
        if !track.keyframes.is_empty() {
            let opacity_kf: Vec<(i64, String)> = track
                .keyframes
                .iter()
                .map(|k| (k.frame, format!("{:.6}", k.value)))
                .collect();
            writeln!(xml, "              <filter>").ok();
            writeln!(xml, "                <effect>").ok();
            writeln!(xml, "                  <name>Opacity</name>").ok();
            writeln!(xml, "                  <effectcategory>motion</effectcategory>").ok();
            writeln!(xml, "                  <effecttype>opacity</effecttype>").ok();
            write_motion_param(
                xml,
                "opacity",
                format!("{:.6}", clip.opacity),
                &opacity_kf,
            );
            writeln!(xml, "                </effect>").ok();
            writeln!(xml, "              </filter>").ok();
        }
    }

    // XML-008: linked clip relationships
    if let Some(ref link_id) = clip.link_group_id {
        writeln!(xml, "              <link>").ok();
        writeln!(
            xml,
            "                <linkclipref>{}</linkclipref>",
            link_id
        )
        .ok();
        writeln!(xml, "                <medialink>true</medialink>").ok();
        writeln!(xml, "              </link>").ok();
    }

    // File reference (XML-011): dedup by (media_ref, is_audio); resolve real path.
    let file_id = format!("{}-{}", clip.media_ref, if is_audio { "a" } else { "v" });
    if emitted.contains(&file_id) {
        writeln!(xml, "              <file id=\"{}\"/>", xml_escape(&file_id)).ok();
        writeln!(xml, "            </clipitem>").ok();
        return;
    }
    emitted.insert(file_id.clone());
    let entry = manifest.and_then(|m| m.entry_for(&clip.media_ref));
    let pathurl = entry
        .map(|e| media_src(&e.source))
        .unwrap_or_else(|| clip.media_ref.clone());
    writeln!(xml, "              <file id=\"{}\">", xml_escape(&file_id)).ok();
    writeln!(xml, "                <name>{}</name>", xml_escape(&display_name)).ok();
    writeln!(
        xml,
        "                <pathurl>{}</pathurl>",
        xml_escape(&pathurl)
    )
    .ok();
    writeln!(xml, "                <rate>").ok();
    writeln!(xml, "                  <timebase>{}</timebase>", fps).ok();
    writeln!(xml, "                </rate>").ok();
    // XML-009: source timecode (PR #136)
    let ntsc = fps % 30 == 0 && fps <= 60;
    let tc = timecode_tags(timecode, fps, ntsc);
    writeln!(xml, "                <timecode>").ok();
    writeln!(xml, "                  <rate>").ok();
    writeln!(xml, "                    <timebase>{}</timebase>", tc.0).ok();
    writeln!(
        xml,
        "                    <ntsc>{}</ntsc>",
        if tc.1 { "TRUE" } else { "FALSE" }
    )
    .ok();
    writeln!(xml, "                  </rate>").ok();
    writeln!(xml, "                  <string>{}</string>", tc.4).ok();
    writeln!(xml, "                  <frame>{}</frame>", tc.2).ok();
    writeln!(
        xml,
        "                  <displayformat>{}</displayformat>",
        if tc.3 { "DF" } else { "NDF" }
    )
    .ok();
    writeln!(xml, "                </timecode>").ok();
    writeln!(xml, "              </file>").ok();

    writeln!(xml, "            </clipitem>").ok();
}

/// Build a Premiere-friendly `file://localhost//…` pathurl from a media source
/// (upstream #14). Project-relative media keeps its relative path.
fn media_src(source: &MediaSource) -> String {
    match source {
        MediaSource::External { absolute_path } => {
            let p = absolute_path.replace('\\', "/");
            format!("file://localhost//{}", p.trim_start_matches('/'))
        }
        MediaSource::Project { relative_path } => relative_path.clone(),
    }
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
    use core_model::Interpolation;

    fn test_clip(id: &str, media_ref: &str, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.into(),
            media_ref: media_ref.into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: Default::default(),
            crop: Default::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            text_animation: None,
            word_timings: None,
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

    #[test]
    fn nested_sequences_emit_inline_then_reference() {
        // Two carriers over the SAME child: the first inlines the full
        // <sequence id="sequence-1">, the second emits a self-closing reference.
        let mut child = Timeline {
            name: "Insert Cut".into(),
            ..Default::default()
        };
        child.id = "child-1".into();
        child.tracks.push(Track {
            id: "ct".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![test_clip("inner", "m-child", 0, 40)],
        });

        let mut carrier_a = test_clip("carA", "child-1", 0, 40);
        carrier_a.media_type = ClipType::Sequence;
        carrier_a.source_clip_type = ClipType::Sequence;
        let mut carrier_b = test_clip("carB", "child-1", 40, 40);
        carrier_b.media_type = ClipType::Sequence;
        carrier_b.source_clip_type = ClipType::Sequence;
        // A frozen carrier trimmed past the child's length must be skipped.
        let mut frozen = test_clip("carC", "child-1", 80, 10);
        frozen.media_type = ClipType::Sequence;
        frozen.source_clip_type = ClipType::Sequence;
        frozen.trim_start_frame = 100;

        let mut parent = Timeline::default();
        parent.tracks.push(Track {
            id: "pt".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
            display_height: 50.0,
            clips: vec![carrier_a, carrier_b, frozen],
        });
        let timelines = HashMap::from([("child-1".to_string(), child)]);

        let xml = XmlExport::export_with_manifest_and_timelines(
            &parent,
            &MediaManifest::default(),
            &timelines,
        );
        assert_eq!(
            xml.matches("<sequence id=\"sequence-1\">").count(),
            1,
            "full child sequence inlined exactly once: {xml}"
        );
        assert_eq!(
            xml.matches("<sequence id=\"sequence-1\"/>").count(),
            1,
            "second use is a self-closing reference"
        );
        assert_eq!(
            xml.matches("<name>Insert Cut</name>").count(),
            3,
            "both carrier clipitems + the nested sequence use the child's name"
        );
        assert!(
            !xml.contains("carC"),
            "frozen carrier past the child length is dropped"
        );
        // The nested sequence carries the child's clip.
        assert!(xml.contains("clipitem id=\"inner\""), "{xml}");
    }

    fn sample_timeline() -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![Clip {
                    id: "clip1".into(),
                    media_ref: "asset-video.mp4".into(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    start_frame: 0,
                    duration_frames: 100,
                    trim_start_frame: 10,
                    trim_end_frame: 110,
                    speed: 1.0,
                    volume: 1.0,
                    opacity: 1.0,
                    fade_in_frames: 5,
                    fade_out_frames: 8,
                    fade_in_interpolation: Interpolation::Linear,
                    fade_out_interpolation: Interpolation::Linear,
                    transform: core_model::Transform::default(),
                    crop: core_model::Crop::default(),
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
                }],
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            folder_id: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    fn mk_clip(id: &str, media_ref: &str, kind: ClipType, start: i64) -> Clip {
        Clip {
            id: id.into(),
            media_ref: media_ref.into(),
            media_type: kind.clone(),
            source_clip_type: kind,
            start_frame: start,
            duration_frames: 30,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: core_model::Transform::default(),
            crop: core_model::Crop::default(),
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

    fn tl_with(clips: Vec<Clip>) -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                display_height: 50.0,
                clips,
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            folder_id: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    fn manifest_with(media_ref: &str, name: &str, path: &str) -> MediaManifest {
        let mut m = MediaManifest::default();
        m.entries.push(core_model::MediaManifestEntry {
            id: media_ref.into(),
            name: name.into(),
            r#type: ClipType::Video,
            source: MediaSource::External {
                absolute_path: path.into(),
            },
            duration: 5.0,
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
        });
        m
    }

    #[test]
    fn xml_014_manifest_resolves_file_localhost_pathurl() {
        let timeline = tl_with(vec![mk_clip("c1", "m1", ClipType::Video, 0)]);
        let m = manifest_with("m1", "shot.mp4", "/media/shot.mp4");
        let xml = XmlExport::export_with_manifest(&timeline, &m);
        assert!(
            xml.contains("<pathurl>file://localhost//media/shot.mp4</pathurl>"),
            "{xml}"
        );
        assert!(xml.contains("<name>shot.mp4</name>"));
        // Without a manifest, pathurl falls back to the media_ref (unchanged).
        assert!(XmlExport::export(&timeline).contains("<pathurl>m1</pathurl>"));
    }

    #[test]
    fn xml_clipitem_name_is_display_name_not_media_ref_id() {
        // The clipitem <name> (shown in Premiere's timeline) must be the resolved asset
        // name, not the raw media_ref id.
        let timeline = tl_with(vec![mk_clip("c1", "vid-id-123", ClipType::Video, 0)]);
        let m = manifest_with("vid-id-123", "Interview.mp4", "/media/Interview.mp4");
        let xml = XmlExport::export_with_manifest(&timeline, &m);
        assert!(xml.contains("<name>Interview.mp4</name>"), "resolved name\n{xml}");
        assert!(!xml.contains("<name>vid-id-123</name>"), "id never used as a name\n{xml}");
    }

    #[test]
    fn xml_015_file_dedup_emits_one_full_and_self_closing_repeats() {
        let timeline = tl_with(vec![
            mk_clip("c1", "m1", ClipType::Video, 0),
            mk_clip("c2", "m1", ClipType::Video, 30),
        ]);
        let xml = XmlExport::export(&timeline);
        assert_eq!(
            xml.matches("<file id=\"m1-v\">").count(),
            1,
            "one full <file> for the shared media"
        );
        assert_eq!(
            xml.matches("<file id=\"m1-v\"/>").count(),
            1,
            "the repeat is a self-closing reference"
        );
    }

    #[test]
    fn clipitem_name_is_xml_escaped() {
        // A media_ref (free-form filename) with XML metacharacters must be escaped in
        // the clipitem <name>, else the whole sequence is malformed and rejected.
        let clip = mk_clip("c1", "A&B<take>.mp4", ClipType::Video, 0);
        let xml = XmlExport::export(&tl_with(vec![clip]));
        assert!(
            xml.contains("<name>A&amp;B&lt;take&gt;.mp4</name>"),
            "clipitem name escaped:\n{xml}"
        );
        assert!(!xml.contains("<take>"), "no raw metacharacters leak into the XML");
    }

    #[test]
    fn xml_001_xmeml_4_format() {
        let xml = XmlExport::export(&sample_timeline());
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<!DOCTYPE xmeml>"));
        assert!(xml.contains("<xmeml version=\"4\">"));
        assert!(xml.contains("</xmeml>"));
    }

    #[test]
    fn xml_002_clip_placement_preserved() {
        let xml = XmlExport::export(&sample_timeline());
        assert!(xml.contains("<start>0</start>"));
        assert!(xml.contains("<duration>100</duration>"));
    }

    #[test]
    fn xml_003_source_trims_preserved() {
        let xml = XmlExport::export(&sample_timeline());
        assert!(xml.contains("<in>10</in>"));
        assert!(xml.contains("<out>110</out>"));
    }

    #[test]
    fn xml_005_volume_and_opacity_preserved() {
        let xml = XmlExport::export(&sample_timeline());
        assert!(xml.contains("<volume>"));
        assert!(xml.contains("<opacity>1.000000</opacity>"));
    }

    #[test]
    fn xml_007_fades_are_transitionitems() {
        // XML-007: fades export as single-sided Cross Dissolve <transitionitem>s (the form
        // Premiere reads), not <fadein>/<fadeout> tags. Clip is [0,100), fade-in 5, fade-out 8.
        let xml = XmlExport::export(&sample_timeline());
        assert!(
            !xml.contains("<fadein>") && !xml.contains("<fadeout>"),
            "legacy fade tags removed\n{xml}"
        );
        assert!(xml.contains("<transitionitem>"), "transitionitem emitted");
        assert!(xml.contains("<name>Cross Dissolve</name>"), "video dissolve");
        assert!(xml.contains("<effectid>Cross Dissolve</effectid>"));
        // fade-in: start-black, ends at frame 5, cut at 0.
        assert!(xml.contains("<alignment>start-black</alignment>"), "fade-in edge");
        assert!(xml.contains("<end>5</end>"), "fade-in ends at 5");
        assert!(xml.contains("<cutPointTicks>0</cutPointTicks>"), "fade-in cut at 0");
        // fade-out: end-black, starts at frame 92 (100-8).
        assert!(xml.contains("<alignment>end-black</alignment>"), "fade-out edge");
        assert!(xml.contains("<start>92</start>"), "fade-out starts at 92");
    }

    #[test]
    fn xml_007_audio_fade_is_cross_fade() {
        // An audio fade uses Cross Fade (not Cross Dissolve) and omits the video-only
        // cutPointTicks/wipe params.
        let mut c = mk_clip("a1", "voice.wav", ClipType::Audio, 0);
        c.fade_in_frames = 4;
        let tl = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: Default::default(),
            tracks: vec![Track {
                id: "aud".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: false,
               display_height: 50.0,
                clips: vec![c],
            }],
            transcription_language: None,
            folder_id: None,
            compound_timelines: Default::default(),
        };
        let xml = XmlExport::export(&tl);
        assert!(xml.contains("<transitionitem>"), "audio transitionitem\n{xml}");
        assert!(xml.contains("<name>Cross Fade ( 0dB)</name>"), "cross fade name");
        assert!(xml.contains("<effectid>KGAudioTransCrossFade0dB</effectid>"));
        assert!(!xml.contains("<cutPointTicks>"), "audio omits cutPointTicks");
        assert!(!xml.contains("<wipecode>"), "audio omits video wipe params");
    }

    #[test]
    fn xml_010_visual_track_order_reversed() {
        let mut timeline = sample_timeline();
        timeline.tracks.push(Track {
            id: "v2".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![Clip {
                id: "clip2".into(),
                media_ref: "asset2.mp4".into(),
                media_type: ClipType::Video,
                ..sample_timeline().tracks[0].clips[0].clone()
            }],
        });
        let xml = XmlExport::export(&timeline);
        // Second track added should appear first in XML (reversed)
        let v1_pos = xml.find("clip1").unwrap();
        let v2_pos = xml.find("clip2").unwrap();
        assert!(v2_pos < v1_pos, "visual track order should be reversed");
    }

    #[test]
    fn xml_012_unresolved_media_skipped() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].clips.push(Clip {
            id: "empty".into(),
            media_ref: String::new(),
            media_type: ClipType::Video,
            ..sample_timeline().tracks[0].clips[0].clone()
        });
        let xml = XmlExport::export(&timeline);
        assert!(!xml.contains("clipitem id=\"empty\""));
    }

    #[test]
    fn xml_013_text_overlays_not_preserved() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].clips.push(Clip {
            id: "txt1".into(),
            media_ref: String::new(),
            media_type: ClipType::Text,
            ..sample_timeline().tracks[0].clips[0].clone()
        });
        let xml = XmlExport::export(&timeline);
        assert!(!xml.contains("txt1"));
    }

    #[test]
    fn xml_speed_change_preserved() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].clips[0].speed = 2.0;
        let xml = XmlExport::export(&timeline);
        assert!(xml.contains("<speed>"));
        assert!(xml.contains("<value>2.000</value>"));
    }

    #[test]
    fn xml_empty_timeline() {
        let timeline = Timeline::default();
        let xml = XmlExport::export(&timeline);
        assert!(xml.contains("</xmeml>"));
    }

    #[test]
    fn xml_muted_track_enabled_false() {
        let mut timeline = sample_timeline();
        timeline.tracks.push(Track {
            id: "a1".into(),
            r#type: ClipType::Audio,
            muted: true,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![],
        });
        let xml = XmlExport::export(&timeline);
        assert!(xml.contains("<enabled>FALSE</enabled>"));
    }

    #[test]
    fn xml_hidden_track_enabled_false() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].hidden = true;
        let xml = XmlExport::export(&timeline);
        // Video track should have enabled=FALSE
        assert!(xml.contains("<enabled>FALSE</enabled>"));
    }

    #[test]
    fn xml_006_transform_and_crop() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].clips[0].transform = core_model::Transform {
            width: 0.8,
            height: 0.8,
            rotation: 10.0,
            center_x: 0.5,
            center_y: 0.3,
            ..core_model::Transform::default()
        };
        timeline.tracks[0].clips[0].crop = core_model::Crop {
            left: 0.1,
            top: 0.0,
            right: 0.0,
            bottom: 0.2,
        };
        let xml = XmlExport::export(&timeline);
        // Transform should appear in filter/effect
        assert!(xml.contains("<parameterid>scale</parameterid>"));
        assert!(xml.contains("<parameterid>center</parameterid>"));
        assert!(xml.contains("<parameterid>crop</parameterid>"));
        assert!(xml.contains("0.800000")); // scale
        assert!(xml.contains("10.000000")); // rotation
    }

    #[test]
    fn xml_008_linked_clip_relationship() {
        let mut timeline = sample_timeline();
        // Add a linked audio clip
        timeline.tracks.push(Track {
            id: "a1".into(),
            r#type: ClipType::Audio,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![Clip {
                id: "clip-audio".into(),
                media_ref: "asset-audio.wav".into(),
                media_type: ClipType::Audio,
                source_clip_type: ClipType::Audio,
                start_frame: 0,
                duration_frames: 100,
                trim_start_frame: 0,
                trim_end_frame: 100,
                speed: 1.0,
                volume: 1.0,
                opacity: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                transform: core_model::Transform::default(),
                crop: core_model::Crop::default(),
                link_group_id: Some("link-1".into()),
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
            }],
        });
        let xml = XmlExport::export(&timeline);
        assert!(xml.contains("<link>"));
        assert!(xml.contains("<linkclipref>link-1</linkclipref>"));
    }

    // MARK: - timecode functions (PR #136)

    #[test]
    fn format_timecode_non_drop_zero() {
        assert_eq!(format_timecode(0, 30, false), "00:00:00:00");
    }

    #[test]
    fn format_timecode_non_drop_basic() {
        assert_eq!(format_timecode(1968620, 30, false), "18:13:40:20");
    }

    #[test]
    fn format_timecode_drop_frame_basic() {
        // 42966 frames at 30 DF = 00;23;53;18
        assert_eq!(format_timecode(42966, 30, true), "00;23;53;18");
    }

    #[test]
    fn format_timecode_drop_frame_minute_boundaries() {
        // Drop-frame skips labels ;00/;01 at each non-tenth minute, so the frame
        // field jumps at a minute boundary. These fail under nominal (undropped)
        // divisors — the coincidental 42966 case does not catch that.
        assert_eq!(format_timecode(1800, 30, true), "00;01;00;02");
        assert_eq!(format_timecode(3600, 30, true), "00;02;00;04");
        assert_eq!(format_timecode(3600, 60, true), "00;01;00;04");
        // The tenth minute does NOT drop, so one real hour reads exactly 01;00;00;00.
        assert_eq!(format_timecode(107892, 30, true), "01;00;00;00");
    }

    #[test]
    fn format_timecode_drop_frame_separator() {
        let s = format_timecode(100, 30, true);
        assert!(s.contains(';'), "drop-frame should use semicolons");
    }

    #[test]
    fn format_timecode_zero_fps_protection() {
        assert_eq!(format_timecode(100, 0, false), "00:00:00:00");
    }

    #[test]
    fn format_timecode_drop_frame_30_vs_60() {
        // 30 DF and 60 DF have different drop counts
        let s30 = format_timecode(100, 30, true);
        let s60 = format_timecode(100, 60, true);
        assert_ne!(s30, s60);
    }

    #[test]
    fn timecode_tags_non_drop_source() {
        let tc = timecode_tags(
            Some(SourceTimecode {
                frame: 1968620,
                quanta: 30,
                drop_frame: false,
            }),
            30,
            true,
        );
        assert_eq!(tc.0, 30); // base
        assert!(tc.1); // ntsc (still NTSC even though NDF)
        assert_eq!(tc.2, 1968620); // frame
        assert!(!tc.3); // drop_frame
        assert_eq!(tc.4, "18:13:40:20");
        assert!(!tc.4.contains(';'));
    }

    #[test]
    fn timecode_tags_drop_frame_source_on_60p() {
        // Fuji 59.94p: tmcd at 30 DF, not 60
        let tc = timecode_tags(
            Some(SourceTimecode {
                frame: 42966,
                quanta: 30,
                drop_frame: true,
            }),
            60,
            true,
        );
        assert_eq!(tc.0, 30); // track quanta, not video rate
        assert!(tc.3); // drop frame
        assert_eq!(tc.4, "00;23;53;18");
    }

    #[test]
    fn timecode_tags_no_source_falls_back() {
        let tc = timecode_tags(None, 30, true);
        assert_eq!(tc.0, 30);
        assert_eq!(tc.2, 0);
        assert!(tc.3); // NTSC 30 → drop frame guess
        assert_eq!(tc.4, "00;00;00;00");
    }

    #[test]
    fn timecode_tags_no_source_non_ntsc() {
        let tc = timecode_tags(None, 30, false);
        assert!(!tc.3);
        assert_eq!(tc.4, "00:00:00:00");
    }

    #[test]
    fn xml_timecode_emitted_in_file_element() {
        let mut timeline = sample_timeline();
        timeline.tracks[0].clips[0].media_ref = "test.mp4".into();
        let mut map = std::collections::HashMap::new();
        map.insert(
            "test.mp4".into(),
            SourceTimecode {
                frame: 100,
                quanta: 30,
                drop_frame: false,
            },
        );
        // Access via private method through XmlExport
        let xml = XmlExport::export_with_timecodes(&timeline, Some(&map), None);
        assert!(xml.contains("<timecode>"));
        assert!(xml.contains("<string>00:00:03:10</string>")); // frame 100 at 30fps
        assert!(xml.contains("<displayformat>NDF</displayformat>"));
    }

    #[test]
    fn xml_012_static_params_have_no_keyframes() {
        let clip = mk_clip("c1", "asset.mp4", ClipType::Video, 0);
        let xml = XmlExport::export(&tl_with(vec![clip]));
        // A clip with no tracks emits plain <value> params, no <keyframe>.
        assert!(xml.contains("<parameterid>scale</parameterid>"));
        assert!(!xml.contains("<keyframe>"));
    }

    #[test]
    fn xml_012_scale_keyframes_exported() {
        let mut clip = mk_clip("c1", "asset.mp4", ClipType::Video, 0);
        clip.scale_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: core_model::AnimPair { a: 50.0, b: 50.0 },
                    interpolation_out: Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 30,
                    value: core_model::AnimPair { a: 120.0, b: 120.0 },
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        let xml = XmlExport::export(&tl_with(vec![clip]));
        // Scale param carries keyframes at the track frames/values.
        assert!(xml.contains("<keyframe><when>0</when><value>50.000000</value></keyframe>"));
        assert!(xml.contains("<keyframe><when>30</when><value>120.000000</value></keyframe>"));
    }

    #[test]
    fn xml_012_opacity_track_becomes_opacity_filter() {
        let mut clip = mk_clip("c1", "asset.mp4", ClipType::Video, 0);
        clip.opacity_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: 0.0,
                    interpolation_out: Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 15,
                    value: 1.0,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        let xml = XmlExport::export(&tl_with(vec![clip]));
        assert!(xml.contains("<name>Opacity</name>"));
        assert!(xml.contains("<effecttype>opacity</effecttype>"));
        assert!(xml.contains("<keyframe><when>0</when><value>0.000000</value></keyframe>"));
        assert!(xml.contains("<keyframe><when>15</when><value>1.000000</value></keyframe>"));
    }
}
