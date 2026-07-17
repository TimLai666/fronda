use core_model::{
    AgentContentBlock, AnimPair, ChatSession, Clip, ClipType, Crop, Effect, Fill, GenerationLog,
    GenerationLogEntry, Interpolation, Keyframe, KeyframeTrack, MediaManifest, MediaSource, Rgba,
    ShapeKind, ShapeStyle, Stroke, TextAlignment, TextBackgroundStyle, TextFill, TextRgba,
    TextShadow, TextStyle, Timeline, ToolResultBlock, Track, Transform,
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
fn upstream_065_swift_isbold_isitalic_on_disk_compat() {
    // #65 on-disk compat: a Swift-authored TextStyle stores isBold/isItalic (bools) and NO
    // fontWeight. Rust must read them so Swift-authored bold/italic text isn't lost on load.
    let swift_bold = json!({ "fontName": "Poppins", "isBold": true, "isItalic": true });
    let s: TextStyle = serde_json::from_value(swift_bold).unwrap();
    approx_eq(s.font_weight, 700.0); // isBold=true → weight 700
    assert!(s.is_italic, "isItalic read");

    let swift_regular = json!({ "fontName": "Poppins", "isBold": false, "isItalic": false });
    let r: TextStyle = serde_json::from_value(swift_regular).unwrap();
    approx_eq(r.font_weight, 400.0);
    assert!(!r.is_italic);

    // fontWeight wins over isBold when both present (Rust is more expressive).
    let both = json!({ "fontName": "X", "fontWeight": 600, "isBold": true });
    let b: TextStyle = serde_json::from_value(both).unwrap();
    approx_eq(b.font_weight, 600.0);
}

#[test]
fn upstream_065_serialize_writes_both_swift_and_rust_keys() {
    // On save, Rust writes BOTH isBold/isItalic (for Swift) AND fontWeight (for Rust), so a
    // .palmier written by Rust round-trips bold/italic into Swift and back.
    let mut style = TextStyle {
        font_weight: 700.0,
        is_italic: true,
        ..Default::default()
    };
    style.font_name = "Anton".into();
    let j = serde_json::to_value(&style).unwrap();
    assert_eq!(j["fontWeight"], json!(700.0), "Rust key");
    assert_eq!(j["isBold"], json!(true), "Swift bold key");
    assert_eq!(j["isItalic"], json!(true), "Swift italic key");

    // Weight below 700 → isBold false.
    let light = TextStyle {
        font_weight: 400.0,
        ..Default::default()
    };
    let jl = serde_json::to_value(&light).unwrap();
    assert_eq!(jl["isBold"], json!(false));

    // Full round-trip preserves weight + italic + other fields.
    let back: TextStyle = serde_json::from_value(j).unwrap();
    approx_eq(back.font_weight, 700.0);
    assert!(back.is_italic);
    assert_eq!(back.font_name, "Anton");
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
fn upstream_330_336_text_style_v0610_round_trip_key_for_key() {
    // A Swift v0.6.10-authored TextStyle (post-#330 rich styling + #336 line styles)
    // must survive a Fronda load→save key-for-key. Swift's synthesized encoder always
    // writes every CodingKey, so every key below is present in real files.
    let swift = json!({
        "fontName": "Anton",
        "fontSize": 72.5,
        "fontScale": 1.1,
        "tracking": 2.5,
        "lineSpacing": 12.25,
        "fontCase": "uppercase",
        "isBold": true,
        "isItalic": false,
        "isUnderlined": true,
        "isStruckThrough": true,
        "isOverlined": true,
        "color": {"r": 1.0, "g": 0.5, "b": 0.25, "a": 1.0},
        "alignment": "left",
        "shadow": {"enabled": true, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.6},
                   "offsetX": 1.5, "offsetY": -3.0, "blur": 5.0},
        "background": {"enabled": true, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.6},
                       "paddingX": 10.5, "paddingY": 6.5, "cornerRadius": 12.5,
                       "offsetX": 3.5, "offsetY": -4.5,
                       "outlineColor": {"r": 0.2, "g": 0.4, "b": 0.6, "a": 1.0},
                       "outlineWidth": 2.5},
        "border": {"enabled": true, "color": {"r": 0.0, "g": 1.0, "b": 0.0, "a": 1.0},
                   "width": 7.5}
    });
    let s: TextStyle = serde_json::from_value(swift).unwrap();

    // In-memory: every new field carries the Swift value.
    approx_eq(s.tracking, 2.5);
    approx_eq(s.line_spacing, 12.25);
    assert_eq!(s.font_case, "uppercase");
    assert!(s.is_underlined);
    assert!(s.is_struck_through);
    assert!(s.is_overlined);
    approx_eq(s.border_width, 7.5);
    approx_eq(s.background_style.padding_x, 10.5);
    approx_eq(s.background_style.padding_y, 6.5);
    approx_eq(s.background_style.corner_radius, 12.5);
    approx_eq(s.background_style.offset_x, 3.5);
    approx_eq(s.background_style.offset_y, -4.5);
    approx_eq(s.background_style.outline_color.r, 0.2);
    approx_eq(s.background_style.outline_color.g, 0.4);
    approx_eq(s.background_style.outline_color.b, 0.6);
    approx_eq(s.background_style.outline_width, 2.5);
    // Old readers keep seeing the TextFill subset.
    assert!(s.background.enabled);
    approx_eq(s.background.color.a, 0.6);
    assert!(s.border.enabled);
    approx_eq(s.border.color.g, 1.0);

    // Re-encode: key-for-key against the Swift original.
    let j = serde_json::to_value(&s).unwrap();
    approx_eq(j["tracking"].as_f64().unwrap(), 2.5);
    approx_eq(j["lineSpacing"].as_f64().unwrap(), 12.25);
    assert_eq!(j["fontCase"], json!("uppercase"));
    assert_eq!(j["isBold"], json!(true));
    assert_eq!(j["isItalic"], json!(false));
    assert_eq!(j["isUnderlined"], json!(true));
    assert_eq!(j["isStruckThrough"], json!(true));
    assert_eq!(j["isOverlined"], json!(true));
    approx_eq(j["border"]["width"].as_f64().unwrap(), 7.5);
    assert_eq!(j["border"]["enabled"], json!(true));
    approx_eq(j["border"]["color"]["g"].as_f64().unwrap(), 1.0);
    assert_eq!(j["background"]["enabled"], json!(true));
    approx_eq(j["background"]["paddingX"].as_f64().unwrap(), 10.5);
    approx_eq(j["background"]["paddingY"].as_f64().unwrap(), 6.5);
    approx_eq(j["background"]["cornerRadius"].as_f64().unwrap(), 12.5);
    approx_eq(j["background"]["offsetX"].as_f64().unwrap(), 3.5);
    approx_eq(j["background"]["offsetY"].as_f64().unwrap(), -4.5);
    approx_eq(j["background"]["outlineColor"]["r"].as_f64().unwrap(), 0.2);
    approx_eq(j["background"]["outlineColor"]["g"].as_f64().unwrap(), 0.4);
    approx_eq(j["background"]["outlineColor"]["b"].as_f64().unwrap(), 0.6);
    approx_eq(j["background"]["outlineWidth"].as_f64().unwrap(), 2.5);
    approx_eq(j["fontSize"].as_f64().unwrap(), 72.5);
    approx_eq(j["shadow"]["offsetX"].as_f64().unwrap(), 1.5);
}

#[test]
fn upstream_330_text_style_pre_0609_decodes_with_defaults() {
    // A pre-#330/#336 file (old Fill-shaped background/border, none of the new keys)
    // still decodes; the new fields land on their Swift defaults.
    let old = json!({
        "fontName": "Poppins",
        "fontSize": 96.0,
        "isBold": false,
        "background": {"enabled": true, "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.3}},
        "border": {"enabled": true, "color": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}}
    });
    let s: TextStyle = serde_json::from_value(old).unwrap();
    approx_eq(s.tracking, 0.0);
    approx_eq(s.line_spacing, 0.0);
    assert_eq!(s.font_case, "mixed");
    assert!(!s.is_underlined);
    assert!(!s.is_struck_through);
    assert!(!s.is_overlined);
    // Swift's Outline.width decode fallback is 4.
    approx_eq(s.border_width, 4.0);
    // Background extras fall back to Swift Background defaults.
    approx_eq(s.background_style.padding_x, 0.0);
    approx_eq(s.background_style.padding_y, 0.0);
    approx_eq(s.background_style.corner_radius, 0.0);
    approx_eq(s.background_style.offset_x, 0.0);
    approx_eq(s.background_style.offset_y, 0.0);
    approx_eq(s.background_style.outline_color.r, 0.0);
    approx_eq(s.background_style.outline_color.a, 1.0);
    approx_eq(s.background_style.outline_width, 0.0);
    // The old subset is untouched.
    assert!(s.background.enabled);
    approx_eq(s.background.color.a, 0.3);
    assert!(s.border.enabled);
}

#[test]
fn upstream_330_text_style_default_writes_v0610_key_set() {
    // On save Fronda writes the full Swift v0.6.10 key set (same dual-write policy
    // as #65's isBold/isItalic), so a Swift open sees the shape it expects.
    let j = serde_json::to_value(TextStyle::default()).unwrap();
    approx_eq(j["tracking"].as_f64().unwrap(), 0.0);
    approx_eq(j["lineSpacing"].as_f64().unwrap(), 0.0);
    assert_eq!(j["fontCase"], json!("mixed"));
    assert_eq!(j["isUnderlined"], json!(false));
    assert_eq!(j["isStruckThrough"], json!(false));
    assert_eq!(j["isOverlined"], json!(false));
    approx_eq(j["border"]["width"].as_f64().unwrap(), 4.0);
    let bg = j["background"].as_object().unwrap();
    for key in [
        "paddingX",
        "paddingY",
        "cornerRadius",
        "offsetX",
        "offsetY",
        "outlineWidth",
    ] {
        approx_eq(bg[key].as_f64().unwrap(), 0.0);
    }
    approx_eq(bg["outlineColor"]["a"].as_f64().unwrap(), 1.0);
    approx_eq(bg["outlineColor"]["r"].as_f64().unwrap(), 0.0);
    // #65 keys still written.
    assert!(j["fontWeight"].is_number());
    assert!(j["isBold"].is_boolean());
}

#[test]
fn upstream_330_text_style_font_case_preserves_unknown_raw_value() {
    // fontCase is kept as the raw string so a future Swift case doesn't get
    // coerced/dropped by a Rust enum. Round-trips verbatim.
    let s: TextStyle =
        serde_json::from_value(json!({"fontName": "X", "fontCase": "smallCaps"})).unwrap();
    assert_eq!(s.font_case, "smallCaps");
    let j = serde_json::to_value(&s).unwrap();
    assert_eq!(j["fontCase"], json!("smallCaps"));
}

#[test]
fn upstream_330_text_style_rust_native_background_keys_coexist() {
    // Rust's #18 caption-background keys (`padding`, snake `corner_radius`) are a
    // separate contract from Swift's paddingX/paddingY/cornerRadius. Both sets
    // round-trip independently — no key collision, no data loss.
    let mixed = json!({
        "fontName": "X",
        "background": {
            "enabled": true,
            "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.6},
            "padding": 8.0,
            "corner_radius": 4.0,
            "paddingX": 1.5,
            "cornerRadius": 9.5
        }
    });
    let s: TextStyle = serde_json::from_value(mixed).unwrap();
    assert_eq!(s.background.padding, Some(8.0));
    assert_eq!(s.background.corner_radius, Some(4.0));
    approx_eq(s.background_style.padding_x, 1.5);
    approx_eq(s.background_style.corner_radius, 9.5);

    let j = serde_json::to_value(&s).unwrap();
    approx_eq(j["background"]["padding"].as_f64().unwrap(), 8.0);
    approx_eq(j["background"]["corner_radius"].as_f64().unwrap(), 4.0);
    approx_eq(j["background"]["paddingX"].as_f64().unwrap(), 1.5);
    approx_eq(j["background"]["cornerRadius"].as_f64().unwrap(), 9.5);
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
    let timeline = Timeline {
        transcription_language: Some("fr-FR".to_string()),
        folder_id: None,
        ..Default::default()
    };
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

#[test]
fn media_manifest_acronym_keys_roundtrip_without_data_loss() {
    // serde's camelCase lowercases acronyms; Swift/on-disk use uppercase (sourceFPS,
    // cachedRemoteURL, imageURLs, imageURLAssetIds). The real media.json must load
    // these WITHOUT dropping them, and re-serialization must emit the Swift keys.
    let manifest: MediaManifest = read_fixture_json("modern-rich.palmier", "media.json");

    let video = manifest
        .entries
        .iter()
        .find(|e| e.id == "asset-project-video")
        .expect("video entry");
    assert_eq!(video.source_fps, Some(30.0), "sourceFPS must load");
    let gi = video.generation_input.as_ref().expect("generationInput");
    assert_eq!(
        gi.image_urls,
        Some(vec!["https://example.com/board.png".to_string()])
    );
    assert_eq!(
        gi.reference_image_urls,
        Some(vec!["https://example.com/ref-image.png".to_string()])
    );
    assert_eq!(
        gi.reference_video_urls,
        Some(vec!["https://example.com/ref-video.mp4".to_string()])
    );
    assert_eq!(
        gi.reference_audio_urls,
        Some(vec!["https://example.com/ref-audio.wav".to_string()])
    );
    assert_eq!(
        gi.image_url_asset_ids,
        Some(vec!["asset-image-ref-1".to_string()])
    );

    let audio = manifest
        .entries
        .iter()
        .find(|e| e.id == "asset-external-audio")
        .expect("audio entry");
    assert_eq!(
        audio.cached_remote_url.as_deref(),
        Some("https://cdn.example.com/interview.wav"),
        "cachedRemoteURL must load"
    );
    assert!(
        audio.cached_remote_url_expires_at.is_some(),
        "cachedRemoteURLExpiresAt must load"
    );

    // Re-serialization emits Swift-compatible uppercase acronym keys, not lowercased.
    let out = serde_json::to_string(&manifest).unwrap();
    for key in [
        "\"sourceFPS\"",
        "\"cachedRemoteURL\"",
        "\"imageURLs\"",
        "\"imageURLAssetIds\"",
    ] {
        assert!(out.contains(key), "save must emit {key}");
    }
    assert!(
        !out.contains("\"sourceFps\""),
        "no lowercased acronym key on save"
    );
    assert!(
        !out.contains("\"cachedRemoteUrl\""),
        "no lowercased acronym key on save"
    );
}

#[test]
fn upstream_216_generation_recovery_fields_round_trip() {
    // Upstream #216: an in-flight generation's backendJobId + resultURLs persist so a
    // project saved mid-generation isn't corrupted; resultURLs keeps its uppercase key.
    let encoded = json!({
        "prompt": "p", "model": "m", "duration": 3, "aspectRatio": "16:9",
        "backendJobId": "job-abc",
        "outputIndex": 2,
        "resultURLs": ["https://cdn/r0.mp4", "https://cdn/r1.mp4", "https://cdn/r2.mp4"]
    });
    let gi: core_model::GenerationInput = serde_json::from_value(encoded).unwrap();
    assert_eq!(gi.backend_job_id.as_deref(), Some("job-abc"));
    // outputIndex is load-bearing for resume (placeholder → resultURLs[index]).
    assert_eq!(gi.output_index, Some(2));
    assert_eq!(gi.result_urls.as_ref().unwrap().len(), 3);

    let re = serde_json::to_value(&gi).unwrap();
    assert_eq!(re["backendJobId"], json!("job-abc"));
    assert_eq!(re["outputIndex"], json!(2));
    assert_eq!(re["resultURLs"][0], json!("https://cdn/r0.mp4"));
    assert!(
        !re.as_object().unwrap().contains_key("resultUrls"),
        "no lowercased acronym key"
    );
}

#[test]
fn upstream_216_generation_status_round_trips_on_entry() {
    // #216: the persisted async-generation status round-trips (present → preserved,
    // absent → None and omitted on save).
    let encoded = json!({
        "id": "gen1", "name": "g.mp4", "type": "video",
        "source": {"external": {"absolutePath": "/g.mp4"}},
        "duration": 3.0,
        "generationStatus": "generating"
    });
    let entry: core_model::MediaManifestEntry = serde_json::from_value(encoded).unwrap();
    assert_eq!(entry.generation_status.as_deref(), Some("generating"));
    assert_eq!(
        serde_json::to_value(&entry).unwrap()["generationStatus"],
        json!("generating")
    );

    let bare: core_model::MediaManifestEntry = serde_json::from_value(json!({
        "id": "x", "name": "x", "type": "video",
        "source": {"external": {"absolutePath": "/x"}}, "duration": 1.0
    }))
    .unwrap();
    assert!(bare.generation_status.is_none());
    assert!(!serde_json::to_value(&bare)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("generationStatus"));
}

#[test]
fn upstream_294_generation_input_target_language_round_trips() {
    // Upstream #294 (dubbing): Swift writes generationInput.targetLanguage via
    // encodeIfPresent — present value must survive a Fronda open→save, absent
    // field must stay absent (no key, no null).
    let encoded = json!({
        "prompt": "dub this", "model": "eleven-dub-v1", "duration": 12, "aspectRatio": "16:9",
        "targetLanguage": "es"
    });
    let gi: core_model::GenerationInput = serde_json::from_value(encoded).unwrap();
    assert_eq!(gi.target_language.as_deref(), Some("es"));

    let re = serde_json::to_value(&gi).unwrap();
    assert_eq!(re["targetLanguage"], json!("es"));

    // Absent stays absent: no key is written (Swift's encodeIfPresent semantics).
    let bare: core_model::GenerationInput = serde_json::from_value(json!({
        "prompt": "p", "model": "m", "duration": 3, "aspectRatio": "16:9"
    }))
    .unwrap();
    assert!(bare.target_language.is_none());
    let re_bare = serde_json::to_value(&bare).unwrap();
    assert!(
        !re_bare.as_object().unwrap().contains_key("targetLanguage"),
        "absent targetLanguage must not be written: {re_bare}"
    );
}

#[test]
fn upstream_238_agent_message_role_system_round_trips() {
    // A `system` role (upstream #238 MCP notices) must decode, not fail the session.
    use core_model::AgentMessageRole;
    assert_eq!(
        serde_json::from_str::<AgentMessageRole>("\"system\"").unwrap(),
        AgentMessageRole::System
    );
    assert_eq!(
        serde_json::to_string(&AgentMessageRole::System).unwrap(),
        "\"system\""
    );
    // Existing roles still round-trip lowercase.
    assert_eq!(
        serde_json::to_string(&AgentMessageRole::User).unwrap(),
        "\"user\""
    );
}

// ── FMT round-trip tests ──────────────────────────────────────────────────

#[test]
fn fmt_007_missing_track_flags_decode_to_defaults() {
    let encoded = json!({
        "type": "video",
        "clips": []
    });
    let track: Track = serde_json::from_value(encoded).unwrap();
    assert!(!track.muted, "muted should default to false");
    assert!(!track.hidden, "hidden should default to false");
    assert!(track.sync_locked, "syncLocked should default to true");
    assert!(track.clips.is_empty());
}

#[test]
fn fmt_008_minimal_clip_decodes_with_correct_defaults() {
    let encoded = json!({
        "mediaRef": "test",
        "startFrame": 0,
        "durationFrames": 30
    });
    let clip: Clip = serde_json::from_value(encoded).unwrap();

    // Numeric defaults
    approx_eq(clip.speed, 1.0);
    approx_eq(clip.volume, 1.0);
    approx_eq(clip.opacity, 1.0);
    assert_eq!(clip.fade_in_frames, 0);
    assert_eq!(clip.fade_out_frames, 0);
    assert_eq!(clip.fade_in_interpolation, Interpolation::Linear);
    assert_eq!(clip.fade_out_interpolation, Interpolation::Linear);
    assert_eq!(clip.trim_start_frame, 0);
    assert_eq!(clip.trim_end_frame, 0);

    // Transform default (center 0.5, width 1.0, height 1.0)
    approx_eq(clip.transform.center_x, 0.5);
    approx_eq(clip.transform.center_y, 0.5);
    approx_eq(clip.transform.width, 1.0);
    approx_eq(clip.transform.height, 1.0);
    approx_eq(clip.transform.rotation, 0.0);
    assert!(!clip.transform.flip_horizontal);
    assert!(!clip.transform.flip_vertical);

    // Crop default (all zero)
    approx_eq(clip.crop.left, 0.0);
    approx_eq(clip.crop.top, 0.0);
    approx_eq(clip.crop.right, 0.0);
    approx_eq(clip.crop.bottom, 0.0);

    // media_type and source_clip_type default to Video
    assert_eq!(clip.media_type, ClipType::Video);
    assert_eq!(clip.source_clip_type, ClipType::Video);

    // Optional fields default to None
    assert!(clip.link_group_id.is_none());
    assert!(clip.caption_group_id.is_none());
    assert!(clip.text_content.is_none());
    assert!(clip.text_style.is_none());
    assert!(clip.shape_style.is_none());
    assert!(clip.effects.is_none());

    // Keyframe tracks default to None
    assert!(clip.opacity_track.is_none());
    assert!(clip.position_track.is_none());
    assert!(clip.scale_track.is_none());
    assert!(clip.rotation_track.is_none());
    assert!(clip.crop_track.is_none());
    assert!(clip.volume_track.is_none());
    assert!(clip.stroke_progress_track.is_none());
    // Upstream #225 fields default to None.
    assert!(clip.text_animation.is_none());
    assert!(clip.word_timings.is_none());
}

#[test]
fn upstream_225_text_animation_and_word_timings_round_trip() {
    // Upstream #225: a text clip's textAnimation + wordTimings survive load/save
    // (present values are preserved, not dropped), under the Swift camelCase keys.
    let encoded = json!({
        "mediaRef": "",
        "mediaType": "text",
        "startFrame": 0,
        "durationFrames": 30,
        "textAnimation": {
            "preset": "wordReveal",
            "perWordFrames": 8,
            "highlight": {"r": 1.0, "g": 0.5, "b": 0.0, "a": 1.0}
        },
        "wordTimings": [
            {"text": "Hello", "startFrame": 0, "endFrame": 10},
            {"text": "World", "startFrame": 10, "endFrame": 30}
        ]
    });
    let clip: Clip = serde_json::from_value(encoded).unwrap();
    let anim = clip.text_animation.as_ref().expect("textAnimation decoded");
    assert_eq!(anim.preset, core_model::TextAnimationPreset::WordReveal);
    assert_eq!(anim.per_word_frames, 8);
    assert!(anim.highlight.is_some());
    let timings = clip.word_timings.as_ref().expect("wordTimings decoded");
    assert_eq!(timings.len(), 2);
    assert_eq!(timings[1].text, "World");
    assert_eq!(timings[1].end_frame, 30);

    // Re-encoding preserves the Swift keys (so a Swift↔Rust round-trip is lossless).
    let re = serde_json::to_value(&clip).unwrap();
    assert_eq!(re["textAnimation"]["preset"], json!("wordReveal"));
    assert_eq!(re["textAnimation"]["perWordFrames"], json!(8));
    assert_eq!(re["wordTimings"][0]["startFrame"], json!(0));
    assert_eq!(re["wordTimings"][1]["text"], json!("World"));
}

#[test]
fn fmt_009_timeline_round_trip_preserves_all_fields() {
    let mut selected = std::collections::HashSet::new();
    selected.insert("clip-1".to_string());

    let timeline = Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: selected,
        tracks: vec![
            Track {
                id: "track-video".to_string(),
                r#type: ClipType::Video,
                muted: false,
                hidden: true,
                sync_locked: false,
                display_height: 50.0,
                clips: vec![Clip {
                    id: "clip-1".to_string(),
                    media_ref: "media-1".to_string(),
                    media_type: ClipType::Video,
                    source_clip_type: ClipType::Video,
                    start_frame: 10,
                    duration_frames: 100,
                    trim_start_frame: 5,
                    trim_end_frame: 3,
                    speed: 1.5,
                    volume: 0.8,
                    fade_in_frames: 2,
                    fade_out_frames: 4,
                    fade_in_interpolation: Interpolation::Smooth,
                    fade_out_interpolation: Interpolation::Hold,
                    opacity: 0.9,
                    transform: Transform {
                        center_x: 0.3,
                        center_y: 0.4,
                        width: 0.5,
                        height: 0.6,
                        rotation: 0.1,
                        flip_horizontal: true,
                        flip_vertical: false,
                    },
                    crop: Crop {
                        left: 0.05,
                        top: 0.06,
                        right: 0.07,
                        bottom: 0.08,
                    },
                    link_group_id: Some("group-1".to_string()),
                    caption_group_id: Some("caption-group-1".to_string()),
                    text_content: Some("Hello".to_string()),
                    text_style: Some(TextStyle {
                        font_name: "Helvetica".to_string(),
                        font_size: 48.0,
                        font_scale: 1.2,
                        color: TextRgba {
                            r: 1.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        },
                        alignment: TextAlignment::Left,
                        shadow: TextShadow {
                            enabled: false,
                            color: TextRgba {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.5,
                            },
                            offset_x: 2.0,
                            offset_y: 3.0,
                            blur: 4.0,
                        },
                        background: TextFill {
                            enabled: true,
                            color: TextRgba {
                                r: 0.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.3,
                            },
                            padding: None,
                            corner_radius: None,
                        },
                        border: TextFill {
                            enabled: true,
                            color: TextRgba {
                                r: 1.0,
                                g: 1.0,
                                b: 1.0,
                                a: 1.0,
                            },
                            padding: None,
                            corner_radius: None,
                        },
                        font_weight: 700.0,
                        is_italic: false,
                        variable_font_axes: None,
                        letter_spacing: None,
                        line_height: None,
                        tracking: 1.5,
                        line_spacing: 4.25,
                        font_case: "lowercase".to_string(),
                        is_underlined: true,
                        is_struck_through: false,
                        is_overlined: true,
                        border_width: 6.5,
                        background_style: TextBackgroundStyle {
                            padding_x: 2.5,
                            padding_y: 3.5,
                            corner_radius: 5.5,
                            offset_x: 1.25,
                            offset_y: -1.75,
                            outline_color: TextRgba {
                                r: 0.1,
                                g: 0.2,
                                b: 0.3,
                                a: 1.0,
                            },
                            outline_width: 1.5,
                        },
                    }),
                    opacity_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: 1.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 30,
                                value: 0.5,
                                interpolation_out: Interpolation::Hold,
                            },
                        ],
                    }),
                    position_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: AnimPair { a: 0.0, b: 0.0 },
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 60,
                                value: AnimPair { a: 0.5, b: 0.5 },
                                interpolation_out: Interpolation::Linear,
                            },
                        ],
                    }),
                    scale_track: Some(KeyframeTrack {
                        keyframes: vec![Keyframe {
                            frame: 0,
                            value: AnimPair { a: 1.0, b: 1.0 },
                            interpolation_out: Interpolation::Smooth,
                        }],
                    }),
                    rotation_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: 0.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 100,
                                value: 360.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                        ],
                    }),
                    crop_track: Some(KeyframeTrack {
                        keyframes: vec![Keyframe {
                            frame: 0,
                            value: Crop {
                                left: 0.0,
                                top: 0.0,
                                right: 0.0,
                                bottom: 0.0,
                            },
                            interpolation_out: Interpolation::Smooth,
                        }],
                    }),
                    volume_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: 1.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 50,
                                value: 0.0,
                                interpolation_out: Interpolation::Linear,
                            },
                        ],
                    }),
                    effects: Some(vec![Effect::new("color.exposure", vec![("ev", 0.5)])]),
                    shape_style: Some(ShapeStyle {
                        kind: ShapeKind::Rect,
                        stroke: Stroke {
                            color: Rgba {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 1.0,
                            },
                            width: 3.0,
                            dashed: false,
                            arrowhead_style: None,
                        },
                        fill: Fill {
                            enabled: true,
                            color: Rgba {
                                r: 1.0,
                                g: 0.0,
                                b: 0.0,
                                a: 0.3,
                            },
                        },
                        arrowhead: None,
                        endpoints: None,
                    }),
                    stroke_progress_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: 0.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 60,
                                value: 1.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                        ],
                    }),
                    compound_timeline_id: None,
                    blend_mode: Default::default(),
                    chroma_key: None,
                    multicam_group_id: None,
                    text_animation: None,
                    word_timings: None,
                }],
            },
            Track {
                id: "track-audio".to_string(),
                r#type: ClipType::Audio,
                muted: true,
                hidden: false,
                sync_locked: true,
                display_height: 50.0,
                clips: vec![Clip {
                    id: "clip-audio-1".to_string(),
                    media_ref: "media-audio-1".to_string(),
                    media_type: ClipType::Audio,
                    source_clip_type: ClipType::Audio,
                    start_frame: 0,
                    duration_frames: 200,
                    trim_start_frame: 10,
                    trim_end_frame: 5,
                    speed: 1.0,
                    volume: 0.7,
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
                    volume_track: Some(KeyframeTrack {
                        keyframes: vec![
                            Keyframe {
                                frame: 0,
                                value: 0.7,
                                interpolation_out: Interpolation::Smooth,
                            },
                            Keyframe {
                                frame: 100,
                                value: 1.0,
                                interpolation_out: Interpolation::Smooth,
                            },
                        ],
                    }),
                    effects: None,
                    shape_style: None,
                    stroke_progress_track: None,
                    compound_timeline_id: None,
                    blend_mode: Default::default(),
                    chroma_key: None,
                    multicam_group_id: None,
                    text_animation: None,
                    word_timings: None,
                }],
            },
        ],
        transcription_language: Some("en-US".to_string()),
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
    };

    // Round-trip through JSON
    let json = serde_json::to_value(&timeline).unwrap();
    let decoded: Timeline = serde_json::from_value(json).unwrap();

    // Top-level
    assert_eq!(decoded.fps, 30);
    assert_eq!(decoded.width, 1920);
    assert_eq!(decoded.height, 1080);
    assert!(decoded.settings_configured);
    assert!(decoded.selected_clip_ids.contains("clip-1"));
    assert_eq!(decoded.transcription_language.as_deref(), Some("en-US"));
    assert_eq!(decoded.tracks.len(), 2);

    // Video track flags
    let vt = &decoded.tracks[0];
    assert_eq!(vt.r#type, ClipType::Video);
    assert!(!vt.muted);
    assert!(vt.hidden);
    assert!(!vt.sync_locked);

    // Audio track flags
    let at = &decoded.tracks[1];
    assert_eq!(at.r#type, ClipType::Audio);
    assert!(at.muted);
    assert!(!at.hidden);
    assert!(at.sync_locked);

    // ── Clip 1 (video) ──────────────────────────────────────────────────────
    let clip = &vt.clips[0];
    assert_eq!(clip.id, "clip-1");
    assert_eq!(clip.media_ref, "media-1");

    // Timing
    assert_eq!(clip.start_frame, 10);
    assert_eq!(clip.duration_frames, 100);
    assert_eq!(clip.trim_start_frame, 5);
    assert_eq!(clip.trim_end_frame, 3);

    // Numeric clip fields
    approx_eq(clip.speed, 1.5);
    approx_eq(clip.volume, 0.8);
    approx_eq(clip.opacity, 0.9);
    assert_eq!(clip.fade_in_frames, 2);
    assert_eq!(clip.fade_out_frames, 4);
    assert_eq!(clip.fade_in_interpolation, Interpolation::Smooth);
    assert_eq!(clip.fade_out_interpolation, Interpolation::Hold);

    // Transform
    approx_eq(clip.transform.center_x, 0.3);
    approx_eq(clip.transform.center_y, 0.4);
    approx_eq(clip.transform.width, 0.5);
    approx_eq(clip.transform.height, 0.6);
    approx_eq(clip.transform.rotation, 0.1);
    assert!(clip.transform.flip_horizontal);
    assert!(!clip.transform.flip_vertical);

    // Crop
    approx_eq(clip.crop.left, 0.05);
    approx_eq(clip.crop.top, 0.06);
    approx_eq(clip.crop.right, 0.07);
    approx_eq(clip.crop.bottom, 0.08);

    // Link groups
    assert_eq!(clip.link_group_id.as_deref(), Some("group-1"));
    assert_eq!(clip.caption_group_id.as_deref(), Some("caption-group-1"));

    // Text content
    assert_eq!(clip.text_content.as_deref(), Some("Hello"));
    let ts = clip.text_style.as_ref().unwrap();
    assert_eq!(ts.font_name, "Helvetica");
    approx_eq(ts.font_size, 48.0);
    approx_eq(ts.font_scale, 1.2);
    assert_eq!(ts.alignment, TextAlignment::Left);
    approx_eq(ts.font_weight, 700.0);
    // Full TextStyle fidelity — assert the remaining data-carrying fields too, so a future
    // dropped field (e.g. a wire-bridge regression) fails this round-trip test.
    approx_eq(ts.color.r, 1.0);
    approx_eq(ts.color.g, 0.0);
    approx_eq(ts.color.b, 0.0);
    approx_eq(ts.color.a, 1.0);
    assert!(!ts.is_italic);
    assert!(!ts.shadow.enabled);
    approx_eq(ts.shadow.offset_x, 2.0);
    approx_eq(ts.shadow.offset_y, 3.0);
    approx_eq(ts.shadow.blur, 4.0);
    assert!(ts.background.enabled);
    approx_eq(ts.background.color.a, 0.3);
    assert!(ts.border.enabled);
    approx_eq(ts.border.color.r, 1.0);
    approx_eq(ts.border.color.g, 1.0);
    approx_eq(ts.border.color.b, 1.0);
    // #330/#336 styling fields survive the full-project round-trip.
    approx_eq(ts.tracking, 1.5);
    approx_eq(ts.line_spacing, 4.25);
    assert_eq!(ts.font_case, "lowercase");
    assert!(ts.is_underlined);
    assert!(!ts.is_struck_through);
    assert!(ts.is_overlined);
    approx_eq(ts.border_width, 6.5);
    approx_eq(ts.background_style.padding_x, 2.5);
    approx_eq(ts.background_style.padding_y, 3.5);
    approx_eq(ts.background_style.corner_radius, 5.5);
    approx_eq(ts.background_style.offset_x, 1.25);
    approx_eq(ts.background_style.offset_y, -1.75);
    approx_eq(ts.background_style.outline_color.b, 0.3);
    approx_eq(ts.background_style.outline_width, 1.5);

    // Keyframes — opacity
    let ot = clip.opacity_track.as_ref().unwrap();
    assert_eq!(ot.keyframes.len(), 2);
    approx_eq(ot.keyframes[0].value, 1.0);
    approx_eq(ot.keyframes[1].value, 0.5);
    assert_eq!(ot.keyframes[1].interpolation_out, Interpolation::Hold);

    // Keyframes — position
    let pt = clip.position_track.as_ref().unwrap();
    assert_eq!(pt.keyframes[0].value.a, 0.0);
    assert_eq!(pt.keyframes[1].frame, 60);
    approx_eq(pt.keyframes[1].value.a, 0.5);

    // Keyframes — scale
    let st = clip.scale_track.as_ref().unwrap();
    assert_eq!(st.keyframes.len(), 1);

    // Keyframes — rotation
    let rt = clip.rotation_track.as_ref().unwrap();
    assert_eq!(rt.keyframes.len(), 2);
    approx_eq(rt.keyframes[1].value, 360.0);

    // Keyframes — crop
    let ct = clip.crop_track.as_ref().unwrap();
    assert_eq!(ct.keyframes.len(), 1);
    approx_eq(ct.keyframes[0].value.left, 0.0);

    // Keyframes — volume
    let cl_vt = clip.volume_track.as_ref().unwrap();
    assert_eq!(cl_vt.keyframes.len(), 2);
    approx_eq(cl_vt.keyframes[0].value, 1.0);
    approx_eq(cl_vt.keyframes[1].value, 0.0);

    // Effects
    let effects = clip.effects.as_ref().unwrap();
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].r#type, "color.exposure");

    // ShapeStyle
    let ss = clip.shape_style.as_ref().unwrap();
    assert_eq!(ss.kind, ShapeKind::Rect);
    approx_eq(ss.stroke.color.r, 1.0);
    approx_eq(ss.stroke.width, 3.0);
    assert!(ss.fill.enabled);

    // Stroke progress
    let sp = clip.stroke_progress_track.as_ref().unwrap();
    assert_eq!(sp.keyframes.len(), 2);
    approx_eq(sp.keyframes[0].value, 0.0);
    approx_eq(sp.keyframes[1].value, 1.0);

    // ── Clip 2 (audio) ──────────────────────────────────────────────────────
    let audio_clip = &at.clips[0];
    assert_eq!(audio_clip.media_ref, "media-audio-1");
    assert_eq!(audio_clip.start_frame, 0);
    assert_eq!(audio_clip.duration_frames, 200);
    assert!(audio_clip.link_group_id.is_none());
    assert!(audio_clip.text_content.is_none());
    assert!(audio_clip.effects.is_none());
}

#[test]
fn fmt_010_generation_log_entry_with_cost_migrates_to_credits() {
    // 0.07 * 100.0 = ~7.000000000000001 due to IEEE 754, so ceil yields 8
    let encoded = json!({
        "model": "test-model",
        "cost": 0.07
    });
    let entry: GenerationLogEntry = serde_json::from_value(encoded).unwrap();
    assert_eq!(entry.cost_credits, Some(8));
    assert!(!entry.id.is_empty());
}

#[test]
fn fmt_011_generation_log_entry_missing_id_auto_generates() {
    let encoded = json!({
        "model": "test-model",
        "costCredits": 50
    });
    let entry: GenerationLogEntry = serde_json::from_value(encoded).unwrap();
    assert!(!entry.id.is_empty());
    assert_eq!(entry.cost_credits, Some(50));
}

#[test]
fn fmt_012_generation_log_entry_cost_credits_directly() {
    let encoded = json!({
        "id": "test-log-1",
        "model": "test-model",
        "costCredits": 42
    });
    let entry: GenerationLogEntry = serde_json::from_value(encoded).unwrap();
    assert_eq!(entry.id, "test-log-1");
    assert_eq!(entry.cost_credits, Some(42));
}

#[test]
fn fmt_013_generation_log_entry_with_created_at_round_trips() {
    let encoded = json!({
        "id": "test-log-1",
        "model": "test-model",
        "costCredits": 10,
        "createdAt": 700000000.0
    });
    let entry: GenerationLogEntry = serde_json::from_value(encoded).unwrap();
    assert!(entry.created_at.is_some());
    assert_eq!(entry.cost_credits, Some(10));

    // Round-trip
    let reencoded = serde_json::to_value(&entry).unwrap();
    let decoded: GenerationLogEntry = serde_json::from_value(reencoded).unwrap();
    assert!(decoded.created_at.is_some());
    assert_eq!(decoded.model, "test-model");
    assert_eq!(decoded.cost_credits, Some(10));
}

#[test]
fn fmt_014_generation_log_round_trip_preserves_all_fields() {
    let log = GenerationLog {
        version: 1,
        entries: vec![
            GenerationLogEntry {
                id: "entry-1".to_string(),
                model: "model-a".to_string(),
                cost_credits: Some(100),
                created_at: None,
            },
            GenerationLogEntry {
                id: "entry-2".to_string(),
                model: "model-b".to_string(),
                cost_credits: None,
                created_at: None,
            },
        ],
    };

    let json = serde_json::to_value(&log).unwrap();
    let decoded: GenerationLog = serde_json::from_value(json).unwrap();

    assert_eq!(decoded.version, 1);
    assert_eq!(decoded.entries.len(), 2);
    assert_eq!(decoded.entries[0].id, "entry-1");
    assert_eq!(decoded.entries[0].cost_credits, Some(100));
    assert_eq!(decoded.entries[0].model, "model-a");
    assert_eq!(decoded.entries[1].model, "model-b");
    assert!(decoded.entries[1].cost_credits.is_none());
}
