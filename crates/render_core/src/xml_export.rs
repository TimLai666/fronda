use core_model::{Clip, ClipType, Timeline, Track};
use std::collections::HashMap;
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
        Self::export_with_timecodes(timeline, None)
    }

    fn export_with_timecodes(
        timeline: &Timeline,
        media_timecodes: Option<&HashMap<String, SourceTimecode>>,
    ) -> String {
        let mut xml = String::new();
        write!(xml, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n").ok();
        write!(xml, "<!DOCTYPE xmeml>\n").ok();
        write!(xml, "<xmeml version=\"4\">\n").ok();
        write!(xml, "  <sequence>\n").ok();
        write!(xml, "    <name>Timeline</name>\n").ok();
        write!(
            xml,
            "    <duration>{}</duration>\n",
            timeline_total_frames(timeline)
        )
        .ok();
        write!(xml, "    <rate>\n").ok();
        write!(xml, "      <timebase>{}</timebase>\n", timeline.fps).ok();
        write!(xml, "      <ntsc>FALSE</ntsc>\n").ok();
        write!(xml, "    </rate>\n").ok();
        write!(xml, "    <media>\n").ok();

        // Video tracks (XML-010: reversed order)
        let video_tracks: Vec<&Track> = timeline
            .tracks
            .iter()
            .filter(|t| t.r#type != ClipType::Audio)
            .collect();
        write!(xml, "      <video>\n").ok();
        for (i, track) in video_tracks.iter().rev().enumerate() {
            write!(xml, "        <track>\n").ok();
            write!(xml, "          <trackindex>{}</trackindex>\n", i + 1).ok();
            write!(xml, "          <videotrack>\n").ok();
            write!(
                xml,
                "            <enabled>{}</enabled>\n",
                if track.hidden { "FALSE" } else { "TRUE" }
            )
            .ok();
            write!(xml, "            <locked>FALSE</locked>\n").ok();

            for clip in &track.clips {
                if clip.media_type == ClipType::Text || clip.media_type == ClipType::Shape {
                    // XML-013: Text/shape overlays not preserved
                    continue;
                }
                let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
                write_clip(&mut xml, clip, timeline.fps, tc);
            }
            write!(xml, "          </videotrack>\n").ok();
            write!(xml, "        </track>\n").ok();
        }
        write!(xml, "      </video>\n").ok();

        // Audio tracks
        let audio_tracks: Vec<&Track> = timeline
            .tracks
            .iter()
            .filter(|t| t.r#type == ClipType::Audio)
            .collect();
        write!(xml, "      <audio>\n").ok();
        for (i, track) in audio_tracks.iter().enumerate() {
            write!(xml, "        <track>\n").ok();
            write!(xml, "          <trackindex>{}</trackindex>\n", i + 1).ok();
            write!(xml, "          <audiotrack>\n").ok();
            write!(
                xml,
                "            <enabled>{}</enabled>\n",
                if track.muted { "FALSE" } else { "TRUE" }
            )
            .ok();
            write!(xml, "            <locked>FALSE</locked>\n").ok();
            for clip in &track.clips {
                let tc = media_timecodes.and_then(|m| m.get(&clip.media_ref).copied());
                write_clip(&mut xml, clip, timeline.fps, tc);
            }
            write!(xml, "          </audiotrack>\n").ok();
            write!(xml, "        </track>\n").ok();
        }
        write!(xml, "      </audio>\n").ok();

        write!(xml, "    </media>\n").ok();
        write!(xml, "  </sequence>\n").ok();
        write!(xml, "</xmeml>\n").ok();
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
/// XML-002~008: preserves clip placement, trims, speed, volume, opacity, transform, crop, fades.
/// XML-011: emits one full <file> element per clip (simplified — no dedup since
///          we don't have media manifest here).
/// XML-012: clips without media_ref are skipped.
/// `timecode` is the optional SourceTimecode from the tmcd track. Upstream PR #136.
fn write_clip(xml: &mut String, clip: &Clip, fps: i64, timecode: Option<SourceTimecode>) {
    if clip.media_ref.is_empty() {
        // XML-012: skip unresolved media
        return;
    }

    write!(xml, "            <clipitem id=\"{}\">\n", clip.id).ok();
    write!(xml, "              <name>{}</name>\n", clip.media_ref).ok();
    write!(
        xml,
        "              <duration>{}</duration>\n",
        clip.duration_frames
    )
    .ok();
    write!(xml, "              <rate>\n").ok();
    write!(xml, "                <timebase>{}</timebase>\n", fps).ok();
    write!(xml, "                <ntsc>FALSE</ntsc>\n").ok();
    write!(xml, "              </rate>\n").ok();

    // XML-002: clip placement
    write!(xml, "              <start>{}</start>\n", clip.start_frame).ok();
    // XML-003: source trims
    write!(xml, "              <in>{}</in>\n", clip.trim_start_frame).ok();
    write!(xml, "              <out>{}</out>\n", clip.trim_end_frame).ok();
    // XML-004: speed changes
    if (clip.speed - 1.0).abs() > f64::EPSILON {
        write!(xml, "              <speed>\n").ok();
        write!(xml, "                <value>{:.3}</value>\n", clip.speed).ok();
        write!(xml, "                <timebase>{}</timebase>\n", fps).ok();
        write!(xml, "                <ntsc>FALSE</ntsc>\n").ok();
        write!(xml, "              </speed>\n").ok();
    }
    // XML-005: volume
    write!(xml, "              <volume>\n").ok();
    write!(xml, "                <value>{:.6}</value>\n", clip.volume).ok();
    write!(xml, "              </volume>\n").ok();
    // XML-005: opacity
    write!(
        xml,
        "              <opacity>{:.6}</opacity>\n",
        clip.opacity
    )
    .ok();
    // XML-007: fades
    if clip.fade_in_frames > 0 {
        write!(xml, "              <fadein>\n").ok();
        write!(
            xml,
            "                <duration>{}</duration>\n",
            clip.fade_in_frames
        )
        .ok();
        write!(xml, "              </fadein>\n").ok();
    }
    if clip.fade_out_frames > 0 {
        write!(xml, "              <fadeout>\n").ok();
        write!(
            xml,
            "                <duration>{}</duration>\n",
            clip.fade_out_frames
        )
        .ok();
        write!(xml, "              </fadeout>\n").ok();
    }
    // XML-006: transform and crop
    write!(xml, "              <filter>\n").ok();
    write!(xml, "                <effect>\n").ok();
    write!(xml, "                  <name>Basic Motion</name>\n").ok();
    write!(
        xml,
        "                  <effectcategory>motion</effectcategory>\n"
    )
    .ok();
    write!(xml, "                  <effecttype>motion</effecttype>\n").ok();
    write!(xml, "                  <parameter>\n").ok();
    write!(
        xml,
        "                    <parameterid>scale</parameterid>\n"
    )
    .ok();
    write!(
        xml,
        "                    <value>{:.6}</value>\n",
        clip.transform.width
    )
    .ok();
    write!(xml, "                  </parameter>\n").ok();
    write!(xml, "                  <parameter>\n").ok();
    write!(
        xml,
        "                    <parameterid>rotation</parameterid>\n"
    )
    .ok();
    write!(
        xml,
        "                    <value>{:.6}</value>\n",
        clip.transform.rotation
    )
    .ok();
    write!(xml, "                  </parameter>\n").ok();
    write!(xml, "                  <parameter>\n").ok();
    write!(
        xml,
        "                    <parameterid>center</parameterid>\n"
    )
    .ok();
    write!(
        xml,
        "                    <value>{:.6} {:.6}</value>\n",
        clip.transform.center_x, clip.transform.center_y
    )
    .ok();
    write!(xml, "                  </parameter>\n").ok();
    write!(xml, "                  <parameter>\n").ok();
    write!(xml, "                    <parameterid>crop</parameterid>\n").ok();
    write!(
        xml,
        "                    <value>{:.6} {:.6} {:.6} {:.6}</value>\n",
        clip.crop.left, clip.crop.top, clip.crop.right, clip.crop.bottom
    )
    .ok();
    write!(xml, "                  </parameter>\n").ok();
    write!(xml, "                </effect>\n").ok();
    write!(xml, "              </filter>\n").ok();

    // XML-008: linked clip relationships
    if let Some(ref link_id) = clip.link_group_id {
        write!(xml, "              <link>\n").ok();
        write!(
            xml,
            "                <linkclipref>{}</linkclipref>\n",
            link_id
        )
        .ok();
        write!(xml, "                <medialink>true</medialink>\n").ok();
        write!(xml, "              </link>\n").ok();
    }

    // File reference (XML-011)
    write!(xml, "              <file>\n").ok();
    write!(xml, "                <name>{}</name>\n", clip.media_ref).ok();
    write!(
        xml,
        "                <pathurl>{}</pathurl>\n",
        clip.media_ref
    )
    .ok();
    write!(xml, "                <rate>\n").ok();
    write!(xml, "                  <timebase>{}</timebase>\n", fps).ok();
    write!(xml, "                </rate>\n").ok();
    // XML-009: source timecode (PR #136)
    let ntsc = fps % 30 == 0 && fps <= 60;
    let tc = timecode_tags(timecode, fps, ntsc);
    write!(xml, "                <timecode>\n").ok();
    write!(xml, "                  <rate>\n").ok();
    write!(xml, "                    <timebase>{}</timebase>\n", tc.0).ok();
    write!(
        xml,
        "                    <ntsc>{}</ntsc>\n",
        if tc.1 { "TRUE" } else { "FALSE" }
    )
    .ok();
    write!(xml, "                  </rate>\n").ok();
    write!(xml, "                  <string>{}</string>\n", tc.4).ok();
    write!(xml, "                  <frame>{}</frame>\n", tc.2).ok();
    write!(
        xml,
        "                  <displayformat>{}</displayformat>\n",
        if tc.3 { "DF" } else { "NDF" }
    )
    .ok();
    write!(xml, "                </timecode>\n").ok();
    write!(xml, "              </file>\n").ok();

    write!(xml, "            </clipitem>\n").ok();
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
                }],
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            compound_timelines: std::collections::HashMap::new(),
        }
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
        assert_eq!(tc.1, true); // ntsc (still NTSC even though NDF)
        assert_eq!(tc.2, 1968620); // frame
        assert_eq!(tc.3, false); // drop_frame
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
        assert_eq!(tc.3, true); // drop frame
        assert_eq!(tc.4, "00;23;53;18");
    }

    #[test]
    fn timecode_tags_no_source_falls_back() {
        let tc = timecode_tags(None, 30, true);
        assert_eq!(tc.0, 30);
        assert_eq!(tc.2, 0);
        assert_eq!(tc.3, true); // NTSC 30 → drop frame guess
        assert_eq!(tc.4, "00;00;00;00");
    }

    #[test]
    fn timecode_tags_no_source_non_ntsc() {
        let tc = timecode_tags(None, 30, false);
        assert_eq!(tc.3, false);
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
        let xml = XmlExport::export_with_timecodes(&timeline, Some(&map));
        assert!(xml.contains("<timecode>"));
        assert!(xml.contains("<string>00:00:03:10</string>")); // frame 100 at 30fps
        assert!(xml.contains("<displayformat>NDF</displayformat>"));
    }
}
