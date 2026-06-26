/// Kind of save-as-media operation.
#[derive(Debug, Clone, PartialEq)]
pub enum SaveAsMediaKind {
    /// SAV-001: Saving a clip as media
    Clip {
        clip_id: String,
        source_name: String,
        is_video: bool,
        has_audio: bool,
    },
    /// SAV-008: Saving a timeline range as media
    TimelineRange { range_start: i64, range_end: i64 },
    /// SAV-012/013: Capturing the current frame
    CaptureFrame {
        /// SAV-012: includes text overlays (timeline tab)
        /// SAV-013: no text overlays (source-media tab)
        include_text_overlays: bool,
    },
}

/// State of a save-as-media placeholder.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceholderState {
    /// SAV-002: Placeholder created, pending export
    Pending,
    /// SAV-010: Rendering in progress
    Rendering,
    /// SAV-006: Export completed and finalized
    Completed { entry_id: String },
    /// SAV-007: Export failed
    Failed { error: String },
}

/// Validation result for a save-as-media plan.
#[derive(Debug, Clone)]
pub struct SaveAsMediaPlan {
    /// The kind of operation
    pub kind: SaveAsMediaKind,
    /// SAV-003/010: Placeholder display name
    pub placeholder_name: String,
    /// SAV-003: Default output filename
    pub default_filename: String,
    /// Whether the operation is valid
    pub is_valid: bool,
    /// Validation error messages (empty when valid)
    pub validation_errors: Vec<String>,
}

/// Default placeholder duration in frames for image captures (5s at 30fps).
pub const DEFAULT_CAPTURE_DURATION_FRAMES: i64 = 150;

// ---------------------------------------------------------------------------
// Validation and planning
// ---------------------------------------------------------------------------

/// SAV-001: Validate and plan a save-clip-as-media operation.
///
/// Returns a `SaveAsMediaPlan` with validation errors if the clip
/// is not video or audio, or lacks a resolvable source.
pub fn plan_save_clip_as_media(
    clip_id: &str,
    source_name: &str,
    is_video: bool,
    has_audio: bool,
) -> SaveAsMediaPlan {
    let mut errors = Vec::new();

    // SAV-001: Only video or audio clips with resolvable source media
    if !is_video && !has_audio {
        errors.push("Clip must be video or audio".to_string());
    }

    if source_name.is_empty() {
        errors.push("Clip must have a resolvable source".to_string());
    }

    let placeholder_name = if is_video || has_audio {
        format!("{source_name} (clip)")
    } else {
        source_name.to_string()
    };

    let default_filename = if is_video {
        format!("clip-{clip_id}.mp4")
    } else if has_audio {
        format!("clip-{clip_id}.m4a")
    } else {
        format!("clip-{clip_id}.mp4")
    };

    SaveAsMediaPlan {
        kind: SaveAsMediaKind::Clip {
            clip_id: clip_id.to_string(),
            source_name: source_name.to_string(),
            is_video,
            has_audio,
        },
        placeholder_name,
        default_filename,
        is_valid: errors.is_empty(),
        validation_errors: errors,
    }
}

/// SAV-008: Validate and plan a save-timeline-range operation.
///
/// Requires a valid positive-length range (end > start).
pub fn plan_save_timeline_range(range_start: i64, range_end: i64) -> SaveAsMediaPlan {
    let mut errors = Vec::new();

    // SAV-008: Valid positive-length range
    if range_end <= range_start {
        errors.push("Timeline range must have positive length (end > start)".to_string());
    }
    if range_start < 0 {
        errors.push("Range start must be non-negative".to_string());
    }

    SaveAsMediaPlan {
        kind: SaveAsMediaKind::TimelineRange {
            range_start,
            range_end,
        },
        // SAV-010: Placeholder named "Timeline range"
        placeholder_name: "Timeline range".to_string(),
        default_filename: "timeline-range.mp4".to_string(),
        is_valid: errors.is_empty(),
        validation_errors: errors,
    }
}

/// SAV-012/013: Plan a capture-frame operation.
pub fn plan_capture_frame(include_text_overlays: bool) -> SaveAsMediaPlan {
    SaveAsMediaPlan {
        kind: SaveAsMediaKind::CaptureFrame {
            include_text_overlays,
        },
        placeholder_name: "Captured frame".to_string(),
        default_filename: "captured-frame.png".to_string(),
        is_valid: true,
        validation_errors: vec![],
    }
}

/// SAV-003/010: Return the placeholder display name for a save kind.
pub fn placeholder_name_for(kind: &SaveAsMediaKind) -> String {
    match kind {
        SaveAsMediaKind::Clip { source_name, .. } => format!("{source_name} (clip)"),
        SaveAsMediaKind::TimelineRange { .. } => "Timeline range".to_string(),
        SaveAsMediaKind::CaptureFrame { .. } => "Captured frame".to_string(),
    }
}

/// SAV-003: Return the default output filename for a save kind.
pub fn default_filename_for(kind: &SaveAsMediaKind) -> String {
    match kind {
        SaveAsMediaKind::Clip {
            clip_id, is_video, ..
        } => {
            if *is_video {
                format!("clip-{clip_id}.mp4")
            } else {
                format!("clip-{clip_id}.m4a")
            }
        }
        SaveAsMediaKind::TimelineRange { .. } => "timeline-range.mp4".to_string(),
        SaveAsMediaKind::CaptureFrame { .. } => "captured-frame.png".to_string(),
    }
}

/// SAV-006/007/011: The final result of a placeholder export.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaceholderResult {
    /// Export succeeded: finalize the placeholder into a real media entry.
    Success {
        entry_id: String,
        entry: core_model::MediaManifestEntry,
    },
    /// Export failed: keep the placeholder with an error.
    Failure { error: String },
}

/// SAV-006/011: Build the final `MediaManifestEntry` from a successful export.
///
/// The caller provides the export output path and the new asset name.
/// The returned entry can be added to the manifest to finalize the placeholder.
pub fn finalize_placeholder(
    placeholder: &PlaceholderState,
    output_path: &str,
    asset_name: &str,
    clip_type: core_model::ClipType,
    duration_seconds: f64,
) -> Option<PlaceholderResult> {
    match placeholder {
        PlaceholderState::Pending | PlaceholderState::Rendering => {
            // Not yet ready for finalization
            None
        }
        PlaceholderState::Completed { entry_id } => {
            let entry = core_model::MediaManifestEntry {
                id: entry_id.clone(),
                name: asset_name.to_string(),
                r#type: clip_type,
                source: core_model::MediaSource::Project {
                    relative_path: output_path.to_string(),
                },
                duration: duration_seconds,
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
            };
            Some(PlaceholderResult::Success {
                entry_id: entry_id.clone(),
                entry,
            })
        }
        PlaceholderState::Failed { error } => Some(PlaceholderResult::Failure {
            error: error.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::*;

    // ── SAV-001: Video clip is valid ──
    #[test]
    fn sav_001_video_clip_valid() {
        let plan = plan_save_clip_as_media("clip1", "My Video", true, true);
        assert!(plan.is_valid);
        assert!(plan.validation_errors.is_empty());
    }

    // ── SAV-001: Audio clip is valid ──
    #[test]
    fn sav_001_audio_clip_valid() {
        let plan = plan_save_clip_as_media("clip2", "My Audio", false, true);
        assert!(plan.is_valid);
    }

    // ── SAV-001: Non-video/audio clip is rejected ──
    #[test]
    fn sav_001_non_media_clip_rejected() {
        let plan = plan_save_clip_as_media("clip3", "Text Clip", false, false);
        assert!(!plan.is_valid);
        assert!(plan
            .validation_errors
            .iter()
            .any(|e| e.contains("video or audio")));
    }

    // ── SAV-001: Empty source name is rejected ──
    #[test]
    fn sav_001_empty_source_rejected() {
        let plan = plan_save_clip_as_media("clip4", "", true, false);
        assert!(!plan.is_valid);
    }

    // ── SAV-002: Placeholder created in pending state ──
    #[test]
    fn sav_002_placeholder_pending() {
        let state = PlaceholderState::Pending;
        assert_eq!(state, PlaceholderState::Pending);
    }

    // ── SAV-003: Clip video placeholder name ──
    #[test]
    fn sav_003_video_placeholder_name() {
        let plan = plan_save_clip_as_media("c1", "Vacation", true, true);
        assert_eq!(plan.placeholder_name, "Vacation (clip)");
    }

    // ── SAV-003: Clip video default filename ──
    #[test]
    fn sav_003_video_default_filename() {
        let plan = plan_save_clip_as_media("c1", "Vacation", true, true);
        assert_eq!(plan.default_filename, "clip-c1.mp4");
    }

    // ── SAV-003: Clip audio default filename ──
    #[test]
    fn sav_003_audio_default_filename() {
        let plan = plan_save_clip_as_media("c2", "Voiceover", false, true);
        assert_eq!(plan.default_filename, "clip-c2.m4a");
    }

    // ── SAV-003: placeholder_name_for matches plan ──
    #[test]
    fn sav_003_placeholder_name_function() {
        let kind = SaveAsMediaKind::Clip {
            clip_id: "x".into(),
            source_name: "My Clip".into(),
            is_video: true,
            has_audio: true,
        };
        assert_eq!(placeholder_name_for(&kind), "My Clip (clip)");
    }

    // ── SAV-003: default_filename_for matches plan ──
    #[test]
    fn sav_003_default_filename_function() {
        let kind = SaveAsMediaKind::Clip {
            clip_id: "abc".into(),
            source_name: "Src".into(),
            is_video: true,
            has_audio: false,
        };
        assert_eq!(default_filename_for(&kind), "clip-abc.mp4");
    }

    #[test]
    fn sav_003_default_filename_audio() {
        let kind = SaveAsMediaKind::Clip {
            clip_id: "def".into(),
            source_name: "Src".into(),
            is_video: false,
            has_audio: true,
        };
        assert_eq!(default_filename_for(&kind), "clip-def.m4a");
    }

    // ── SAV-006: Completed state has entry_id ──
    #[test]
    fn sav_006_completed_state() {
        let state = PlaceholderState::Completed {
            entry_id: "entry-1".into(),
        };
        match state {
            PlaceholderState::Completed { entry_id } => {
                assert_eq!(entry_id, "entry-1");
            }
            _ => panic!("expected Completed state"),
        }
    }

    // ── SAV-007: Failed state has error message ──
    #[test]
    fn sav_007_failed_state() {
        let state = PlaceholderState::Failed {
            error: "disk full".into(),
        };
        match state {
            PlaceholderState::Failed { error } => {
                assert!(error.contains("disk full"));
            }
            _ => panic!("expected Failed state"),
        }
    }

    // ── SAV-007: Pending → Rendering → Failed transition ──
    #[test]
    fn sav_007_state_transitions() {
        let mut state = PlaceholderState::Pending;
        assert_eq!(state, PlaceholderState::Pending);

        state = PlaceholderState::Rendering;
        assert_eq!(state, PlaceholderState::Rendering);

        state = PlaceholderState::Failed {
            error: "timeout".into(),
        };
        assert!(matches!(state, PlaceholderState::Failed { .. }));
    }

    // ── SAV-008: Valid timeline range ──
    #[test]
    fn sav_008_valid_timeline_range() {
        let plan = plan_save_timeline_range(100, 200);
        assert!(plan.is_valid);
        assert!(plan.validation_errors.is_empty());
    }

    // ── SAV-008: Zero-length range rejected ──
    #[test]
    fn sav_008_zero_length_range_rejected() {
        let plan = plan_save_timeline_range(100, 100);
        assert!(!plan.is_valid);
    }

    // ── SAV-008: Negative range rejected ──
    #[test]
    fn sav_008_negative_range_rejected() {
        let plan = plan_save_timeline_range(200, 100);
        assert!(!plan.is_valid);
    }

    // ── SAV-008: Negative start rejected ──
    #[test]
    fn sav_008_negative_start_rejected() {
        let plan = plan_save_timeline_range(-10, 100);
        assert!(!plan.is_valid);
    }

    // ── SAV-010: Timeline range placeholder named ──
    #[test]
    fn sav_010_timeline_range_placeholder() {
        let plan = plan_save_timeline_range(0, 150);
        assert_eq!(plan.placeholder_name, "Timeline range");
    }

    // ── SAV-010: Timeline range default filename ──
    #[test]
    fn sav_010_timeline_range_filename() {
        let plan = plan_save_timeline_range(0, 150);
        assert_eq!(plan.default_filename, "timeline-range.mp4");
    }

    // ── SAV-010: Rendering state ──
    #[test]
    fn sav_010_rendering_state() {
        let state = PlaceholderState::Rendering;
        assert_eq!(state, PlaceholderState::Rendering);
    }

    // ── SAV-011: Timeline range follows same placeholder rules ──
    #[test]
    fn sav_011_timeline_range_fails_with_finalization() {
        // Completed after success
        let completed = PlaceholderState::Completed {
            entry_id: "e1".into(),
        };
        assert!(matches!(completed, PlaceholderState::Completed { .. }));

        // Failed after error
        let failed = PlaceholderState::Failed {
            error: "render error".into(),
        };
        assert!(matches!(failed, PlaceholderState::Failed { .. }));
    }

    // ── SAV-012: Capture with text overlays ──
    #[test]
    fn sav_012_capture_with_text_overlays() {
        let plan = plan_capture_frame(true);
        assert!(plan.is_valid);
        match &plan.kind {
            SaveAsMediaKind::CaptureFrame {
                include_text_overlays,
            } => {
                assert!(*include_text_overlays);
            }
            _ => panic!("expected CaptureFrame"),
        }
    }

    // ── SAV-013: Capture without text overlays ──
    #[test]
    fn sav_013_capture_without_text_overlays() {
        let plan = plan_capture_frame(false);
        match &plan.kind {
            SaveAsMediaKind::CaptureFrame {
                include_text_overlays,
            } => {
                assert!(!*include_text_overlays);
            }
            _ => panic!("expected CaptureFrame"),
        }
    }

    // ── Capture frame placeholder name ──
    #[test]
    fn capture_frame_placeholder_name() {
        let plan = plan_capture_frame(false);
        assert_eq!(plan.placeholder_name, "Captured frame");
    }

    // ── Capture frame default filename ──
    #[test]
    fn capture_frame_default_filename() {
        let plan = plan_capture_frame(false);
        assert_eq!(plan.default_filename, "captured-frame.png");
    }

    // ── placeholder_name_for all variants ──
    #[test]
    fn placeholder_name_for_all() {
        assert_eq!(
            placeholder_name_for(&SaveAsMediaKind::Clip {
                clip_id: "c".into(),
                source_name: "Src".into(),
                is_video: true,
                has_audio: false,
            }),
            "Src (clip)"
        );
        assert_eq!(
            placeholder_name_for(&SaveAsMediaKind::TimelineRange {
                range_start: 0,
                range_end: 100
            }),
            "Timeline range"
        );
        assert_eq!(
            placeholder_name_for(&SaveAsMediaKind::CaptureFrame {
                include_text_overlays: false
            }),
            "Captured frame"
        );
    }

    // ── default_filename_for all variants ──
    #[test]
    fn default_filename_for_all() {
        assert_eq!(
            default_filename_for(&SaveAsMediaKind::Clip {
                clip_id: "c".into(),
                source_name: "S".into(),
                is_video: true,
                has_audio: false,
            }),
            "clip-c.mp4"
        );
        assert_eq!(
            default_filename_for(&SaveAsMediaKind::TimelineRange {
                range_start: 0,
                range_end: 100
            }),
            "timeline-range.mp4"
        );
        assert_eq!(
            default_filename_for(&SaveAsMediaKind::CaptureFrame {
                include_text_overlays: true
            }),
            "captured-frame.png"
        );
    }

    // ── SAV-006: Finalize placeholder on success ────────────────

    #[test]
    fn sav_006_finalize_completed_placeholder() {
        let placeholder = PlaceholderState::Completed {
            entry_id: "entry-final".into(),
        };
        let result = finalize_placeholder(
            &placeholder,
            "media/clip-export.mp4",
            "My Video (clip)",
            core_model::ClipType::Video,
            5.0,
        );
        match result {
            Some(PlaceholderResult::Success { entry_id, entry }) => {
                assert_eq!(entry_id, "entry-final");
                assert_eq!(entry.name, "My Video (clip)");
                assert_eq!(entry.r#type, core_model::ClipType::Video);
                assert!((entry.duration - 5.0).abs() < f64::EPSILON);
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn sav_006_finalize_pending_placeholder_not_ready() {
        let placeholder = PlaceholderState::Pending;
        let result = finalize_placeholder(
            &placeholder,
            "media/output.mp4",
            "Asset",
            core_model::ClipType::Video,
            1.0,
        );
        assert!(result.is_none(), "pending should not be finalizable");
    }

    #[test]
    fn sav_006_finalize_rendering_placeholder_not_ready() {
        let placeholder = PlaceholderState::Rendering;
        let result = finalize_placeholder(
            &placeholder,
            "media/output.mp4",
            "Asset",
            core_model::ClipType::Video,
            1.0,
        );
        assert!(result.is_none(), "rendering should not be finalizable");
    }

    // ── SAV-007: Finalize placeholder on failure ────────────────

    #[test]
    fn sav_007_finalize_failed_placeholder() {
        let placeholder = PlaceholderState::Failed {
            error: "disk full".into(),
        };
        let result = finalize_placeholder(
            &placeholder,
            "media/output.mp4",
            "Asset",
            core_model::ClipType::Video,
            1.0,
        );
        match result {
            Some(PlaceholderResult::Failure { error }) => {
                assert_eq!(error, "disk full");
            }
            other => panic!("expected Failure, got {:?}", other),
        }
    }

    // ── SAV-011: Timeline range follows same rules ──────────────

    #[test]
    fn sav_011_timeline_range_finalize_success() {
        let placeholder = PlaceholderState::Completed {
            entry_id: "range-final".into(),
        };
        let result = finalize_placeholder(
            &placeholder,
            "media/timeline-range.mp4",
            "Timeline range",
            core_model::ClipType::Video,
            10.0,
        );
        match result {
            Some(PlaceholderResult::Success { entry_id, entry }) => {
                assert_eq!(entry_id, "range-final");
                assert_eq!(entry.name, "Timeline range");
                assert_eq!(entry.r#type, core_model::ClipType::Video);
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }

    #[test]
    fn sav_011_timeline_range_finalize_failure() {
        let placeholder = PlaceholderState::Failed {
            error: "render timeout".into(),
        };
        let result = finalize_placeholder(
            &placeholder,
            "media/timeline-range.mp4",
            "Timeline range",
            core_model::ClipType::Video,
            10.0,
        );
        match result {
            Some(PlaceholderResult::Failure { error }) => {
                assert_eq!(error, "render timeout");
            }
            other => panic!("expected Failure, got {:?}", other),
        }
    }

    #[test]
    fn sav_011_audio_clip_finalize() {
        let placeholder = PlaceholderState::Completed {
            entry_id: "audio-final".into(),
        };
        let result = finalize_placeholder(
            &placeholder,
            "media/clip-export.m4a",
            "Voiceover (clip)",
            core_model::ClipType::Audio,
            30.0,
        );
        match result {
            Some(PlaceholderResult::Success { entry, .. }) => {
                assert_eq!(entry.r#type, core_model::ClipType::Audio);
                match entry.source {
                    core_model::MediaSource::Project { relative_path } => {
                        assert_eq!(relative_path, "media/clip-export.m4a");
                    }
                    _ => panic!("expected Project source"),
                }
            }
            other => panic!("expected Success, got {:?}", other),
        }
    }
}
