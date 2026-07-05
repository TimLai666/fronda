//! Integration tests for XMEML 4 XML export (spec_xml_export).
//!
//! These tests construct realistic timelines and verify the XML output
//! against multiple spec requirements in a single comprehensive scenario.

use core_model::{
    AnimPair, Clip, ClipType, Crop, Interpolation, Keyframe, KeyframeTrack, Timeline, Track,
    Transform,
};
use render_core::xml_export::XmlExport;

/// Build a timeline with video, audio, and text clips that exercises all
/// XML-001 through XML-012 spec items.
fn comprehensive_timeline() -> Timeline {
    // Video clip with speed change, fades, volume, transform, crop, link group
    let v1 = Clip {
        id: "v1".into(),
        media_ref: "asset-video.mp4".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 0,
        duration_frames: 100,
        trim_start_frame: 10,
        trim_end_frame: 110,
        speed: 1.5,
        volume: 0.8,
        opacity: 0.9,
        fade_in_frames: 5,
        fade_out_frames: 8,
        fade_in_interpolation: Interpolation::Linear,
        fade_out_interpolation: Interpolation::Linear,
        transform: Transform {
            width: 0.8,
            height: 0.8,
            rotation: 10.0,
            center_x: 0.5,
            center_y: 0.3,
            flip_horizontal: false,
            flip_vertical: false,
        },
        crop: Crop {
            left: 0.1,
            top: 0.0,
            right: 0.0,
            bottom: 0.2,
        },
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
        effects: None,
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        text_animation: None,
        word_timings: None,
    };

    // Second video on a separate visual track (to test reversal)
    let v2 = Clip {
        id: "v2".into(),
        media_ref: "asset-B-roll.mov".into(),
        media_type: ClipType::Video,
        source_clip_type: ClipType::Video,
        start_frame: 50,
        duration_frames: 80,
        trim_start_frame: 0,
        trim_end_frame: 80,
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
        effects: None,
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        text_animation: None,
        word_timings: None,
    };

    // Audio clip linked to v1 (same link group)
    let a1 = Clip {
        id: "a1".into(),
        media_ref: "asset-audio.wav".into(),
        media_type: ClipType::Audio,
        source_clip_type: ClipType::Audio,
        start_frame: 0,
        duration_frames: 100,
        trim_start_frame: 0,
        trim_end_frame: 100,
        speed: 1.0,
        volume: 0.7,
        opacity: 1.0,
        fade_in_frames: 3,
        fade_out_frames: 5,
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
        effects: None,
        shape_style: None,
        stroke_progress_track: None,
        compound_timeline_id: None,
        blend_mode: Default::default(),
        chroma_key: None,
        text_animation: None,
        word_timings: None,
    };

    // Text clip (should NOT appear in XML output - XML-013)
    let txt1 = Clip {
        id: "txt1".into(),
        media_ref: String::new(),
        media_type: ClipType::Text,
        source_clip_type: ClipType::Text,
        start_frame: 10,
        duration_frames: 50,
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
        text_content: Some("Hello".into()),
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
    };

    // Unresolved clip (empty media_ref, should be skipped - XML-012)
    let unresolved = Clip {
        id: "unresolved".into(),
        media_ref: String::new(),
        media_type: ClipType::Video,
        ..v1.clone()
    };

    Timeline {
        id: String::new(),
        name: String::new(),
        fps: 30,
        width: 1920,
        height: 1080,
        settings_configured: true,
        selected_clip_ids: std::collections::HashSet::new(),
        transcription_language: None,
        folder_id: None,
        compound_timelines: std::collections::HashMap::new(),
        tracks: vec![
            Track {
                id: "v-track".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![v1],
            },
            Track {
                id: "v-B-roll".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![v2],
            },
            Track {
                id: "text-track".into(),
                r#type: ClipType::Text,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![txt1],
            },
            Track {
                id: "a-track".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![a1, unresolved],
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// XML-001: Output starts with XMEML 4 / FCP format
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_xmeml_4_format() {
    let xml = XmlExport::export(&comprehensive_timeline());
    assert!(
        xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "Should start with XML declaration"
    );
    assert!(
        xml.contains("<!DOCTYPE xmeml>"),
        "Should have xmeml doctype"
    );
    assert!(
        xml.contains("<xmeml version=\"4\">"),
        "Should declare XMEML version 4"
    );
    assert!(xml.contains("</xmeml>"), "Should close xmeml element");
}

// ---------------------------------------------------------------------------
// XML-002: Clip placement on tracks is preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_clip_placement_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // v1 starts at frame 0, duration 100
    assert!(xml.contains("<start>0</start>"), "v1 should start at 0");
    assert!(xml.contains("<duration>100</duration>"), "v1 duration 100");
    // v2 starts at frame 50, duration 80
    assert!(xml.contains("id=\"v2\""), "v2 should be present");
    assert!(xml.contains("<start>50</start>"), "v2 should start at 50");
    // The unresolved clip should not appear
    assert!(
        !xml.contains("unresolved"),
        "unresolved clip should be skipped"
    );
}

// ---------------------------------------------------------------------------
// XML-003: Source trims are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_source_trims_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    assert!(xml.contains("<in>10</in>"), "v1 trim start should be 10");
    assert!(xml.contains("<out>110</out>"), "v1 trim end should be 110");
}

// ---------------------------------------------------------------------------
// XML-004: Speed changes are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_speed_change_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    assert!(xml.contains("<speed>"), "speed element should be emitted");
    assert!(xml.contains("<value>1.500</value>"), "speed value 1.5");
}

// ---------------------------------------------------------------------------
// XML-005: Volume and opacity are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_volume_and_opacity_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // Volume 0.8 for v1
    assert!(xml.contains("<volume>"), "volume element should exist");
    assert!(
        xml.contains("0.800000"),
        "v1 volume 0.8 (or clip volume contains 0.8)"
    );
    // Opacity 0.9 for v1
    assert!(
        xml.contains("<opacity>0.900000</opacity>"),
        "v1 opacity should be 0.9"
    );
}

// ---------------------------------------------------------------------------
// XML-006: Transform and crop are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_transform_and_crop_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // Transform parameters in filter/effect
    assert!(
        xml.contains("<parameterid>scale</parameterid>"),
        "scale parameter should exist"
    );
    assert!(
        xml.contains("<parameterid>rotation</parameterid>"),
        "rotation parameter should exist"
    );
    assert!(
        xml.contains("<parameterid>center</parameterid>"),
        "center parameter should exist"
    );
    assert!(
        xml.contains("<parameterid>crop</parameterid>"),
        "crop parameter should exist"
    );
    // v1 transform values: width=0.8, rotation=10.0
    assert!(xml.contains("0.800000"), "scale 0.8 should appear");
    assert!(xml.contains("10.000000"), "rotation 10.0 should appear");
}

// ---------------------------------------------------------------------------
// XML-007: Fades are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_fades_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // v1 has fade_in=5, fade_out=8, now exported as single-sided <transitionitem>s (the
    // Premiere-read form) rather than <fadein>/<fadeout> tags.
    assert!(
        !xml.contains("<fadein>") && !xml.contains("<fadeout>"),
        "legacy fade tags removed"
    );
    assert!(xml.contains("<transitionitem>"), "transitionitem emitted");
    assert!(
        xml.contains("<alignment>start-black</alignment>"),
        "fade-in edge"
    );
    assert!(
        xml.contains("<alignment>end-black</alignment>"),
        "fade-out edge"
    );
}

// ---------------------------------------------------------------------------
// XML-008: Linked clip relationships are preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_linked_clip_relationships() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // v1 and a1 share link group "g1"
    assert!(xml.contains("<link>"), "link element should exist");
    assert!(
        xml.contains("<linkclipref>g1</linkclipref>"),
        "link should reference g1"
    );
    assert!(
        xml.contains("<medialink>true</medialink>"),
        "link should be medialink true"
    );
}

// ---------------------------------------------------------------------------
// XML-010: Visual track order is reversed
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_visual_track_order_reversed() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // v2 is on the second visual track; should appear BEFORE v1 in XML
    let v1_pos = xml.find("id=\"v1\"").unwrap();
    let v2_pos = xml.find("id=\"v2\"").unwrap();
    assert!(
        v2_pos < v1_pos,
        "v2 (second visual track) should appear before v1 in XML (reversed order)"
    );
}

// ---------------------------------------------------------------------------
// XML-012: Unresolved media (empty media_ref) are skipped
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_unresolved_media_skipped() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // The unresolved clip has id "unresolved" and empty media_ref
    assert!(
        !xml.contains("unresolved"),
        "clip with empty media_ref should be skipped"
    );
}

// ---------------------------------------------------------------------------
// XML-013: Text overlays are not preserved
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_text_overlays_not_preserved() {
    let xml = XmlExport::export(&comprehensive_timeline());
    // The text clip has id "txt1"
    assert!(
        !xml.contains("txt1"),
        "text overlay clip should not appear in XML"
    );
}

// ---------------------------------------------------------------------------
// XML-014: Empty timeline produces valid XML
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_empty_timeline() {
    let timeline = Timeline::default();
    let xml = XmlExport::export(&timeline);
    assert!(xml.contains("<?xml"), "empty timeline should produce XML");
    assert!(
        xml.contains("</xmeml>"),
        "empty timeline should close xmeml"
    );
}

// ---------------------------------------------------------------------------
// Sequence: rate, duration in file element
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_sequence_metadata() {
    let timeline = comprehensive_timeline();
    let xml = XmlExport::export(&timeline);
    // Sequence duration
    assert!(
        xml.contains("<duration>130</duration>"),
        "should have total duration (v2 ends at 130)"
    );
    // Rate / timebase
    assert!(
        xml.contains("<timebase>30</timebase>"),
        "timebase should be 30 fps"
    );
    assert!(xml.contains("<ntsc>FALSE</ntsc>"), "ntsc should be FALSE");
}

// ---------------------------------------------------------------------------
// XML-012 / INS-003: position keyframes store TOP-LEFT; the exported "center"
// param must be the resolved centre (top_left + size/2), not the raw value.
// ---------------------------------------------------------------------------
#[test]
fn xml_spec_position_keyframes_export_resolved_center() {
    let mut clip = Clip {
        id: "pv".into(),
        media_ref: "asset.mp4".into(),
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
        transform: Transform {
            width: 0.5,
            height: 0.5,
            rotation: 0.0,
            center_x: 0.5,
            center_y: 0.5,
            flip_horizontal: false,
            flip_vertical: false,
        },
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
    };
    // Top-left held at the origin; a half-canvas clip → resolved centre (0.25, 0.25).
    clip.position_track = Some(KeyframeTrack {
        keyframes: vec![Keyframe {
            frame: 0,
            value: AnimPair { a: 0.0, b: 0.0 },
            interpolation_out: Interpolation::Hold,
        }],
    });
    let timeline = Timeline {
        tracks: vec![Track {
            id: "t1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
           display_height: 50.0,
            clips: vec![clip],
        }],
        ..Timeline::default()
    };
    let xml = XmlExport::export(&timeline);
    assert!(
        xml.contains("<keyframe><when>0</when><value>0.250000 0.250000</value></keyframe>"),
        "center keyframe should be resolved centre (0.25,0.25), not raw top-left (0,0):\n{xml}"
    );
    assert!(
        !xml.contains("<value>0.000000 0.000000</value>"),
        "raw top-left (0,0) must not leak into the center param"
    );
}
