//! Composite a single timeline frame to a PNG for the preview canvas.
//!
//! gpui renders images from file paths (`gpui::img(path)`), so the preview shows
//! the real composited frame by writing it to a cache PNG and pointing `img` at
//! it. Compositing + decode reuse the pure `render_core` compositor and the
//! `video_export` decode helpers, so this is testable against a real fixture.

use crate::video_export::{decode_frame_rgba, source_path, source_time_seconds};
use core_model::{MediaManifest, Timeline};
use render_core::compositor::compose_frame;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Composite `frame` of `timeline` at its project dimensions and write it as a
/// PNG to `out`. Sources resolve from `manifest` (project-relative against
/// `project_root`); each clip's source frame is decoded at its mapped time.
pub fn render_frame_png(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_root: &Path,
    frame: i64,
    out: &Path,
) -> Result<(), String> {
    let w = timeline.width.max(1) as usize;
    let h = timeline.height.max(1) as usize;
    let fps = timeline.fps;

    let paths: HashMap<&str, PathBuf> = manifest
        .entries
        .iter()
        .filter_map(|e| source_path(e, project_root).map(|p| (e.id.as_str(), p)))
        .collect();

    let img = compose_frame(timeline, manifest, frame, w, h, |clip| {
        let path = paths.get(clip.media_ref.as_str())?;
        decode_frame_rgba(path, source_time_seconds(clip, frame, fps))
    });

    let buf = image::RgbaImage::from_raw(w as u32, h as u32, img.pixels)
        .ok_or("composited frame buffer size mismatch")?;
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    buf.save_with_format(out, image::ImageFormat::Png)
        .map_err(|e| format!("write preview png: {e}"))
}

/// Cache path for a rendered preview frame, keyed by project revision + frame so
/// a changed project or playhead produces a distinct file.
pub fn preview_cache_path(cache_dir: &Path, revision: u64, frame: i64) -> PathBuf {
    cache_dir.join(format!("preview-{revision}-{frame}.png"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{
        Clip, ClipType, Crop, Interpolation, MediaManifestEntry, MediaSource, Track, Transform,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("fronda-preview-render-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn full_frame_clip(media_ref: &str) -> Clip {
        Clip {
            id: "c1".into(),
            media_ref: media_ref.into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 3,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: Transform::from_top_left(0.0, 0.0, 1.0, 1.0),
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

    fn timeline_with(clip: Clip) -> Timeline {
        Timeline {
            fps: 15,
            width: 64,
            height: 48,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips: vec![clip],
            }],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            compound_timelines: Default::default(),
        }
    }

    #[test]
    fn cache_path_is_revision_and_frame_keyed() {
        let dir = Path::new("/cache");
        assert_ne!(preview_cache_path(dir, 1, 0), preview_cache_path(dir, 1, 5));
        assert_ne!(preview_cache_path(dir, 1, 0), preview_cache_path(dir, 2, 0));
    }

    #[test]
    fn renders_fixture_frame_to_png() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
        assert!(fixture.is_file(), "fixture missing: {}", fixture.display());

        let clip = full_frame_clip("m1");
        let timeline = timeline_with(clip);
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "m1".into(),
            name: "m1".into(),
            r#type: ClipType::Video,
            source: MediaSource::External {
                absolute_path: fixture.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
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

        let dir = temp_dir("fixture");
        let out = dir.join("frame0.png");
        render_frame_png(&timeline, &manifest, &dir, 0, &out).expect("fixture frame should render");

        let bytes = std::fs::read(&out).unwrap();
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[1..4], b"PNG");
        let decoded = image::open(&out).unwrap();
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 48);
    }

    #[test]
    fn empty_timeline_renders_transparent_png() {
        let dir = temp_dir("empty");
        let out = dir.join("empty.png");
        let timeline = Timeline {
            fps: 30,
            width: 16,
            height: 16,
            tracks: vec![],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            compound_timelines: Default::default(),
        };
        render_frame_png(&timeline, &MediaManifest::default(), &dir, 0, &out).unwrap();
        let decoded = image::open(&out).unwrap();
        assert_eq!(decoded.width(), 16);
        assert_eq!(decoded.height(), 16);
    }
}
