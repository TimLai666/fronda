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
        let d = f / (fps * 600);
        let m = f % (fps * 600);
        f += drop * 9 * d
            + if m > drop {
                drop * ((m - drop) / (fps * 60))
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

    fn export_with_timecodes(
        timeline: &Timeline,
        media_timecodes: Option<&HashMap<String, SourceTimecode>>,
        manifest: Option<&MediaManifest>,
    ) -> String {
        // XML-011 dedup: a full <file> is emitted once per (media_ref, is_audio);
        // later uses become self-closing <file id="…"/> references.
        let mut emitted: HashSet<String> = HashSet::new();
        let mut xml = String::new();
        writeln!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").ok();
        writeln!(xml, "<!DOCTYPE xmeml>").ok();
        writeln!(xml, "<xmeml version=\"4\">").ok();
        writeln!(xml, "  <sequence>").ok();
        writeln!(xml, "    <name>Timeline</name>").ok();
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
                let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
                write_clip(&mut xml, clip, timeline.fps, tc, manifest, false, &mut emitted);
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
                let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
                write_clip(&mut xml, clip, timeline.fps, tc, manifest, true, &mut emitted);
            }
            writeln!(xml, "          </audiotrack>").ok();
            writeln!(xml, "        </track>").ok();
        }
        writeln!(xml, "      </audio>").ok();

        writeln!(xml, "    </media>").ok();
        writeln!(xml, "  </sequence>").ok();
        writeln!(xml, "</xmeml>").ok();
        xml
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

    writeln!(xml, "            <clipitem id=\"{}\">", clip.id).ok();
    writeln!(xml, "              <name>{}</name>", clip.media_ref).ok();
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
    // XML-007: fades
    if clip.fade_in_frames > 0 {
        writeln!(xml, "              <fadein>").ok();
        writeln!(
            xml,
            "                <duration>{}</duration>",
            clip.fade_in_frames
        )
        .ok();
        writeln!(xml, "              </fadein>").ok();
    }
    if clip.fade_out_frames > 0 {
        writeln!(xml, "              <fadeout>").ok();
        writeln!(
            xml,
            "                <duration>{}</duration>",
            clip.fade_out_frames
        )
        .ok();
        writeln!(xml, "              </fadeout>").ok();
    }
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

    let center_kf: Vec<(i64, String)> = clip
        .position_track
        .as_ref()
        .map(|t| {
            t.keyframes
                .iter()
                .map(|k| (k.frame, format!("{:.6} {:.6}", k.value.a, k.value.b)))
                .collect()
        })
        .unwrap_or_default();
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
    let name = entry
        .map(|e| e.name.clone())
        .unwrap_or_else(|| clip.media_ref.clone());
    let pathurl = entry
        .map(|e| media_src(&e.source))
        .unwrap_or_else(|| clip.media_ref.clone());
    writeln!(xml, "              <file id=\"{}\">", xml_escape(&file_id)).ok();
    writeln!(xml, "                <name>{}</name>", xml_escape(&name)).ok();
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

    fn sample_timeline() -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
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
                }],
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
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
        }
    }

    fn tl_with(clips: Vec<Clip>) -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips,
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
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
    fn xml_007_fades_preserved() {
        let xml = XmlExport::export(&sample_timeline());
        assert!(xml.contains("<fadein>"));
        assert!(xml.contains("<duration>5</duration>"));
        assert!(xml.contains("<fadeout>"));
        assert!(xml.contains("<duration>8</duration>"));
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
