//! Mention system for the agent chat (MNT-001 to MNT-010).
//!
//! Pure functions for mention creation, normalization, deduplication,
//! pruning, and context packing.

use core_model::{AgentMention, AgentTimelineRangeMention, Clip};
use std::collections::HashSet;
use uuid::Uuid;

/// MNT-002: Normalize a display name by replacing whitespace and hyphens
/// with a compact dash-separated form.
pub fn normalize_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut prev_was_sep = false;

    for c in name.chars() {
        if c.is_whitespace() || c == '-' || c == '_' {
            if !prev_was_sep {
                result.push('-');
                prev_was_sep = true;
            }
        } else {
            result.push(c);
            prev_was_sep = false;
        }
    }

    result
}

/// Create a media asset mention.
///
/// MNT-001: media asset mention kind.
pub fn media_mention(asset_name: &str, media_id: &str) -> AgentMention {
    AgentMention {
        id: Uuid::new_v4(),
        display_name: normalize_name(asset_name),
        media_ref: Some(media_id.to_string()),
        r#type: None,
        clip_id: None,
        timeline_range: None,
    }
}

/// Create a clip mention with compact label.
///
/// MNT-001: timeline clip mention kind.
/// MNT-004: includes compact clip label and start timecode.
pub fn clip_mention(clip: &Clip, track_label: &str, fps: i64) -> AgentMention {
    let start_timecode = frames_to_timecode(clip.start_frame, fps);
    let display_name = format!("{}-{}-{}", clip.id, track_label, start_timecode);

    AgentMention {
        id: Uuid::new_v4(),
        display_name: normalize_name(&display_name),
        media_ref: Some(clip.media_ref.clone()),
        r#type: Some(clip.media_type.clone()),
        clip_id: Some(clip.id.clone()),
        timeline_range: None,
    }
}

/// Create a timeline range mention.
///
/// MNT-001: timeline range mention kind.
/// MNT-005: half-open semantics (start inclusive, end exclusive).
pub fn range_mention(start_frame: i64, end_frame: i64, fps: i64) -> AgentMention {
    let duration_frames = end_frame - start_frame;
    AgentMention {
        id: Uuid::new_v4(),
        display_name: format!("range-{}-{}", start_frame, end_frame),
        media_ref: None,
        r#type: None,
        clip_id: None,
        timeline_range: Some(AgentTimelineRangeMention {
            start_frame,
            end_frame,
            duration_frames,
            fps,
            start_timecode: frames_to_timecode(start_frame, fps),
            end_timecode: frames_to_timecode(end_frame, fps),
            duration_timecode: frames_to_timecode(duration_frames, fps),
            range_semantics: "half-open".to_string(),
        }),
    }
}

/// MNT-003: Disambiguate asset mentions with matching display names by
/// appending a short id suffix.
pub fn disambiguate_mentions(mentions: &mut [AgentMention]) {
    // Collect groups by display_name (clone the names to avoid borrowing conflicts)
    let names: Vec<String> = mentions.iter().map(|m| m.display_name.clone()).collect();
    let mut groups: std::collections::BTreeMap<String, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (i, name) in names.iter().enumerate() {
        groups.entry(name.clone()).or_default().push(i);
    }

    for indices in groups.values() {
        if indices.len() <= 1 {
            continue;
        }
        for &i in indices {
            let suffix = mentions[i]
                .media_ref
                .as_ref()
                .map(|m| {
                    if m.len() >= 4 {
                        m[m.len().saturating_sub(4)..].to_string()
                    } else {
                        m.clone()
                    }
                })
                .unwrap_or_default();
            if !suffix.is_empty() {
                mentions[i].display_name = format!("{}-{}", mentions[i].display_name, suffix);
            }
        }
    }
}

/// MNT-006: Deduplicate mentions of the same asset/clip/range.
pub fn deduplicate_mentions(mentions: Vec<AgentMention>) -> Vec<AgentMention> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for mention in mentions {
        let key = mention_key(&mention);
        if seen.insert(key) {
            result.push(mention);
        }
    }

    result
}

fn mention_key(mention: &AgentMention) -> (Option<String>, Option<String>, Option<(i64, i64)>) {
    let media = mention.media_ref.clone();
    let clip = mention.clip_id.clone();
    let range = mention
        .timeline_range
        .as_ref()
        .map(|r| (r.start_frame, r.end_frame));
    (media, clip, range)
}

/// MNT-007: Remove mentions that are no longer referenced in the draft text.
pub fn prune_mentions(text: &str, mentions: Vec<AgentMention>) -> Vec<AgentMention> {
    mentions
        .into_iter()
        .filter(|m| text.contains(&m.display_name))
        .collect()
}

/// MNT-008: Filter mentions to only those still referenced in the final
/// outgoing message.
pub fn pack_referenced_mentions(text: &str, mentions: Vec<AgentMention>) -> Vec<AgentMention> {
    prune_mentions(text, mentions)
}

// ---------------------------------------------------------------------------
// MNT-009 / MNT-010: Image inlining
// ---------------------------------------------------------------------------

/// Result of attempting to inline an image mention.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageInlineResult {
    /// Successfully inlined with image data.
    Inlined { media_ref: String, data_url: String },
    /// Could not read the image.
    Failed { media_ref: String, reason: String },
}

/// Try to inline an image asset mention.
///
/// Returns an `ImageInlineResult`: on success the image data URL, on failure
/// a message describing why. The actual image reading is done via callback to
/// keep this pure logic.
pub fn try_inline_image(
    media_ref: &str,
    read_image_data: impl FnOnce(&str) -> Option<String>,
) -> ImageInlineResult {
    match read_image_data(media_ref) {
        Some(data_url) => ImageInlineResult::Inlined {
            media_ref: media_ref.to_string(),
            data_url,
        },
        None => ImageInlineResult::Failed {
            media_ref: media_ref.to_string(),
            reason: format!("Could not read image for media: {media_ref}"),
        },
    }
}

/// Build context message for image mentions in a conversation.
///
/// Inlines images when possible, falls back to text description on failure.
/// Returns a tuple of (inlined_results, failed_descriptions).
pub fn pack_image_mentions(
    image_mentions: &[String],
    read_image: impl Fn(&str) -> Option<String>,
) -> (Vec<ImageInlineResult>, Vec<String>) {
    let mut inlined = Vec::new();
    let mut failed = Vec::new();

    for mention in image_mentions {
        match try_inline_image(mention, &read_image) {
            ok @ ImageInlineResult::Inlined { .. } => {
                inlined.push(ok);
            }
            ImageInlineResult::Failed { ref media_ref, .. } => {
                let desc = format!("Image could not be read: {media_ref}");
                failed.push(desc);
            }
        }
    }

    (inlined, failed)
}

/// Convert frames to a timecode string (HH:MM:SS:FF).
fn frames_to_timecode(frames: i64, fps: i64) -> String {
    if fps <= 0 {
        return "00:00:00:00".to_string();
    }
    let total_seconds = frames / fps;
    let remaining_frames = frames % fps;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!(
        "{:02}:{:02}:{:02}:{:02}",
        hours, minutes, seconds, remaining_frames
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::ClipType;

    #[test]
    fn mnt_001_three_mention_kinds() {
        let media = media_mention("beach-video.mp4", "mid-001");
        assert!(media.media_ref.is_some(), "MNT-001: media mention has ref");
        assert!(media.clip_id.is_none());
        assert!(media.timeline_range.is_none());

        let clip_data = Clip {
            id: "clip-001".into(),
            media_ref: "mid-001".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 100,
            duration_frames: 50,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
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
        };
        let clip_m = clip_mention(&clip_data, "V1", 30);
        assert!(
            clip_m.clip_id.is_some(),
            "MNT-001: clip mention has clip_id"
        );
        assert_eq!(clip_m.clip_id.unwrap(), "clip-001");

        let range = range_mention(0, 100, 30);
        assert!(
            range.timeline_range.is_some(),
            "MNT-001: range mention has timeline_range"
        );
        assert_eq!(range.timeline_range.unwrap().start_frame, 0);
    }

    #[test]
    fn mnt_002_normalize_whitespace_and_hyphens() {
        assert_eq!(normalize_name("beach video.mp4"), "beach-video.mp4");
        assert_eq!(normalize_name("my--clip   name"), "my-clip-name");
        assert_eq!(normalize_name("simple"), "simple");
        assert_eq!(normalize_name("  leading trailing  "), "-leading-trailing-");
    }

    #[test]
    fn mnt_003_disambiguate_collisions() {
        let mut mentions = vec![
            media_mention("beach.mp4", "aaaaaaaa-0001"),
            media_mention("beach.mp4", "aaaaaaaa-0002"),
            media_mention("sunset.mp4", "bbbbbbbb-0001"),
        ];
        disambiguate_mentions(&mut mentions);
        // First two have same display_name - should be disambiguated
        assert_ne!(
            mentions[0].display_name, mentions[1].display_name,
            "MNT-003: colliding names disambiguated"
        );
        // Third is unique - should not change
        assert_eq!(mentions[2].display_name, "sunset.mp4");
    }

    #[test]
    fn mnt_005_half_open_semantics() {
        let range = range_mention(10, 20, 30);
        let r = range.timeline_range.unwrap();
        assert_eq!(r.start_frame, 10);
        assert_eq!(r.end_frame, 20);
        assert_eq!(r.duration_frames, 10);
        assert_eq!(r.range_semantics, "half-open");
    }

    #[test]
    fn mnt_006_deduplicate_duplicates() {
        let m1 = media_mention("beach.mp4", "mid-001");
        let m2 = media_mention("beach.mp4", "mid-001");
        let m3 = media_mention("sunset.mp4", "mid-002");
        let result = deduplicate_mentions(vec![m1, m2, m3]);
        assert_eq!(result.len(), 2, "MNT-006: duplicates removed");
    }

    #[test]
    fn mnt_007_prune_removed_mentions() {
        let m1 = media_mention("beach.mp4", "mid-001");
        let m2 = media_mention("sunset.mp4", "mid-002");
        let text = "Check out beach.mp4";
        let pruned = prune_mentions(text, vec![m1, m2]);
        assert_eq!(pruned.len(), 1, "MNT-007: only beach.mp4 kept");
        assert_eq!(pruned[0].display_name, "beach.mp4");
    }

    #[test]
    fn mnt_008_pack_referenced_mentions() {
        let m1 = media_mention("clip-A.mp4", "mid-001");
        let m2 = media_mention("clip-B.mp4", "mid-002");
        let text = "Edit clip-A.mp4";
        let packed = pack_referenced_mentions(text, vec![m1, m2]);
        assert_eq!(packed.len(), 1);
    }

    #[test]
    fn frames_to_timecode_standard() {
        assert_eq!(frames_to_timecode(0, 30), "00:00:00:00");
        assert_eq!(frames_to_timecode(30, 30), "00:00:01:00");
        assert_eq!(frames_to_timecode(35, 30), "00:00:01:05");
        assert_eq!(frames_to_timecode(900, 30), "00:00:30:00");
        assert_eq!(frames_to_timecode(1800, 30), "00:01:00:00");
        assert_eq!(frames_to_timecode(108000, 30), "01:00:00:00");
    }

    #[test]
    fn frames_to_timecode_zero_fps() {
        assert_eq!(frames_to_timecode(100, 0), "00:00:00:00");
    }

    // ── MNT-009 / MNT-010: Image inlining ────────────────────────────

    #[test]
    fn mnt_009_try_inline_image_success() {
        let result = try_inline_image("img-001", |ref_| {
            assert_eq!(ref_, "img-001");
            Some("data:image/png;base64,abc123".to_string())
        });
        match result {
            ImageInlineResult::Inlined {
                media_ref,
                data_url,
            } => {
                assert_eq!(media_ref, "img-001");
                assert_eq!(data_url, "data:image/png;base64,abc123");
            }
            _ => panic!("Expected Inlined, got {:?}", result),
        }
    }

    #[test]
    fn mnt_009_try_inline_image_failure() {
        let result = try_inline_image("img-002", |_| None);
        match result {
            ImageInlineResult::Failed { media_ref, reason } => {
                assert_eq!(media_ref, "img-002");
                assert!(reason.contains("Could not read image"));
            }
            _ => panic!("Expected Failed, got {:?}", result),
        }
    }

    #[test]
    fn mnt_010_pack_image_mentions_separates_success_and_failure() {
        let mentions = vec!["good-img".to_string(), "bad-img".to_string()];
        let (inlined, failed) = pack_image_mentions(&mentions, |ref_| match ref_ {
            "good-img" => Some("data:image/png;base64,good".to_string()),
            _ => None,
        });
        assert_eq!(inlined.len(), 1);
        assert_eq!(failed.len(), 1);
        assert!(failed[0].contains("bad-img"));
    }

    #[test]
    fn mnt_010_pack_image_mentions_empty_input() {
        let (inlined, failed) = pack_image_mentions(&[], |_| unreachable!());
        assert!(inlined.is_empty());
        assert!(failed.is_empty());
    }
}
