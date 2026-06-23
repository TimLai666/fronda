use core_model::{ClipType, Interpolation, Timeline};
use serde::{Deserialize, Serialize};

/// Describes how a single clip should be rendered in the composition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionClip {
    /// Reference to the clip id
    pub clip_id: String,
    /// The underlying media type
    pub media_type: ClipType,
    /// Timeline frame where this clip starts in the composition
    pub composition_start: i64,
    /// Duration in composition frames
    pub duration_frames: i64,
    /// Source trim start in source frames
    pub source_trim_start: i64,
    /// Source trim end in source frames
    pub source_trim_end: i64,
    /// Playback speed
    pub speed: f64,
    /// Volume level (0.0 = silent, 1.0 = original)
    pub volume: f64,
    /// Opacity (0.0 = transparent, 1.0 = opaque)
    pub opacity: f64,
    /// Whether this clip is a text overlay (rendered via overlay path)
    pub is_text_overlay: bool,
    /// Whether this clip is an image (needs synthetic video generation)
    pub is_image: bool,
    /// Whether this clip is a Lottie asset
    pub is_lottie: bool,
    /// Fade in frames
    pub fade_in_frames: i64,
    /// Fade out frames
    pub fade_out_frames: i64,
    /// Fade in interpolation
    pub fade_in_interpolation: Interpolation,
    /// Fade out interpolation
    pub fade_out_interpolation: Interpolation,
}

/// A composition track maps to a timeline track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionTrack {
    /// The timeline track index
    pub timeline_track_index: usize,
    /// Whether this is a visual track
    pub is_visual: bool,
    /// Whether this track is hidden/muted
    pub is_hidden: bool,
    pub is_muted: bool,
    /// Clips assigned to this composition track
    pub clips: Vec<CompositionClip>,
}

/// The video resolution for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RenderResolution {
    pub width: u64,
    pub height: u64,
}

impl RenderResolution {
    /// Render at the timeline's native resolution.
    pub fn native(timeline: &Timeline) -> Self {
        Self {
            width: timeline.width.max(2) as u64,
            height: timeline.height.max(2) as u64,
        }
    }

    /// Scale to fit within the given short-side target while preserving aspect ratio.
    /// Always produces even dimensions >= 2.
    pub fn scale_to_short_side(
        canvas_width: u64,
        canvas_height: u64,
        short_side_target: u64,
    ) -> Self {
        let short_side = canvas_width.min(canvas_height);
        if short_side == 0 {
            return Self {
                width: 2,
                height: 2,
            };
        }
        let ratio = (short_side_target as f64) / (short_side as f64);
        let w = (canvas_width as f64 * ratio).round() as u64;
        let h = (canvas_height as f64 * ratio).round() as u64;
        Self {
            width: (w.max(2) / 2) * 2,
            height: (h.max(2) / 2) * 2,
        }
    }
}

/// All supported output resolutions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportResolution {
    R720p,
    R1080p,
    R1440p, // 2K
    R4K,
    MatchTimeline,
}

impl ExportResolution {
    pub fn render_size(self, timeline: &Timeline) -> RenderResolution {
        match self {
            ExportResolution::MatchTimeline => RenderResolution::native(timeline),
            ExportResolution::R720p => RenderResolution::scale_to_short_side(
                timeline.width as u64,
                timeline.height as u64,
                720,
            ),
            ExportResolution::R1080p => RenderResolution::scale_to_short_side(
                timeline.width as u64,
                timeline.height as u64,
                1080,
            ),
            ExportResolution::R1440p => RenderResolution::scale_to_short_side(
                timeline.width as u64,
                timeline.height as u64,
                1440,
            ),
            ExportResolution::R4K => RenderResolution::scale_to_short_side(
                timeline.width as u64,
                timeline.height as u64,
                2160,
            ),
        }
    }
}

/// Supported export container/codec formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    H264,
    H265,
    ProRes,
}

impl ExportFormat {
    pub fn file_extension(self) -> &'static str {
        match self {
            ExportFormat::H264 | ExportFormat::H265 => "mp4",
            ExportFormat::ProRes => "mov",
        }
    }
}

/// The full composition plan for rendering or exporting a timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompositionPlan {
    pub resolution: RenderResolution,
    pub fps: i64,
    pub total_frames: i64,
    pub tracks: Vec<CompositionTrack>,
    pub offline_media_refs: Vec<String>,
    pub unprocessable_media_refs: Vec<String>,
}

impl CompositionPlan {
    /// Build a composition plan from a timeline at the given resolution.
    /// This is a pure data transformation — no platform APIs involved.
    pub fn from_timeline(timeline: &Timeline, resolution: RenderResolution) -> Self {
        let tracks: Vec<CompositionTrack> = timeline
            .tracks
            .iter()
            .enumerate()
            .map(|(ti, track)| {
                let is_visual = track.r#type != ClipType::Audio;
                let clips: Vec<CompositionClip> = track
                    .clips
                    .iter()
                    .map(|clip| CompositionClip {
                        clip_id: clip.id.clone(),
                        media_type: clip.media_type.clone(),
                        composition_start: clip.start_frame,
                        duration_frames: clip.duration_frames,
                        source_trim_start: clip.trim_start_frame,
                        source_trim_end: clip.trim_end_frame,
                        speed: clip.speed,
                        volume: clip.volume,
                        opacity: clip.opacity,
                        is_text_overlay: clip.media_type == ClipType::Text,
                        is_image: clip.media_type == ClipType::Image,
                        is_lottie: clip.media_type == ClipType::Lottie,
                        fade_in_frames: clip.fade_in_frames,
                        fade_out_frames: clip.fade_out_frames,
                        fade_in_interpolation: clip.fade_in_interpolation,
                        fade_out_interpolation: clip.fade_out_interpolation,
                    })
                    .collect();
                CompositionTrack {
                    timeline_track_index: ti,
                    is_visual,
                    is_hidden: track.hidden,
                    is_muted: track.muted,
                    clips,
                }
            })
            .collect();

        // Sort visual tracks top-to-bottom, audio tracks below
        let tracks = {
            let mut sorted = tracks;
            sorted.sort_by_key(|t| !t.is_visual);
            sorted
        };

        // Collect offline/unprocessable media (empty for now — platform adapter fills this)
        let total_frames = timeline
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.start_frame + c.duration_frames)
            .max()
            .unwrap_or(0);

        CompositionPlan {
            resolution,
            fps: timeline.fps,
            total_frames,
            tracks,
            offline_media_refs: Vec::new(),
            unprocessable_media_refs: Vec::new(),
        }
    }

    /// Estimate the output bitrate in bps based on resolution and format.
    /// Uses a megapixel-based heuristic, independent of any specific encoder.
    pub fn estimated_bitrate(&self, format: ExportFormat) -> u64 {
        let megapixels =
            (self.resolution.width as f64 * self.resolution.height as f64) / 1_000_000.0;
        let base = match format {
            ExportFormat::H264 => 8_000_000,
            ExportFormat::H265 => 5_000_000,
            ExportFormat::ProRes => 30_000_000,
        };
        (base as f64 * megapixels / 2.0).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_timeline() -> Timeline {
        use core_model::{Clip, ClipType, Crop, Interpolation, Track, Transform};
        let v1 = Clip {
            id: "v1".into(),
            media_ref: "asset-v".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 100,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
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
        };
        let a1 = Clip {
            id: "a1".into(),
            media_ref: "asset-a".into(),
            media_type: ClipType::Audio,
            source_clip_type: ClipType::Audio,
            start_frame: 0,
            duration_frames: 100,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 0.8,
            opacity: 1.0,
            fade_in_frames: 5,
            fade_out_frames: 10,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: Some("g1".into()),
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
        };
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            tracks: vec![
                Track {
                    id: "v-track".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: true,
                    clips: vec![v1],
                },
                Track {
                    id: "a-track".into(),
                    r#type: ClipType::Audio,
                    muted: false,
                    hidden: true,
                    sync_locked: true,
                    clips: vec![a1],
                },
            ],
        }
    }

    #[test]
    fn composition_plan_builds_from_timeline() {
        let timeline = make_timeline();
        let resolution = RenderResolution::native(&timeline);
        let plan = CompositionPlan::from_timeline(&timeline, resolution);

        assert_eq!(plan.total_frames, 100);
        assert_eq!(plan.fps, 30);
        assert_eq!(plan.resolution.width, 1920);
        assert_eq!(plan.resolution.height, 1080);
    }

    #[test]
    fn composition_plan_sorts_visual_above_audio() {
        let timeline = make_timeline();
        let resolution = RenderResolution::native(&timeline);
        let plan = CompositionPlan::from_timeline(&timeline, resolution);

        assert!(plan.tracks[0].is_visual, "first track should be visual");
        assert!(!plan.tracks[1].is_visual, "second track should be audio");
    }

    #[test]
    fn composition_plan_preserves_track_state() {
        let timeline = make_timeline();
        let resolution = RenderResolution::native(&timeline);
        let plan = CompositionPlan::from_timeline(&timeline, resolution);

        assert!(!plan.tracks[0].is_hidden);
        assert!(plan.tracks[1].is_hidden);
        assert!(!plan.tracks[1].is_muted);
    }

    #[test]
    fn composition_plan_clip_properties() {
        let timeline = make_timeline();
        let resolution = RenderResolution::native(&timeline);
        let plan = CompositionPlan::from_timeline(&timeline, resolution);

        let v_clip = &plan.tracks[0].clips[0];
        assert_eq!(v_clip.clip_id, "v1");
        assert_eq!(v_clip.composition_start, 0);
        assert_eq!(v_clip.duration_frames, 100);
        assert!(!v_clip.is_text_overlay);
        assert!(!v_clip.is_image);

        let a_clip = &plan.tracks[1].clips[0];
        assert!((a_clip.volume - 0.8).abs() < 0.001);
        assert_eq!(a_clip.fade_in_frames, 5);
        assert_eq!(a_clip.fade_out_frames, 10);
    }

    #[test]
    fn render_resolution_scale_to_short_side_720p() {
        let size = RenderResolution::scale_to_short_side(1920, 1080, 720);
        assert_eq!(size.width, 1280);
        assert_eq!(size.height, 720);
    }

    #[test]
    fn render_resolution_scale_to_short_side_4k() {
        let size = RenderResolution::scale_to_short_side(1920, 1080, 2160);
        assert_eq!(size.width, 3840);
        assert_eq!(size.height, 2160);
    }

    #[test]
    fn render_resolution_scale_to_short_side_even_enforced() {
        let size = RenderResolution::scale_to_short_side(1921, 1081, 720);
        assert_eq!(size.width % 2, 0);
        assert_eq!(size.height % 2, 0);
    }

    #[test]
    fn render_resolution_scale_to_short_side_zero_protection() {
        let size = RenderResolution::scale_to_short_side(0, 0, 720);
        assert_eq!(size.width, 2);
        assert_eq!(size.height, 2);
    }

    #[test]
    fn render_resolution_scale_to_short_side_upscales_tiny_canvas() {
        let size = RenderResolution::scale_to_short_side(1, 1, 720);
        assert_eq!(size.width, 720);
        assert_eq!(size.height, 720);
    }

    #[test]
    fn export_resolution_720p() {
        let timeline = make_timeline();
        let size = ExportResolution::R720p.render_size(&timeline);
        assert_eq!(size.width, 1280);
        assert_eq!(size.height, 720);
    }

    #[test]
    fn export_resolution_match_timeline() {
        let timeline = make_timeline();
        let size = ExportResolution::MatchTimeline.render_size(&timeline);
        assert_eq!(size.width, 1920);
        assert_eq!(size.height, 1080);
    }

    #[test]
    fn estimate_bitrate_prores_higher_than_h264() {
        let timeline = make_timeline();
        let resolution = RenderResolution::native(&timeline);
        let plan = CompositionPlan::from_timeline(&timeline, resolution);
        assert!(
            plan.estimated_bitrate(ExportFormat::ProRes)
                > plan.estimated_bitrate(ExportFormat::H264)
        );
    }

    #[test]
    fn export_format_extension() {
        assert_eq!(ExportFormat::H264.file_extension(), "mp4");
        assert_eq!(ExportFormat::ProRes.file_extension(), "mov");
    }
}
