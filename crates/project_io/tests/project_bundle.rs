use core_model::{ClipType, GENERATION_LOG_FILENAME, MANIFEST_FILENAME, TIMELINE_FILENAME};
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
            .join("chat")
            .join("00000000-0000-0000-0000-000000000010.json"),
        "not valid json",
    )
    .unwrap();

    let bundle = ProjectBundle::open(&destination).unwrap();
    assert!(bundle.chat_sessions.is_empty());
}

#[test]
fn invalid_media_json_is_fatal() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("modern-rich.palmier");
    copy_dir_all(&fixture_bundle_path("modern-rich.palmier"), &destination);
    fs::write(destination.join(MANIFEST_FILENAME), "{ definitely-bad").unwrap();

    let error = ProjectBundle::open(&destination).unwrap_err();
    match error {
        BundleError::DecodeJson { path, .. } => {
            assert!(path.ends_with(MANIFEST_FILENAME));
        }
        other => panic!("unexpected error: {other:?}"),
    }
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
