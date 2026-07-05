use core_model::{
    ClipType, CHAT_DIRECTORY_NAME, GENERATION_LOG_FILENAME, MANIFEST_FILENAME,
    MEDIA_DIRECTORY_NAME, THUMBNAIL_FILENAME, TIMELINE_FILENAME, TRANSCRIPTS_DIRECTORY_NAME,
    VISUAL_INDEXES_DIRECTORY_NAME,
};
use project_io::{BundleError, ProjectBundle};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn fixture_bundle_path(bundle: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rust-rewrite/projects")
        .join(bundle)
}

fn copy_dir_all(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();

    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_all(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

fn assert_bundle_semantics(actual: &ProjectBundle, expected: &ProjectBundle) {
    assert_eq!(actual.timeline, expected.timeline);
    assert_eq!(actual.manifest, expected.manifest);
    assert_eq!(actual.generation_log, expected.generation_log);
    assert_eq!(actual.chat_sessions, expected.chat_sessions);
}

#[test]
fn opens_modern_bundle_and_exposes_optional_paths() {
    let bundle = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();

    assert_eq!(bundle.timeline.fps, 30);
    assert!(bundle.manifest.is_some());
    assert!(bundle.generation_log.is_some());
    assert_eq!(bundle.chat_sessions.len(), 1);
    assert!(bundle.thumbnail_path.is_some());
    assert!(bundle.media_dir.is_some());
    assert!(bundle.chat_dir.is_some());
    assert!(bundle
        .project_path_for("media/generated-shot.mp4")
        .is_file());
}

#[test]
fn opens_legacy_bundle_with_swift_compat_defaults() {
    let bundle = ProjectBundle::open(fixture_bundle_path("legacy-defaults.palmier")).unwrap();

    let track = &bundle.timeline.tracks[0];
    assert!(!track.muted);
    assert!(!track.hidden);
    assert!(track.sync_locked);
    assert_eq!(track.clips[0].media_type, ClipType::Video);
    assert_eq!(bundle.manifest.as_ref().unwrap().version, 1);
    assert_eq!(
        bundle.generation_log.as_ref().unwrap().entries[0].cost_credits,
        Some(6)
    );
    assert!(bundle.chat_sessions[0].is_open);
}

#[test]
fn ignores_invalid_generation_log_but_still_opens_bundle() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    fs::write(destination.join(GENERATION_LOG_FILENAME), "{ not-json").unwrap();

    let bundle = ProjectBundle::open(&destination).unwrap();
    assert!(bundle.generation_log.is_none());
    assert_eq!(bundle.timeline.tracks.len(), 2);
}

#[test]
fn skips_corrupt_chat_files() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("legacy-defaults.palmier");
    copy_dir_all(
        &fixture_bundle_path("legacy-defaults.palmier"),
        &destination,
    );
    fs::write(
        destination
            .join(CHAT_DIRECTORY_NAME)
            .join("00000000-0000-0000-0000-000000000010.json"),
        "not valid json",
    )
    .unwrap();

    let bundle = ProjectBundle::open(&destination).unwrap();
    assert!(bundle.chat_sessions.is_empty());
}

#[test]
fn invalid_media_json_recovers_to_empty_manifest() {
    // Upstream palmier-pro #224: a corrupt media.json must not block the open.
    // The project loads with an empty manifest (media offline) and the
    // original file is preserved on disk until the next save.
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    fs::write(destination.join(MANIFEST_FILENAME), "{ definitely-bad").unwrap();

    let bundle = ProjectBundle::open(&destination).expect("open should recover");
    let manifest = bundle.manifest.expect("corrupt manifest degrades to empty");
    assert!(
        manifest.entries.is_empty(),
        "recovered manifest should have no entries"
    );

    // Original corrupt file is left untouched on open.
    let raw = fs::read_to_string(destination.join(MANIFEST_FILENAME)).unwrap();
    assert_eq!(raw, "{ definitely-bad");
}

#[test]
fn missing_project_json_is_fatal() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    fs::remove_file(destination.join(TIMELINE_FILENAME)).unwrap();

    let error = ProjectBundle::open(&destination).unwrap_err();
    match error {
        BundleError::MissingRequiredFile { path } => {
            assert!(path.ends_with(TIMELINE_FILENAME));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn saves_bundle_to_new_path_with_semantic_round_trip_and_assets() {
    let source = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    let temp = tempdir().unwrap();
    let destination = temp.path().join("saved-copy.palmier");

    source.save_to(&destination).unwrap();

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert_bundle_semantics(&reopened, &source);
    assert!(reopened.thumbnail_path.is_some());
    assert!(reopened.media_dir.is_some());
    assert!(reopened.chat_dir.is_some());
    assert!(destination.join(THUMBNAIL_FILENAME).is_file());
    assert!(destination.join(MEDIA_DIRECTORY_NAME).is_dir());
    assert!(destination
        .join(MEDIA_DIRECTORY_NAME)
        .join("generated-shot.mp4")
        .is_file());
}

#[test]
fn save_in_place_persists_timeline_changes() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);

    let mut bundle = ProjectBundle::open(&destination).unwrap();
    bundle.timeline.fps = 24;
    bundle.timeline.tracks[0].clips[0].duration_frames = 72;

    bundle.save().unwrap();

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert_eq!(reopened.timeline.fps, 24);
    assert_eq!(reopened.timeline.tracks[0].clips[0].duration_frames, 72);
    assert!(reopened.thumbnail_path.is_some());
    assert!(reopened.media_dir.is_some());
}

#[test]
fn save_removes_stale_optional_files_and_directories_when_absent() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);

    let mut bundle = ProjectBundle::open(&destination).unwrap();
    bundle.manifest = None;
    bundle.generation_log = None;
    bundle.chat_sessions.clear();
    bundle.thumbnail_path = None;
    bundle.media_dir = None;
    bundle.chat_dir = None;

    bundle.save().unwrap();

    assert!(!destination.join(MANIFEST_FILENAME).exists());
    assert!(!destination.join(GENERATION_LOG_FILENAME).exists());
    assert!(!destination.join(CHAT_DIRECTORY_NAME).exists());
    assert!(!destination.join(THUMBNAIL_FILENAME).exists());
    assert!(!destination.join(MEDIA_DIRECTORY_NAME).exists());

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert!(reopened.manifest.is_none());
    assert!(reopened.generation_log.is_none());
    assert!(reopened.chat_sessions.is_empty());
    assert!(reopened.thumbnail_path.is_none());
    assert!(reopened.media_dir.is_none());
    assert!(reopened.chat_dir.is_none());
}

#[test]
fn save_preserves_unreadable_chat_files() {
    // A corrupt / unknown-format chat file is skipped on load; a later save must NOT
    // delete it. Previously the whole chat directory was wiped on save, permanently
    // losing any session that failed to parse on open.
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    let corrupt = destination
        .join(CHAT_DIRECTORY_NAME)
        .join("99999999-0000-0000-0000-000000000099.json");
    fs::write(&corrupt, "not valid json").unwrap();

    let mut bundle = ProjectBundle::open(&destination).unwrap();
    assert!(!bundle.chat_sessions.is_empty(), "valid session still loads");
    bundle.timeline.fps = 24; // unrelated change → triggers a full save
    bundle.save().unwrap();

    assert!(corrupt.exists(), "unreadable chat file must survive a save");
    assert_eq!(fs::read_to_string(&corrupt).unwrap(), "not valid json");
    let reopened = ProjectBundle::open(&destination).unwrap();
    assert!(!reopened.chat_sessions.is_empty(), "valid session rewritten");
}

#[test]
fn save_leaves_no_stray_temp_files() {
    // Atomic writes go via a sibling .tmp then rename; none must linger after save.
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    let mut bundle = ProjectBundle::open(&destination).unwrap();
    bundle.timeline.fps = 24;
    bundle.save().unwrap();

    for entry in fs::read_dir(&destination).unwrap() {
        let path = entry.unwrap().path();
        assert_ne!(
            path.extension().and_then(|e| e.to_str()),
            Some("tmp"),
            "stray temp file left behind: {path:?}"
        );
    }
}

#[test]
fn saves_and_loads_transcripts() {
    let temp = tempdir().unwrap();
    let source = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    let destination = temp.path().join("with-transcripts.palmier");

    let mut bundle = source.clone();
    use search_core::search_index::CacheIdentity;
    use search_core::transcript::{TranscribedWord, Transcript, TranscriptSegment};

    bundle.transcripts.insert(
        "media-001".into(),
        Transcript {
            identity: CacheIdentity {
                path: "/videos/clip1.mp4".into(),
                modification_time: 1_700_000_000,
                file_size: 42_000,
            },
            is_full_file: true,
            segments: vec![TranscriptSegment {
                start_seconds: 0.0,
                end_seconds: 2.0,
                text: "hello world".into(),
                words: vec![TranscribedWord {
                    word: "hello".into(),
                    start_seconds: 0.0,
                    end_seconds: 0.5,
                }],
            }],
            language: Some("en".into()),
        },
    );

    bundle.save_to(&destination).unwrap();
    assert!(destination.join(TRANSCRIPTS_DIRECTORY_NAME).is_dir());
    assert!(destination
        .join(TRANSCRIPTS_DIRECTORY_NAME)
        .join("media-001.json")
        .is_file());

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert_eq!(reopened.transcripts.len(), 1);
    assert_eq!(
        reopened
            .transcripts
            .get("media-001")
            .unwrap()
            .language
            .as_deref(),
        Some("en")
    );
    assert!(reopened.transcripts_dir.is_some());
}

#[test]
fn saves_and_loads_visual_indexes() {
    let temp = tempdir().unwrap();
    let source = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    let destination = temp.path().join("with-vindex.palmier");

    let mut bundle = source.clone();
    use search_core::search_index::{CacheIdentity, EmbeddingRow, VisualIndex};

    bundle.visual_indexes.insert(
        "media-002".into(),
        VisualIndex {
            identity: CacheIdentity {
                path: "/videos/clip2.mp4".into(),
                modification_time: 1_700_000_001,
                file_size: 99_000,
            },
            rows: vec![EmbeddingRow {
                frame: 0,
                embedding: vec![0.1, 0.2, 0.3],
            }],
        },
    );

    bundle.save_to(&destination).unwrap();
    assert!(destination.join(VISUAL_INDEXES_DIRECTORY_NAME).is_dir());

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert_eq!(reopened.visual_indexes.len(), 1);
    let loaded = reopened.visual_indexes.get("media-002").unwrap();
    assert_eq!(loaded.rows[0].embedding, vec![0.1, 0.2, 0.3]);
}

#[test]
fn transcripts_empty_when_directory_missing() {
    let bundle = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    assert!(bundle.transcripts.is_empty());
    assert!(bundle.visual_indexes.is_empty());
    assert!(bundle.transcripts_dir.is_none());
    assert!(bundle.visual_indexes_dir.is_none());
}

#[test]
fn save_removes_stale_transcripts_and_visual_indexes_when_empty() {
    let temp = tempdir().unwrap();
    let source = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    let destination = temp.path().join("stale-cache.palmier");

    let mut bundle = source.clone();
    use search_core::search_index::{CacheIdentity, EmbeddingRow, VisualIndex};

    // Save with data first
    bundle.visual_indexes.insert(
        "tmp".into(),
        VisualIndex {
            identity: CacheIdentity {
                path: "/tmp/v.mp4".into(),
                modification_time: 0,
                file_size: 0,
            },
            rows: vec![EmbeddingRow {
                frame: 0,
                embedding: vec![0.5],
            }],
        },
    );
    bundle.save_to(&destination).unwrap();
    assert!(destination.join(VISUAL_INDEXES_DIRECTORY_NAME).is_dir());

    // Clear and save again — should remove the directory
    let mut bundle2 = ProjectBundle::open(&destination).unwrap();
    bundle2.visual_indexes.clear();
    bundle2.save().unwrap();

    assert!(!destination.join(VISUAL_INDEXES_DIRECTORY_NAME).exists());

    let reopened = ProjectBundle::open(&destination).unwrap();
    assert!(reopened.visual_indexes.is_empty());
    assert!(reopened.visual_indexes_dir.is_none());
}

#[test]
fn save_project_state_writes_only_timeline_and_manifest() {
    let temp = tempdir().unwrap();
    let root = temp.path().join("narrow.palmier");
    fs::create_dir_all(root.join(CHAT_DIRECTORY_NAME)).unwrap();
    let chat_path = root.join(CHAT_DIRECTORY_NAME).join("session1.json");
    let chat_content = r#"{"id":"s1","title":"Chat","messages":[]}"#;
    fs::write(&chat_path, chat_content).unwrap();

    let timeline: core_model::Timeline = serde_json::from_str(r#"{"fps":60}"#).unwrap();
    let mut manifest = core_model::MediaManifest::default();
    manifest.folders.push(core_model::MediaFolder {
        id: "f1".into(),
        name: "B-roll".into(),
        parent_folder_id: None,
    });

    project_io::save_project_state(&root, &timeline, &manifest).unwrap();

    let reopened = ProjectBundle::open(&root).unwrap();
    assert_eq!(reopened.timeline.fps, 60);
    let folders = &reopened.manifest.unwrap().folders;
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "B-roll");
    assert_eq!(
        fs::read_to_string(&chat_path).unwrap(),
        chat_content,
        "chat files must be untouched"
    );
    assert!(!root.join(GENERATION_LOG_FILENAME).exists());
}

#[test]
fn non_finite_float_is_sanitized_on_save_so_project_reopens() {
    // A non-finite f64 serializes to `null` and makes the saved project.json
    // unopenable; save-time sanitization must replace it with a finite default so
    // the file always reopens.
    let mut bundle = ProjectBundle::open(fixture_bundle_path("modern-rich.palmier")).unwrap();
    bundle.timeline.tracks[0].clips[0].speed = f64::NAN;
    bundle.timeline.tracks[0].clips[0].opacity = f64::INFINITY;

    let temp = tempdir().unwrap();
    let dest = temp.path().join("out.palmier");
    bundle.save_to(&dest).unwrap();

    let reopened = ProjectBundle::open(&dest).expect("sanitized project must reopen");
    let clip = &reopened.timeline.tracks[0].clips[0];
    assert!(clip.speed.is_finite(), "speed not sanitized: {}", clip.speed);
    assert!(clip.opacity.is_finite(), "opacity not sanitized: {}", clip.opacity);
    assert_eq!(clip.speed, 1.0);
    assert_eq!(clip.opacity, 1.0);
}

// ── Upstream #255: multi-timeline ProjectFile round-trip ────────────────────

#[test]
fn opens_swift_v061_multi_timeline_project_and_preserves_siblings() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("multi.palmier");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(TIMELINE_FILENAME),
        r#"{
            "timelines": [
                {"id": "tl-a", "name": "Main", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []},
                {"id": "tl-b", "name": "Shorts cut", "fps": 30, "width": 1080, "height": 1920,
                 "settingsConfigured": true, "tracks": []}
            ],
            "activeTimelineId": "tl-b",
            "openTimelineIds": ["tl-a", "tl-b"],
            "viewStates": {"tl-a": {"playheadFrame": 5, "zoomScale": 1.5, "scrollOffsetX": 10.0}}
        }"#,
    )
    .unwrap();

    let mut bundle = ProjectBundle::open(&root).unwrap();
    assert_eq!(bundle.timeline.id, "tl-b", "activeTimelineId wins");
    assert_eq!(bundle.timeline.name, "Shorts cut");
    assert_eq!(bundle.multi.siblings.len(), 1);
    assert_eq!(bundle.multi.siblings[0].id, "tl-a");
    assert_eq!(bundle.multi.active_index, 1, "original array position kept");

    // Edit the active timeline and save: the sibling + view state survive,
    // and array order is preserved.
    bundle.timeline.fps = 24;
    bundle.save().unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(TIMELINE_FILENAME)).unwrap()).unwrap();
    let timelines = json["timelines"].as_array().unwrap();
    assert_eq!(timelines.len(), 2);
    assert_eq!(timelines[0]["id"], "tl-a", "sibling kept at index 0");
    assert_eq!(timelines[1]["id"], "tl-b");
    assert_eq!(timelines[1]["fps"], 24, "active edit persisted");
    assert_eq!(json["activeTimelineId"], "tl-b");
    assert_eq!(json["viewStates"]["tl-a"]["playheadFrame"], 5);
}

#[test]
fn opens_legacy_bare_timeline_and_saves_projectfile_form() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("legacy.palmier");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(TIMELINE_FILENAME),
        r#"{"fps": 24, "width": 1280, "height": 720, "settingsConfigured": true, "tracks": []}"#,
    )
    .unwrap();

    let bundle = ProjectBundle::open(&root).unwrap();
    assert_eq!(bundle.timeline.fps, 24);
    assert!(bundle.multi.siblings.is_empty());

    bundle.save().unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(TIMELINE_FILENAME)).unwrap()).unwrap();
    assert!(json.get("timelines").is_some(), "saved in ProjectFile form");
    assert_eq!(json["timelines"][0]["fps"], 24);

    // The upgraded file reopens identically.
    let reopened = ProjectBundle::open(&root).unwrap();
    assert_eq!(reopened.timeline.fps, 24);
    assert_eq!(reopened.timeline.id, bundle.timeline.id, "id now stable");
}

#[test]
fn save_project_state_preserves_sibling_timelines() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("narrow.palmier");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(TIMELINE_FILENAME),
        r#"{
            "timelines": [
                {"id": "tl-a", "name": "Main", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []},
                {"id": "tl-b", "name": "Alt", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []}
            ],
            "activeTimelineId": "tl-a"
        }"#,
    )
    .unwrap();

    // The narrow save holds only the active timeline; the sibling must survive.
    let mut active = ProjectBundle::open(&root).unwrap().timeline;
    assert_eq!(active.id, "tl-a");
    active.width = 3840;
    project_io::save_project_state(&root, &active, &core_model::MediaManifest::default()).unwrap();

    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(TIMELINE_FILENAME)).unwrap()).unwrap();
    let timelines = json["timelines"].as_array().unwrap();
    assert_eq!(timelines.len(), 2, "sibling tl-b survived the narrow save");
    assert_eq!(timelines[0]["width"], 3840, "active edit written");
    assert_eq!(timelines[1]["id"], "tl-b");
}


#[test]
fn save_project_state_with_siblings_is_authoritative_deletions_stick() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("authoritative.palmier");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(TIMELINE_FILENAME),
        r#"{
            "timelines": [
                {"id": "tl-a", "name": "A", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []},
                {"id": "tl-b", "name": "B", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []},
                {"id": "tl-c", "name": "C", "fps": 30, "width": 1920, "height": 1080,
                 "settingsConfigured": true, "tracks": []}
            ],
            "activeTimelineId": "tl-b",
            "openTimelineIds": ["tl-a", "tl-b", "tl-c"],
            "viewStates": {"tl-c": {"playheadFrame": 9, "zoomScale": 1.0, "scrollOffsetX": 0.0}}
        }"#,
    )
    .unwrap();

    // The editor deleted C: the save carries only active B + sibling A.
    let bundle = ProjectBundle::open(&root).unwrap();
    let active = bundle.timeline.clone();
    let a = bundle
        .multi
        .siblings
        .iter()
        .find(|t| t.id == "tl-a")
        .unwrap()
        .clone();
    project_io::save_project_state_with_siblings(
        &root,
        &active,
        &[a],
        &core_model::MediaManifest::default(),
    )
    .unwrap();

    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(TIMELINE_FILENAME)).unwrap()).unwrap();
    let ids: Vec<&str> = json["timelines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["tl-a", "tl-b"], "C stays deleted, order kept");
    let open: Vec<&str> = json["openTimelineIds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(open, vec!["tl-a", "tl-b"], "open ids pruned of C");
    assert!(
        json.get("viewStates").is_none(),
        "view state for the deleted timeline pruned (and empty map omitted)"
    );

    // A second save does not resurrect anything either.
    project_io::save_project_state_with_siblings(
        &root,
        &active,
        &[bundle.multi.siblings[0].clone()],
        &core_model::MediaManifest::default(),
    )
    .unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join(TIMELINE_FILENAME)).unwrap()).unwrap();
    assert_eq!(json["timelines"].as_array().unwrap().len(), 2);
}
