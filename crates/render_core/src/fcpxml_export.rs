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

impl FcpxmlExport {
    /// Generate an FCPXML 1.10 document for `timeline`, resolving asset paths and
    /// source durations from `manifest`.
    pub fn export(timeline: &Timeline, manifest: &MediaManifest) -> String {
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
        let redundant_audio = redundant_audio_clip_ids(timeline);

        for (ti, track) in timeline.tracks.iter().enumerate() {
            let lane = lane_of_track[ti];
            for clip in &track.clips {
                if clip.media_ref.is_empty()
                    || clip.media_type == ClipType::Text
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
                // #247: the in-point reads from the asset's timecode origin, so the source
                // frame is offset by the asset's embedded start timecode. (No retiming in this
                // exporter, so origin lands directly on `start`; a retimed clip would carry it
                // in a timeMap instead.)
                let origin = start_timecode_frames(manifest.entry_for(&clip.media_ref), fps);
                writeln!(
                    xml,
                    "              <asset-clip ref=\"{ref_id}\" lane=\"{lane}\" offset=\"{}\" name=\"{}\" duration=\"{}\" start=\"{}\"{format_attr}/>",
                    time_str(clip.start_frame, fps),
                    xml_escape(&display_name(manifest, &clip.media_ref)),
                    time_str(clip.duration_frames.max(1), fps),
                    time_str(origin + clip.trim_start_frame.max(0), fps),
                )
                .ok();
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
fn redundant_audio_clip_ids(timeline: &Timeline) -> std::collections::HashSet<String> {
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
        }
    }
    redundant
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
    let name = display_name(manifest, media_ref);
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
            clips,
        }
    }

    fn timeline(tracks: Vec<Track>) -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks,
            transcription_language: None,
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
