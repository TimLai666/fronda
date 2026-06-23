use core_model::{
    AgentContentBlock, ChatSession, Clip, ClipType, GenerationLog, Interpolation, MediaManifest,
    MediaSource, ShapeKind, ShapeStyle, TextStyle, Timeline, ToolResultBlock,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

fn fixture_path(bundle: &str, relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rust-rewrite/projects")
        .join(bundle)
        .join(relative_path)
}

fn read_fixture_json<T: DeserializeOwned>(bundle: &str, relative_path: &str) -> T {
    let path = fixture_path(bundle, relative_path);
    let bytes = fs::read(path).expect("fixture should exist");
    serde_json::from_slice(&bytes).expect("fixture JSON should decode")
}

fn approx_eq(left: f64, right: f64) {
    assert!((left - right).abs() < 1e-9, "left={left} right={right}");
}

#[test]
fn modern_timeline_fixture_decodes_rich_contracts() {
    let timeline: Timeline = read_fixture_json("modern-rich.palmier", "project.json");

    assert_eq!(timeline.fps, 30);
    assert_eq!(timeline.tracks.len(), 2);

    let video_track = &timeline.tracks[0];
    assert_eq!(video_track.r#type, ClipType::Video);
    assert_eq!(video_track.clips.len(), 2);

    let video_clip = &video_track.clips[0];
    assert_eq!(video_clip.media_type, ClipType::Video);
    assert_eq!(video_clip.source_clip_type, ClipType::Video);
    assert_eq!(video_clip.trim_start_frame, 15);
    assert_eq!(video_clip.trim_end_frame, 5);
    assert_eq!(video_clip.fade_out_interpolation, Interpolation::Smooth);
    approx_eq(video_clip.transform.center_x, 0.48);
    approx_eq(video_clip.crop.left, 0.05);
    assert_eq!(
        video_clip.rotation_track.as_ref().unwrap().keyframes[0].interpolation_out,
        Interpolation::Hold
    );
    assert_eq!(
        video_clip.position_track.as_ref().unwrap().keyframes[1].frame,
        60
    );

    let text_clip = &video_track.clips[1];
    assert_eq!(text_clip.media_type, ClipType::Text);
    assert_eq!(text_clip.text_content.as_deref(), Some("Fronda"));
    approx_eq(text_clip.text_style.as_ref().unwrap().font_scale, 0.85);

    let audio_track = &timeline.tracks[1];
    let audio_clip = &audio_track.clips[0];
    assert_eq!(audio_clip.media_type, ClipType::Audio);
    assert_eq!(audio_clip.volume_track.as_ref().unwrap().keyframes.len(), 2);
}

#[test]
fn legacy_track_and_clip_defaults_decode_like_swift() {
    let timeline: Timeline = read_fixture_json("legacy-defaults.palmier", "project.json");

    let track = &timeline.tracks[0];
    assert!(!track.muted);
    assert!(!track.hidden);
    assert!(track.sync_locked);

    let clip = &track.clips[0];
    assert_eq!(clip.media_type, ClipType::Video);
    assert_eq!(clip.source_clip_type, ClipType::Video);
    assert_eq!(clip.speed, 1.0);
    assert_eq!(clip.volume, 1.0);
    assert_eq!(clip.opacity, 1.0);
    assert_eq!(clip.fade_in_frames, 0);
    assert_eq!(clip.fade_in_interpolation, Interpolation::Linear);
    approx_eq(clip.transform.center_x, 0.1);
    approx_eq(clip.transform.center_y, 0.05);
}

#[test]
fn legacy_text_style_missing_font_scale_defaults_to_one() {
    let timeline: Timeline = read_fixture_json("legacy-defaults.palmier", "project.json");
    let style = timeline.tracks[0].clips[1].text_style.as_ref().unwrap();

    assert_eq!(style.font_scale, 1.0);
}

#[test]
fn upstream_065_font_weight_defaults_and_round_trips() {
    // Upstream PR #65: font_weight defaults to 400, round-trips through JSON.
    let encoded = json!({
        "fontName": "Helvetica",
        "fontWeight": 700
    });
    let style: TextStyle = serde_json::from_value(encoded).unwrap();
    approx_eq(style.font_weight, 700.0);
    assert_eq!(style.font_name, "Helvetica");

    // Missing font_weight defaults to 400
    let encoded_no_weight = json!({
        "fontName": "Helvetica"
    });
    let style2: TextStyle = serde_json::from_value(encoded_no_weight).unwrap();
    approx_eq(style2.font_weight, 400.0);

    // Round-trip
    let reencoded = serde_json::to_value(&style).unwrap();
    assert_eq!(reencoded["fontWeight"], json!(700.0));
}

#[test]
fn upstream_065_font_weight_legacy_missing_defaults_to_400() {
    // Legacy fixture has no fontWeight → should decode as 400
    let timeline: Timeline = read_fixture_json("legacy-defaults.palmier", "project.json");
    let style = timeline.tracks[0].clips[1].text_style.as_ref().unwrap();
    assert!(
        (style.font_weight - 400.0).abs() < 1e-9,
        "expected 400, got {}",
        style.font_weight
    );
}

#[test]
fn upstream_040_timeline_defaults_to_auto() {
    // PR #40: New timeline has transcription_language == None (Auto/system default)
    let timeline = Timeline::default();
    assert_eq!(timeline.transcription_language, None);
}

#[test]
fn upstream_040_timeline_round_trips_language() {
    // PR #40: Setting transcription_language persists through encode/decode.
    let mut timeline = Timeline::default();
    timeline.transcription_language = Some("fr-FR".to_string());
    let json = serde_json::to_value(&timeline).unwrap();
    assert_eq!(json["transcriptionLanguage"], json!("fr-FR"));

    let decoded: Timeline = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.transcription_language, Some("fr-FR".to_string()));
}

#[test]
fn upstream_040_auto_is_omitted_from_encoding() {
    // PR #40: When transcription_language is None, it's omitted from JSON output.
    let timeline = Timeline::default();
    let json = serde_json::to_value(&timeline).unwrap();
    assert!(
        !json
            .as_object()
            .unwrap()
            .contains_key("transcriptionLanguage"),
        "None transcriptionLanguage should be omitted from JSON"
    );
}

#[test]
fn upstream_040_legacy_missing_transcription_language_defaults_to_none() {
    // PR #40: Legacy projects without transcriptionLanguage → None (backward compat)
    let timeline: Timeline = read_fixture_json("legacy-defaults.palmier", "project.json");
    assert_eq!(timeline.transcription_language, None);
}

#[test]
fn upstream_046_shape_style_serde_round_trip() {
    // PR #46: ShapeStyle serializes and deserializes with camelCase keys.
    let encoded = json!({
        "type": "rect",
        "stroke": {"color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0}, "width": 3.0},
        "fill": {"enabled": true, "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 0.3}}
    });
    let style: ShapeStyle = serde_json::from_value(encoded).unwrap();
    assert_eq!(style.kind, ShapeKind::Rect);
    assert_eq!(style.stroke.width, 3.0);
    assert!(style.fill.enabled);

    // Round-trip
    let re: Value = serde_json::to_value(&style).unwrap();
    assert_eq!(re["type"], "rect");
    assert_eq!(re["stroke"]["width"], 3.0);
}

#[test]
fn upstream_046_shape_style_default_stroke_width() {
    // PR #46: Missing stroke width defaults to 2.0.
    let encoded = json!({"type": "oval"});
    let style: ShapeStyle = serde_json::from_value(encoded).unwrap();
    assert_eq!(style.kind, ShapeKind::Oval);
    assert_eq!(style.stroke.width, 2.0);
}

#[test]
fn upstream_046_clip_type_shape_decodes() {
    // PR #46: Clips can have media_type = "shape" in JSON.
    let encoded = json!({
        "id": "s1",
        "mediaRef": "",
        "mediaType": "shape",
        "sourceClipType": "shape",
        "startFrame": 0,
        "durationFrames": 100
    });
    let clip: Clip = serde_json::from_value(encoded).unwrap();
    assert_eq!(clip.media_type, ClipType::Shape);
    assert_eq!(clip.source_clip_type, ClipType::Shape);
}

#[test]
fn upstream_046_clip_with_shape_style_decodes() {
    // PR #46: Clip with embedded shape_style.
    let encoded = json!({
        "id": "s1",
        "mediaRef": "",
        "mediaType": "shape",
        "sourceClipType": "shape",
        "startFrame": 0,
        "durationFrames": 100,
        "shapeStyle": {
            "type": "arrow",
            "endpoints": {
                "start": {"x": 0.1, "y": 0.5},
                "end": {"x": 0.9, "y": 0.5}
            }
        }
    });
    let clip: Clip = serde_json::from_value(encoded).unwrap();
    assert_eq!(clip.media_type, ClipType::Shape);
    let style = clip.shape_style.unwrap();
    assert_eq!(style.kind, ShapeKind::Arrow);
    let endpoints = style.endpoints.unwrap();
    assert_eq!(endpoints.start.x, 0.1);
}

#[test]
fn upstream_046_legacy_clip_missing_shape_style_defaults_to_none() {
    // PR #46: Legacy clips without shape_style → None.
    let timeline: Timeline = read_fixture_json("legacy-defaults.palmier", "project.json");
    let clip = &timeline.tracks[0].clips[0];
    assert!(clip.shape_style.is_none());
    assert!(clip.stroke_progress_track.is_none());
}

#[test]
fn media_source_swift_shape_decodes_and_reencodes() {
    let encoded = json!({
        "external": {
            "absolutePath": "/tmp/interview.mov"
        }
    });

    let source: MediaSource = serde_json::from_value(encoded.clone()).unwrap();
    assert_eq!(
        source,
        MediaSource::External {
            absolute_path: "/tmp/interview.mov".to_string()
        }
    );

    let round_trip = serde_json::to_value(source).unwrap();
    assert_eq!(round_trip, encoded);
}

#[test]
fn legacy_generation_log_cost_migrates_to_credits() {
    let log: GenerationLog = read_fixture_json("legacy-defaults.palmier", "generation-log.json");

    assert_eq!(log.version, 1);
    assert_eq!(log.entries[0].cost_credits, Some(6));
    assert!(!log.entries[1].id.is_empty());
    assert_eq!(log.entries[1].cost_credits, None);
}

#[test]
fn legacy_chat_session_defaults_is_open_and_keeps_asset_mentions() {
    let session: ChatSession = read_fixture_json(
        "legacy-defaults.palmier",
        "chat/00000000-0000-0000-0000-000000000010.json",
    );

    assert!(session.is_open);
    assert_eq!(session.messages.len(), 1);
    let mention = &session.messages[0].mentions[0];
    assert_eq!(mention.media_ref.as_deref(), Some("legacy-video-1"));
    assert_eq!(mention.r#type, Some(ClipType::Video));
    assert!(mention.timeline_range.is_none());
}

#[test]
fn modern_chat_blocks_decode_tool_use_and_tool_result_shapes() {
    let session: ChatSession = read_fixture_json(
        "modern-rich.palmier",
        "chat/11111111-2222-3333-4444-555555555555.json",
    );

    let assistant = &session.messages[1];
    assert_eq!(assistant.blocks.len(), 3);

    match &assistant.blocks[1] {
        AgentContentBlock::ToolUse { id, name, input } => {
            assert_eq!(id, "tool-1");
            assert_eq!(name, "inspect_timeline");
            assert_eq!(input, "{\"clipId\":\"clip-video-1\"}");
        }
        other => panic!("unexpected block: {other:?}"),
    }

    match &assistant.blocks[2] {
        AgentContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            assert_eq!(tool_use_id, "tool-1");
            assert!(!is_error);
            assert!(matches!(content[0], ToolResultBlock::Text { .. }));
            assert!(matches!(content[1], ToolResultBlock::Image { .. }));
        }
        other => panic!("unexpected block: {other:?}"),
    }
}

#[test]
fn media_manifest_missing_version_decodes_as_v1() {
    let manifest: MediaManifest = read_fixture_json("legacy-defaults.palmier", "media.json");

    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.entries.len(), 2);
}
