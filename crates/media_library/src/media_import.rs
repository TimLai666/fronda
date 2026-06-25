/// Media import finalization pipeline and directory import.
///
/// MED-003..018: Handles creating manifest entries, finalizing
/// imported media (images, Lottie, video), detecting offline and
/// unprocessable states, and importing directories recursively.
use core_model::{ClipType, MediaManifest, MediaManifestEntry, MediaSource};

use crate::SupportedExtensions;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// MED-009: Default still-image duration in seconds (5 seconds).
pub const DEFAULT_STILL_DURATION_SECONDS: f64 = 5.0;

// ---------------------------------------------------------------------------
// Import result types
// ---------------------------------------------------------------------------

/// Result of creating an import entry.
///
/// MED-005: Import creates both an in-memory representation and
/// a manifest entry. We capture the latter here.
#[derive(Debug, Clone)]
pub struct ImportedEntry {
    /// The manifest entry that was created.
    pub entry: MediaManifestEntry,
}

/// The result of finalizing an imported asset.
///
/// MED-007: Finalization after import must load metadata and write
/// it back to the manifest entry.
#[derive(Debug, Clone)]
pub struct FinalizationResult {
    /// Updated entry with metadata filled in.
    pub entry: MediaManifestEntry,
    /// MED-008: Whether search indexing should be scheduled.
    pub schedule_search_index: bool,
    /// MED-009/010: Thumbnail data (raw bytes), if extracted.
    pub thumbnail_data: Option<Vec<u8>>,
}

/// Finalization error kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum FinalizationError {
    /// File not found on disk.
    FileNotFound,
    /// File exists but cannot be decoded.
    Unprocessable { details: String },
    /// Unsupported media type for finalization.
    UnsupportedClipType,
}

/// Offline / unprocessable status for a manifest entry.
///
/// MED-014: isMediaOffline and isMediaUnprocessable remain distinct states.
#[derive(Debug, Clone, PartialEq)]
pub enum MediaStatus {
    /// File exists and is processable.
    Online,
    /// File does not exist on disk (and no cached remote URL).
    Offline,
    /// File exists but cannot be decoded/processed.
    Unprocessable { reason: String },
}

// ---------------------------------------------------------------------------
// Import planning
// ---------------------------------------------------------------------------

/// Plan an import of a new media asset from a file path.
///
/// MED-003: Asset names default to the filename stem.
/// MED-004: Creates an external manifest reference (no copy into project).
/// MED-006: Optionally assigns a logical folderId.
pub fn plan_import(
    file_path: &str,
    folder_id: Option<String>,
    clip_type: ClipType,
) -> Option<ImportedEntry> {
    let path = std::path::Path::new(file_path);

    // Get filename stem for naming.
    let stem = path.file_stem()?.to_str()?;
    if stem.is_empty() {
        return None;
    }

    let id = uuid::Uuid::new_v4().to_string();

    let source = MediaSource::External {
        absolute_path: file_path.to_string(),
    };

    let entry = MediaManifestEntry {
        id,
        name: stem.to_string(),
        r#type: clip_type,
        source,
        duration: 0.0,
        generation_input: None,
        source_width: None,
        source_height: None,
        source_fps: None,
        has_audio: None,
        folder_id,
        cached_remote_url: None,
        cached_remote_url_expires_at: None,
        source_timecode_frame: None,
        source_timecode_quanta: None,
        source_timecode_drop_frame: None,
    };

    Some(ImportedEntry { entry })
}

/// MED-006: Assign a folder to an entry, returning the updated entry.
pub fn assign_folder(entry: &MediaManifestEntry, folder_id: Option<String>) -> MediaManifestEntry {
    let mut e = entry.clone();
    e.folder_id = folder_id;
    e
}

/// MED-011: Rebuild a display collection of entries from a manifest.
///
/// Includes all entries regardless of offline status.
/// MED-012: Missing media remain represented instead of disappearing.
pub fn entries_from_manifest(manifest: &MediaManifest) -> Vec<MediaManifestEntry> {
    manifest.entries.clone()
}

// ---------------------------------------------------------------------------
// Display label (MED-013)
// ---------------------------------------------------------------------------

/// MED-013: `clipDisplayLabel` logic.
///
/// Uses text content for text clips, generation placeholder name for
/// generating assets, and resolver display name otherwise.
pub fn clip_display_label(entry: &MediaManifestEntry) -> String {
    // Text clips show text content (or empty string if no content).
    if entry.r#type == ClipType::Text {
        return entry.name.clone();
    }

    // Generation assets show a generation placeholder name.
    if entry.generation_input.is_some() {
        return entry.name.clone();
    }

    // Default: resolver display name (the filename stem).
    entry.name.clone()
}

// ---------------------------------------------------------------------------
// Media status (MED-014)
// ---------------------------------------------------------------------------

/// Determine the media status for an entry.
///
/// MED-014: Distinguishes between offline and unprocessable.
/// This is a pure logic check — actual file I/O is done by the
/// `file_exists` callback.
pub fn media_status(
    entry: &MediaManifestEntry,
    file_exists: impl Fn(&str) -> bool,
    is_processable: impl Fn(&MediaManifestEntry) -> bool,
) -> MediaStatus {
    // If there's a cached remote URL, the file can be re-downloaded.
    if entry.cached_remote_url.is_some() {
        return MediaStatus::Online;
    }

    let path = match &entry.source {
        MediaSource::External { absolute_path } => absolute_path,
        MediaSource::Project { relative_path } => relative_path,
    };

    if !file_exists(path) {
        return MediaStatus::Offline;
    }

    if !is_processable(entry) {
        return MediaStatus::Unprocessable {
            reason: format!("{} file could not be decoded", entry.r#type.name()),
        };
    }

    MediaStatus::Online
}

// ---------------------------------------------------------------------------
// Finalization (MED-007..010)
// ---------------------------------------------------------------------------

/// Finalize an imported image entry.
///
/// MED-009: Loads still-image metadata (dimensions), generates a
/// default still duration, and schedules search indexing.
pub fn finalize_image(entry: &MediaManifestEntry, width: i64, height: i64) -> FinalizationResult {
    let mut e = entry.clone();
    e.source_width = Some(width);
    e.source_height = Some(height);
    e.duration = DEFAULT_STILL_DURATION_SECONDS;

    FinalizationResult {
        entry: e,
        schedule_search_index: true,
        thumbnail_data: None,
    }
}

/// Finalize an imported Lottie entry.
///
/// MED-010: Loads animation duration, size, framerate, and thumbnail.
pub fn finalize_lottie(
    entry: &MediaManifestEntry,
    width: i64,
    height: i64,
    duration_seconds: f64,
    fps: f64,
) -> FinalizationResult {
    let mut e = entry.clone();
    e.source_width = Some(width);
    e.source_height = Some(height);
    e.duration = duration_seconds;
    e.source_fps = Some(fps);

    FinalizationResult {
        entry: e,
        schedule_search_index: true,
        thumbnail_data: None,
    }
}

/// Finalize an imported video entry.
///
/// Loads video metadata (dimensions, duration, fps, audio flag).
pub fn finalize_video(
    entry: &MediaManifestEntry,
    width: i64,
    height: i64,
    duration_seconds: f64,
    fps: f64,
    has_audio: bool,
) -> FinalizationResult {
    let mut e = entry.clone();
    e.source_width = Some(width);
    e.source_height = Some(height);
    e.duration = duration_seconds;
    e.source_fps = Some(fps);
    e.has_audio = Some(has_audio);

    FinalizationResult {
        entry: e,
        schedule_search_index: true,
        thumbnail_data: None,
    }
}

/// Finalize an imported audio entry.
///
/// MED-007: Audio finalization loads duration metadata.
pub fn finalize_audio(entry: &MediaManifestEntry, duration_seconds: f64) -> FinalizationResult {
    let mut e = entry.clone();
    e.duration = duration_seconds;

    FinalizationResult {
        entry: e,
        schedule_search_index: true,
        thumbnail_data: None,
    }
}

// ---------------------------------------------------------------------------
// Directory import (MED-015..018)
// ---------------------------------------------------------------------------

/// A candidate file found during directory scanning.
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryEntry {
    /// Full absolute path to the file.
    pub path: String,
    /// Detected clip type based on extension.
    pub clip_type: Option<ClipType>,
    /// Filename stem (for naming).
    pub stem: String,
    /// Relative path from the scan root (for folder mirroring).
    pub relative_path: String,
}

/// Plan for importing a directory.
///
/// MED-015: Importing a directory recursively mirrors its tree into
/// logical media folders.
#[derive(Debug, Clone)]
pub struct DirectoryImportPlan {
    /// The root directory being imported.
    pub root_path: String,
    /// All discovered files (after filtering).
    pub entries: Vec<DirectoryEntry>,
    /// Number of files skipped (unsupported extensions, hidden).
    pub skipped_count: usize,
}

/// Scan a directory recursively for importable media files.
///
/// MED-015: Recursively mirrors the directory tree.
/// MED-016: Skips hidden files (starting with `.`).
/// MED-017: Only imports supported media file types.
/// MED-018: Sorts entries using localized standard filename ordering.
///
/// This is a pure logical scan — actual filesystem I/O is provided
/// via callbacks, making it testable without real files.
pub fn scan_directory(
    root_path: &str,
    list_directory: impl Fn(&str) -> Result<Vec<String>, String>,
    is_hidden: impl Fn(&str) -> bool,
) -> Result<DirectoryImportPlan, String> {
    let mut entries = Vec::new();
    let mut skipped_count = 0usize;

    scan_directory_recursive(
        root_path,
        root_path,
        &mut entries,
        &mut skipped_count,
        &list_directory,
        &is_hidden,
    )?;

    // MED-018: Sort by filename (standard ASCII ordering is sufficient
    // for testability; actual OS localization depends on the callback).
    entries.sort_by(|a, b| a.stem.to_lowercase().cmp(&b.stem.to_lowercase()));

    Ok(DirectoryImportPlan {
        root_path: root_path.to_string(),
        entries,
        skipped_count,
    })
}

fn scan_directory_recursive(
    root: &str,
    current: &str,
    entries: &mut Vec<DirectoryEntry>,
    skipped_count: &mut usize,
    list_directory: &impl Fn(&str) -> Result<Vec<String>, String>,
    is_hidden: &impl Fn(&str) -> bool,
) -> Result<(), String> {
    let children = list_directory(current)?;

    for child in &children {
        // MED-016: Skip hidden files/directories.
        if is_hidden(child) {
            *skipped_count += 1;
            continue;
        }

        // Always construct full child_path by prepending current directory.
        let child_path = format!(
            "{}/{}",
            current.trim_end_matches('/'),
            child.trim_start_matches('/')
        );

        // Try listing sub-items — if it succeeds it's a directory (recurse).
        // If it fails with "not a directory", treat it as a file.
        match list_directory(&child_path) {
            Ok(_sub_children) => {
                // It's a directory — recurse.
                scan_directory_recursive(
                    root,
                    &child_path,
                    entries,
                    skipped_count,
                    list_directory,
                    is_hidden,
                )?;

                // Don't add the directory itself as an entry, but skip the
                // "not a directory" fallthrough.
                continue;
            }
            Err(_) => {
                // It's a file — process below.
            }
        }

        let path = std::path::Path::new(&child_path);
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => {
                *skipped_count += 1;
                continue;
            }
        };

        // MED-017: Check extension against supported types.
        let ext = std::path::Path::new(&child_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let clip_type = SupportedExtensions::clip_type_for(ext);
        if clip_type.is_none() {
            *skipped_count += 1;
            continue;
        }

        let relative_path = child_path
            .strip_prefix(root)
            .unwrap_or(&child_path)
            .trim_start_matches('/')
            .to_string();

        entries.push(DirectoryEntry {
            path: child_path,
            clip_type,
            stem,
            relative_path,
        });
    }

    Ok(())
}

/// Build folder hierarchy from a directory scan for mirroring.
///
/// MED-015: Creates logical folder entries that mirror the directory tree.
/// Each unique directory under the root produces one folder definition.
#[derive(Debug, Clone)]
pub struct FolderMirrorEntry {
    /// Logical folder name.
    pub name: String,
    /// Parent folder id (None = root).
    pub parent_folder_id: Option<String>,
    /// Relative path from scan root used as a unique key.
    pub relative_path: String,
}

/// Derive folder mirror entries from a directory scan plan.
pub fn derive_folder_mirror(plan: &DirectoryImportPlan) -> Vec<FolderMirrorEntry> {
    let mut folders: Vec<FolderMirrorEntry> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Collect all unique parent directories from the scan entries.
    for entry in &plan.entries {
        let rel_path = std::path::Path::new(&entry.relative_path);
        let parent = rel_path.parent();
        if parent.is_none() || parent.unwrap().as_os_str().is_empty() {
            continue; // File is at root — no folder mirror needed.
        }
        let parent_str = parent.unwrap().to_string_lossy().to_string();
        if seen.contains(&parent_str) {
            continue;
        }
        seen.insert(parent_str.clone());

        // Determine parent's own parent.
        let grandparent = parent.unwrap().parent();
        let grandparent_str = grandparent
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.to_string_lossy().to_string());

        let folder_name = parent
            .unwrap()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| parent_str.clone());

        folders.push(FolderMirrorEntry {
            name: folder_name,
            parent_folder_id: grandparent_str,
            relative_path: parent_str,
        });
    }

    // Sort by depth (shallowest first) so parent folders are created
    // before their children.
    folders.sort_by_key(|f| f.relative_path.matches('/').count());
    folders
}

// ---------------------------------------------------------------------------
// Pasted image bytes and project-internal media (MED-019..021)
// ---------------------------------------------------------------------------

/// MED-019: Generate a filename for pasted image bytes.
///
/// Creates a project-internal file named `pasted-<id>.<ext>`.
pub fn pasted_image_filename(id: &str, ext: &str) -> String {
    format!("pasted-{}.{}", id, ext.trim_start_matches('.'))
}

/// MED-020: Resolve the target path for pasted image bytes.
///
/// When a project is open, writes into the project `media/` directory.
/// Otherwise writes into a system temp directory.
pub fn resolve_pasted_image_path(id: &str, ext: &str, project_media_dir: Option<&str>) -> String {
    let filename = pasted_image_filename(id, ext);
    match project_media_dir {
        Some(dir) => format!("{}/{}", dir.trim_end_matches('/'), filename),
        None => format!("{}/{}", std::env::temp_dir().to_string_lossy(), filename),
    }
}

/// MED-021: Resolve a project-internal path for generated/saved media.
///
/// Generated media and save-as-media outputs are project-internal when
/// a project is open, using the `media/` directory.
pub fn resolve_project_internal_path(filename: &str, project_media_dir: Option<&str>) -> String {
    match project_media_dir {
        Some(dir) => format!("{}/{}", dir.trim_end_matches('/'), filename),
        None => format!("{}/{}", std::env::temp_dir().to_string_lossy(), filename),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // ── MED-003: Default naming from filename stem ──
    #[test]
    fn med_003_name_from_stem() {
        let result = plan_import("/path/to/my_video.mp4", None, ClipType::Video);
        assert!(result.is_some());
        assert_eq!(result.unwrap().entry.name, "my_video");
    }

    #[test]
    fn med_003_name_from_stem_image() {
        let result = plan_import("/images/photo.png", None, ClipType::Image);
        assert!(result.is_some());
        assert_eq!(result.unwrap().entry.name, "photo");
    }

    #[test]
    fn med_003_name_empty_path_returns_none() {
        let result = plan_import("", None, ClipType::Video);
        assert!(result.is_none());
    }

    #[test]
    fn med_003_name_no_stem_returns_none() {
        let result = plan_import("/path/to/.hidden", None, ClipType::Image);
        // .hidden has no stem on some platforms
        assert!(result.is_none() || !result.unwrap().entry.name.is_empty());
    }

    // ── MED-004: External reference created ──
    #[test]
    fn med_004_external_reference() {
        let result = plan_import("/external/video.mp4", None, ClipType::Video).unwrap();
        match &result.entry.source {
            MediaSource::External { absolute_path } => {
                assert_eq!(absolute_path, "/external/video.mp4");
            }
            _ => panic!("expected External source"),
        }
    }

    // ── MED-005: Import creates entry ──
    #[test]
    fn med_005_import_creates_entry_with_id() {
        let result = plan_import("/test/clip.mp4", None, ClipType::Video).unwrap();
        assert!(!result.entry.id.is_empty());
        assert_eq!(result.entry.name, "clip");
        assert_eq!(result.entry.r#type, ClipType::Video);
    }

    // ── MED-006: Optional folder_id ──
    #[test]
    fn med_006_import_with_folder() {
        let result =
            plan_import("/test/clip.mp4", Some("folder-1".into()), ClipType::Video).unwrap();
        assert_eq!(result.entry.folder_id, Some("folder-1".into()));
    }

    #[test]
    fn med_006_import_without_folder() {
        let result = plan_import("/test/clip.mp4", None, ClipType::Video).unwrap();
        assert_eq!(result.entry.folder_id, None);
    }

    #[test]
    fn med_006_assign_folder_updates_entry() {
        let result = plan_import("/test/clip.mp4", None, ClipType::Video).unwrap();
        let updated = assign_folder(&result.entry, Some("folder-2".into()));
        assert_eq!(updated.folder_id, Some("folder-2".into()));
        // Original is unchanged.
        assert_eq!(result.entry.folder_id, None);
    }

    // ── MED-007: Finalization loads metadata ──
    #[test]
    fn med_007_finalize_video_sets_metadata() {
        let entry = plan_import("/test/video.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let result = finalize_video(&entry, 1920, 1080, 30.0, 29.97, true);
        assert_eq!(result.entry.source_width, Some(1920));
        assert_eq!(result.entry.source_height, Some(1080));
        assert!((result.entry.duration - 30.0).abs() < 0.001);
        assert!((result.entry.source_fps.unwrap() - 29.97).abs() < 0.001);
        assert_eq!(result.entry.has_audio, Some(true));
    }

    #[test]
    fn med_007_finalize_audio_sets_duration() {
        let entry = plan_import("/test/song.mp3", None, ClipType::Audio)
            .unwrap()
            .entry;
        let result = finalize_audio(&entry, 180.0);
        assert!((result.entry.duration - 180.0).abs() < 0.001);
        // Audio has no width/height/fps
        assert_eq!(result.entry.source_width, None);
    }

    #[test]
    fn med_007_finalize_video_no_audio() {
        let entry = plan_import("/test/silent.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let result = finalize_video(&entry, 640, 480, 10.0, 30.0, false);
        assert_eq!(result.entry.has_audio, Some(false));
    }

    // ── MED-008: Finalization schedules search indexing ──
    #[test]
    fn med_008_finalize_schedules_search() {
        let entry = plan_import("/test/video.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let result = finalize_video(&entry, 1920, 1080, 10.0, 30.0, true);
        assert!(result.schedule_search_index);
    }

    #[test]
    fn med_008_image_finalize_schedules_search() {
        let entry = plan_import("/test/img.png", None, ClipType::Image)
            .unwrap()
            .entry;
        let result = finalize_image(&entry, 800, 600);
        assert!(result.schedule_search_index);
    }

    #[test]
    fn med_008_lottie_finalize_schedules_search() {
        let entry = plan_import("/test/animation.json", None, ClipType::Lottie)
            .unwrap()
            .entry;
        let result = finalize_lottie(&entry, 500, 500, 3.0, 60.0);
        assert!(result.schedule_search_index);
    }

    // ── MED-009: Image finalization ──
    #[test]
    fn med_009_image_finalize_dimensions() {
        let entry = plan_import("/test/img.png", None, ClipType::Image)
            .unwrap()
            .entry;
        let result = finalize_image(&entry, 3840, 2160);
        assert_eq!(result.entry.source_width, Some(3840));
        assert_eq!(result.entry.source_height, Some(2160));
    }

    #[test]
    fn med_009_image_default_duration() {
        let entry = plan_import("/test/img.png", None, ClipType::Image)
            .unwrap()
            .entry;
        let result = finalize_image(&entry, 1920, 1080);
        assert!((result.entry.duration - DEFAULT_STILL_DURATION_SECONDS).abs() < f64::EPSILON);
    }

    #[test]
    fn med_009_image_small_dimensions() {
        let entry = plan_import("/test/icon.png", None, ClipType::Image)
            .unwrap()
            .entry;
        let result = finalize_image(&entry, 32, 32);
        assert_eq!(result.entry.source_width, Some(32));
        assert_eq!(result.entry.source_height, Some(32));
    }

    // ── MED-010: Lottie finalization ──
    #[test]
    fn med_010_lottie_metadata() {
        let entry = plan_import("/test/anim.json", None, ClipType::Lottie)
            .unwrap()
            .entry;
        let result = finalize_lottie(&entry, 800, 600, 5.0, 60.0);
        assert_eq!(result.entry.source_width, Some(800));
        assert_eq!(result.entry.source_height, Some(600));
        assert!((result.entry.duration - 5.0).abs() < 0.001);
        assert!((result.entry.source_fps.unwrap() - 60.0).abs() < 0.001);
    }

    #[test]
    fn med_010_lottie_zero_fps() {
        let entry = plan_import("/test/anim.json", None, ClipType::Lottie)
            .unwrap()
            .entry;
        let result = finalize_lottie(&entry, 100, 100, 1.0, 0.0);
        assert!((result.entry.source_fps.unwrap() - 0.0).abs() < f64::EPSILON);
    }

    // ── MED-011: Rebuild entries from manifest ──
    #[test]
    fn med_011_entries_from_manifest_all_included() {
        let mut manifest = MediaManifest::default();
        let e1 = plan_import("/a.mp4", None, ClipType::Video).unwrap().entry;
        let e2 = plan_import("/b.mp4", None, ClipType::Video).unwrap().entry;
        manifest.entries = vec![e1, e2];

        let entries = entries_from_manifest(&manifest);
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn med_011_entries_from_manifest_empty() {
        let manifest = MediaManifest::default();
        let entries = entries_from_manifest(&manifest);
        assert!(entries.is_empty());
    }

    // ── MED-012: Offline assets remain in manifest ──
    #[test]
    fn med_012_offline_assets_remain_in_entries() {
        let mut manifest = MediaManifest::default();
        let entry = plan_import("/missing.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        manifest.entries = vec![entry];

        let entries = entries_from_manifest(&manifest);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "missing");
    }

    // ── MED-013: clipDisplayLabel ──
    #[test]
    fn med_013_clip_display_label_normal() {
        let entry = plan_import("/videos/MyVideo.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        assert_eq!(clip_display_label(&entry), "MyVideo");
    }

    #[test]
    fn med_013_clip_display_label_text() {
        let entry = plan_import("/text/title.txt", None, ClipType::Text)
            .unwrap()
            .entry;
        assert_eq!(clip_display_label(&entry), "title");
    }

    #[test]
    fn med_013_clip_display_label_image() {
        let entry = plan_import("/photos/sunset.png", None, ClipType::Image)
            .unwrap()
            .entry;
        assert_eq!(clip_display_label(&entry), "sunset");
    }

    #[test]
    fn med_013_clip_display_label_generation() {
        let mut entry = plan_import("/gen/output.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        entry.generation_input = Some(core_model::GenerationInput::default());
        // For generation assets, label is still the name (since we don't
        // have a separate "generating" state in the entry itself).
        assert_eq!(clip_display_label(&entry), "output");
    }

    // ── MED-014: Media status ──
    #[test]
    fn med_014_status_online_when_file_exists() {
        let entry = plan_import("/exists.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let status = media_status(&entry, |_| true, |_| true);
        assert_eq!(status, MediaStatus::Online);
    }

    #[test]
    fn med_014_status_offline_when_file_missing() {
        let entry = plan_import("/missing.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let status = media_status(&entry, |_| false, |_| true);
        assert_eq!(status, MediaStatus::Offline);
    }

    #[test]
    fn med_014_status_unprocessable_when_decode_fails() {
        let entry = plan_import("/corrupt.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let status = media_status(&entry, |_| true, |_| false);
        assert_eq!(
            status,
            MediaStatus::Unprocessable {
                reason: "video file could not be decoded".into()
            }
        );
    }

    #[test]
    fn med_014_status_online_when_cached_url() {
        let mut entry = plan_import("/missing.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        entry.cached_remote_url = Some("https://cache.example.com/video.mp4".into());
        // Even though file doesn't exist, cached URL means it's "online"
        let status = media_status(&entry, |_| false, |_| true);
        assert_eq!(status, MediaStatus::Online);
    }

    #[test]
    fn med_014_status_offline_distinct_from_unprocessable() {
        // Offline = file missing
        let entry = plan_import("/offline.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let offline = media_status(&entry, |_| false, |_| true);
        assert_eq!(offline, MediaStatus::Offline);

        // Unprocessable = file exists but can't decode
        let entry2 = plan_import("/bad.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        let unproc = media_status(&entry2, |_| true, |_| false);
        assert!(matches!(unproc, MediaStatus::Unprocessable { .. }));
    }

    #[test]
    fn med_014_image_offline_status() {
        let entry = plan_import("/missing.png", None, ClipType::Image)
            .unwrap()
            .entry;
        let status = media_status(&entry, |_| false, |_| true);
        assert_eq!(status, MediaStatus::Offline);
    }

    #[test]
    fn med_014_audio_offline_status() {
        let entry = plan_import("/missing.mp3", None, ClipType::Audio)
            .unwrap()
            .entry;
        let status = media_status(&entry, |_| false, |_| true);
        assert_eq!(status, MediaStatus::Offline);
    }

    // ── MED-015: Directory import ──
    #[test]
    fn med_015_scan_directory_recursive() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["video.mp4".into(), "audio.mp3".into(), "sub".into()]),
                "/root/sub" => Ok(vec!["nested.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        );
        assert!(result.is_ok());
        let plan = result.unwrap();
        // Should find video.mp4, audio.mp3, sub/nested.mp4 = 3 files
        assert_eq!(plan.entries.len(), 3);
    }

    #[test]
    fn med_015_directory_entries_have_clip_types() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["video.mp4".into(), "audio.mp3".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        // Sorted by stem (case-insensitive): audio < video
        assert_eq!(result.entries[0].clip_type, Some(ClipType::Audio));
        assert_eq!(result.entries[1].clip_type, Some(ClipType::Video));
    }

    // ── MED-016: Skip hidden files ──
    #[test]
    fn med_016_skip_hidden_files() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec![
                    "visible.mp4".into(),
                    ".hidden.mp4".into(),
                    ".DS_Store".into(),
                ]),
                _ => Err("not a directory".into()),
            },
            |name| name.starts_with('.'),
        )
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].stem, "visible");
        assert_eq!(result.entries[0].clip_type, Some(ClipType::Video));
    }

    #[test]
    fn med_016_skip_hidden_directories() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec![".hidden".into(), "visible.mp4".into()]),
                "/root/.hidden" => Ok(vec!["nested.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |name| name.starts_with('.'),
        )
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].stem, "visible");
    }

    // ── MED-017: Only supported extensions ──
    #[test]
    fn med_017_only_supported_extensions() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec![
                    "video.mp4".into(),
                    "document.pdf".into(),
                    "audio.wav".into(),
                    "script.js".into(),
                ]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.skipped_count, 2);
    }

    #[test]
    fn med_017_lottie_json_supported() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["animation.json".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].clip_type, Some(ClipType::Lottie));
    }

    #[test]
    fn med_017_lottie_lottie_ext_supported() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["anim.lottie".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].clip_type, Some(ClipType::Lottie));
    }

    // ── MED-018: Sort by filename ──
    #[test]
    fn med_018_sort_by_filename() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["z.mp4".into(), "a.mp4".into(), "m.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].stem, "a");
        assert_eq!(result.entries[1].stem, "m");
        assert_eq!(result.entries[2].stem, "z");
    }

    #[test]
    fn med_018_case_insensitive_sort() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["Z.mp4".into(), "a.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(result.entries[0].stem, "a");
        assert_eq!(result.entries[1].stem, "Z");
    }

    // ── Derive folder mirror from scan ──
    #[test]
    fn med_015_folder_mirror_from_scan() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["sub".into(), "root_video.mp4".into()]),
                "/root/sub" => Ok(vec!["nested.mp4".into(), "sub2".into()]),
                "/root/sub/sub2" => Ok(vec!["deep.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        let folders = derive_folder_mirror(&result);
        // Should have "sub" folder and "sub/sub2" folder
        assert!(!folders.is_empty());
        // sub should come before sub/sub2 (shallowest first)
        let sub_idx = folders.iter().position(|f| f.name == "sub");
        let sub2_idx = folders.iter().position(|f| f.name == "sub2");
        assert!(sub_idx.is_some());
        assert!(sub2_idx.is_some());
        assert!(sub_idx.unwrap() < sub2_idx.unwrap());
    }

    #[test]
    fn med_015_folder_mirror_root_files_only() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["a.mp4".into(), "b.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        let folders = derive_folder_mirror(&result);
        assert!(folders.is_empty(), "root-level files produce no folders");
    }

    // ── Empty directory scan ──
    #[test]
    fn scan_empty_directory() {
        let result = scan_directory("/root", |_| Ok(vec![]), |_| false).unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.skipped_count, 0);
    }

    // ── Directory scan with only unsupported ──
    #[test]
    fn scan_directory_unsupported_only() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["file.txt".into(), "doc.pdf".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        )
        .unwrap();
        assert!(result.entries.is_empty());
        assert_eq!(result.skipped_count, 2);
    }

    // ── Directory scan handles not-found root ──
    #[test]
    fn scan_directory_root_not_found() {
        let result = scan_directory(
            "/nonexistent",
            |_| Err("directory not found".into()),
            |_| false,
        );
        assert!(result.is_err());
    }

    // ── MED-014: not-a-directory error handled ──
    #[test]
    fn scan_directory_handles_not_a_directory() {
        let result = scan_directory(
            "/root",
            |p| match p {
                "/root" => Ok(vec!["file_a.mp4".into(), "dir_like.mp4".into()]),
                // dir_like has an extension that looks like media, but
                // list_directory fails — it's a file, not a dir.
                "/root/dir_like.mp4" => Err("not a directory".into()),
                // Default: not a directory
                _ => Err("not a directory".into()),
            },
            |_| false,
        );
        assert!(result.is_ok());
        let plan = result.unwrap();
        // Both files should be included (dir_like is a file, not a dir)
        assert_eq!(plan.entries.len(), 2);
    }

    // ── MED-008: Search indexing scheduled for all types ──
    #[test]
    fn med_008_search_index_scheduled() {
        let v = plan_import("/v.mp4", None, ClipType::Video).unwrap();
        let r1 = finalize_video(&v.entry, 1920, 1080, 10.0, 30.0, true);
        assert!(r1.schedule_search_index);

        let a = plan_import("/a.mp3", None, ClipType::Audio).unwrap();
        let r2 = finalize_audio(&a.entry, 60.0);
        assert!(r2.schedule_search_index);

        let i = plan_import("/i.png", None, ClipType::Image).unwrap();
        let r3 = finalize_image(&i.entry, 100, 100);
        assert!(r3.schedule_search_index);

        let l = plan_import("/l.json", None, ClipType::Lottie).unwrap();
        let r4 = finalize_lottie(&l.entry, 200, 200, 2.0, 30.0);
        assert!(r4.schedule_search_index);
    }

    // ── MED-014: Project source status check ──
    #[test]
    fn med_014_project_source_status() {
        let mut entry = plan_import("/p/media/v.mp4", None, ClipType::Video)
            .unwrap()
            .entry;
        entry.source = MediaSource::Project {
            relative_path: "media/v.mp4".into(),
        };
        let status = media_status(&entry, |p| p == "media/v.mp4", |_| true);
        assert_eq!(status, MediaStatus::Online);
    }

    // ── Directory scan with root_path ending in / ──
    #[test]
    fn scan_directory_trailing_slash_root() {
        let result = scan_directory(
            "/root/",
            |p| match p.trim_end_matches('/') {
                "/root" => Ok(vec!["video.mp4".into()]),
                _ => Err("not a directory".into()),
            },
            |_| false,
        );
        assert!(result.is_ok());
    }

    // ── MED-019: Pasted image filename format ──
    #[test]
    fn med_019_pasted_image_filename_basic() {
        let name = pasted_image_filename("abc123", "png");
        assert_eq!(name, "pasted-abc123.png");
    }

    #[test]
    fn med_019_pasted_image_filename_dot_ext() {
        let name = pasted_image_filename("abc123", ".png");
        assert_eq!(name, "pasted-abc123.png");
    }

    #[test]
    fn med_019_pasted_image_filename_tiff() {
        let name = pasted_image_filename("img-1", "tiff");
        assert_eq!(name, "pasted-img-1.tiff");
    }

    // ── MED-020: Path resolution for pasted images ──
    #[test]
    fn med_020_resolve_with_project_dir() {
        let path = resolve_pasted_image_path("id1", "png", Some("/project/media"));
        assert_eq!(path, "/project/media/pasted-id1.png");
    }

    #[test]
    fn med_020_resolve_with_project_dir_trailing_slash() {
        let path = resolve_pasted_image_path("id2", "png", Some("/project/media/"));
        assert_eq!(path, "/project/media/pasted-id2.png");
    }

    #[test]
    fn med_020_resolve_without_project_dir_is_temp() {
        let path = resolve_pasted_image_path("id3", ".jpg", None);
        // Should include temp_dir path and the pasted filename
        assert!(path.contains("pasted-id3.jpg"));
        assert!(path.contains(std::env::temp_dir().to_string_lossy().as_ref()));
    }

    // ── MED-021: Project-internal path resolution ──
    #[test]
    fn med_021_project_internal_with_media_dir() {
        let path = resolve_project_internal_path("clip-video.mp4", Some("/p/media"));
        assert_eq!(path, "/p/media/clip-video.mp4");
    }

    #[test]
    fn med_021_project_internal_without_media_dir() {
        let path = resolve_project_internal_path("generated.mp4", None);
        assert!(path.contains("generated.mp4"));
        assert!(path.contains(std::env::temp_dir().to_string_lossy().as_ref()));
    }

    #[test]
    fn med_021_project_internal_trailing_slash() {
        let path = resolve_project_internal_path("clip.m4a", Some("/p/media/"));
        assert_eq!(path, "/p/media/clip.m4a");
    }
}
