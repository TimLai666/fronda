use core_model::{MediaManifest, MediaManifestEntry, MediaSource};
use std::collections::HashSet;

/// A single media collection instruction for bundle export.
#[derive(Debug, Clone, PartialEq)]
pub struct BundleMediaEntry {
    /// The media manifest entry id
    pub entry_id: String,
    /// Absolute source path (present for External sources)
    pub source_path: Option<String>,
    /// Target filename within the bundle's `media/` directory
    pub target_filename: String,
}

/// The planned state before I/O execution.
/// BND-002: The exported bundle includes timeline JSON, media manifest,
/// generation log, and collected media.
/// BND-003: Resolvable source media are copied into the bundle's `media/` directory.
/// BND-004: Copied media are rewritten in the exported manifest as project-relative sources.
/// BND-006: Multiple references to the same external source file are deduplicated.
#[derive(Debug, Clone)]
pub struct BundleExportPlan {
    /// Media entries to copy (deduplicated by absolute path).
    pub entries_to_copy: Vec<BundleMediaEntry>,
    /// The rewritten manifest with project-relative paths for collected media.
    pub rewritten_manifest: MediaManifest,
}

/// Report after bundle export execution.
/// BND-005: Missing or uncollectable media are reported.
/// BND-007: The export report distinguishes collected media from missing media.
#[derive(Debug, Clone, Default)]
pub struct BundleExportReport {
    /// Entry IDs of successfully collected media
    pub collected: Vec<String>,
    /// Entry IDs of media that could not be collected
    pub missing: Vec<String>,
    /// Total source entries considered
    pub total_entries: usize,
    /// Number of deduplicated files (additional refs to same file)
    pub deduplicated_files: usize,
}

impl BundleExportReport {
    pub fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }

    pub fn total_collected(&self) -> usize {
        self.collected.len()
    }

    pub fn total_missing(&self) -> usize {
        self.missing.len()
    }
}

/// BND-001: Plan a self-contained `.palmier` bundle export.
///
/// Examines the manifest, identifies external media files to collect,
/// deduplicates by absolute path, and produces a plan plus a rewritten
/// manifest with project-relative paths.
pub fn plan_bundle_export(manifest: &MediaManifest, bundle_media_dir: &str) -> BundleExportPlan {
    let mut entries_to_copy: Vec<BundleMediaEntry> = Vec::new();
    let mut rewritten_entries: Vec<MediaManifestEntry> = Vec::new();
    let mut seen_absolute_paths: HashSet<String> = HashSet::new();

    for entry in &manifest.entries {
        match &entry.source {
            MediaSource::External { absolute_path } => {
                let target_filename =
                    filename_for_external(absolute_path, &mut seen_absolute_paths);

                // Plan the copy instruction
                entries_to_copy.push(BundleMediaEntry {
                    entry_id: entry.id.clone(),
                    source_path: Some(absolute_path.clone()),
                    target_filename: target_filename.clone(),
                });

                // Rewrite to project-relative
                let relative_path = format!("{}/{}", bundle_media_dir, target_filename);
                rewritten_entries.push(MediaManifestEntry {
                    source: MediaSource::Project { relative_path },
                    ..entry.clone()
                });
            }
            MediaSource::Project { .. } => {
                // Already project-relative, keep as-is
                rewritten_entries.push(entry.clone());
            }
        }
    }

    BundleExportPlan {
        entries_to_copy,
        rewritten_manifest: MediaManifest {
            version: manifest.version,
            entries: rewritten_entries,
            folders: manifest.folders.clone(),
        },
    }
}

/// Generate a unique filename for an external source path, deduplicating
/// by returning the same filename when the same absolute path is seen again.
///
/// BND-006: Multiple references to the same external source file are deduplicated.
fn filename_for_external(absolute_path: &str, seen_paths: &mut HashSet<String>) -> String {
    // Derive a filename from the path: sanitize the absolute path to a flat name
    let sanitized = absolute_path.replace([':', '/', '\\'], "_");

    // If we've seen this path before, reuse the same filename (dedup)
    if seen_paths.contains(absolute_path) {
        // Return the same filename that was assigned first time
        // (computed deterministically from the path)
        sanitized
    } else {
        seen_paths.insert(absolute_path.to_string());
        sanitized
    }
}

/// BND-007: Build a report distinguishing collected from missing media.
///
/// The `file_exists` callback checks whether a source file exists on disk.
pub fn build_bundle_report(
    plan: &BundleExportPlan,
    file_exists: impl Fn(&str) -> bool,
) -> BundleExportReport {
    let total_entries = plan.entries_to_copy.len();
    let mut collected = Vec::new();
    let mut missing = Vec::new();
    let mut seen_source_paths: HashSet<String> = HashSet::new();
    let mut deduplicated_files = 0;

    for entry in &plan.entries_to_copy {
        match &entry.source_path {
            Some(path) if file_exists(path) => {
                collected.push(entry.entry_id.clone());
                if !seen_source_paths.insert(path.clone()) {
                    deduplicated_files += 1;
                }
            }
            _ => {
                missing.push(entry.entry_id.clone());
            }
        }
    }

    BundleExportReport {
        collected,
        missing,
        total_entries,
        deduplicated_files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::ClipType;

    fn make_entry(id: &str, external_path: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: format!("entry-{id}"),
            r#type: ClipType::Video,
            source: MediaSource::External {
                absolute_path: external_path.to_string(),
            },
            duration: 10.0,
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
        }
    }

    fn make_project_entry(id: &str, relative_path: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            source: MediaSource::Project {
                relative_path: relative_path.to_string(),
            },
            ..make_entry(id, "/ignored")
        }
    }

    fn manifest_with(entries: Vec<MediaManifestEntry>) -> MediaManifest {
        MediaManifest {
            version: 2,
            entries,
            folders: vec![],
        }
    }

    // ── BND-001: Plan produces entries for external media ──
    #[test]
    fn bnd_001_plan_produces_copy_entries_for_external() {
        let manifest = manifest_with(vec![make_entry("a", "/Users/test/video1.mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        assert_eq!(plan.entries_to_copy.len(), 1);
        assert_eq!(plan.entries_to_copy[0].entry_id, "a");
        assert_eq!(
            plan.entries_to_copy[0].source_path.as_deref(),
            Some("/Users/test/video1.mp4")
        );
    }

    // ── BND-002: Plan output includes rewritten manifest entries ──
    #[test]
    fn bnd_002_rewritten_manifest_included() {
        let manifest = manifest_with(vec![make_entry("a", "/Users/test/video1.mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        assert_eq!(plan.rewritten_manifest.entries.len(), 1);
        // The rewritten manifest should use Project source
        match &plan.rewritten_manifest.entries[0].source {
            MediaSource::Project { relative_path } => {
                assert!(
                    relative_path.starts_with("media/"),
                    "expected media/ prefix, got {relative_path}"
                );
            }
            other => panic!("expected Project source, got {other:?}"),
        }
    }

    // ── BND-003: External media gets a copy instruction ──
    #[test]
    fn bnd_003_external_media_gets_copy_instruction() {
        let manifest = manifest_with(vec![make_entry("vid", "/movies/clip.mov")]);
        let plan = plan_bundle_export(&manifest, "media");
        assert_eq!(plan.entries_to_copy.len(), 1);
        assert!(plan.entries_to_copy[0].source_path.is_some());
    }

    // ── BND-004: Copied media rewritten to project-relative ──
    #[test]
    fn bnd_004_rewritten_as_project_relative() {
        let manifest = manifest_with(vec![make_entry("a", "/Users/test/video.mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        let rewritten = &plan.rewritten_manifest.entries[0];
        match &rewritten.source {
            MediaSource::Project { relative_path } => {
                assert_eq!(relative_path, "media/_Users_test_video.mp4");
            }
            other => panic!("expected Project source, got {other:?}"),
        }
    }

    // ── BND-005: Missing media reported in report ──
    #[test]
    fn bnd_005_missing_media_reported() {
        let manifest = manifest_with(vec![make_entry("a", "/nonexistent/file.mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        let report = build_bundle_report(&plan, |_| false);
        assert!(report.has_missing());
        assert_eq!(report.missing, vec!["a"]);
        assert!(report.collected.is_empty());
    }

    // ── BND-006: Duplicate external paths deduplicated ──
    #[test]
    fn bnd_006_duplicate_paths_deduplicated() {
        let manifest = manifest_with(vec![
            make_entry("a", "/shared/file.mp4"),
            make_entry("b", "/shared/file.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        // Both entries produce copy instructions
        assert_eq!(plan.entries_to_copy.len(), 2);
        // Both point to the same target filename
        assert_eq!(
            plan.entries_to_copy[0].target_filename,
            plan.entries_to_copy[1].target_filename
        );
        // Both rewritten as project-relative with same filename
        let rel_a = match &plan.rewritten_manifest.entries[0].source {
            MediaSource::Project { relative_path } => relative_path.clone(),
            _ => panic!("expected project source"),
        };
        let rel_b = match &plan.rewritten_manifest.entries[1].source {
            MediaSource::Project { relative_path } => relative_path.clone(),
            _ => panic!("expected project source"),
        };
        assert_eq!(
            rel_a, rel_b,
            "deduplicated entries should share the same relative path"
        );
    }

    // ── BND-007: Report distinguishes collected from missing ──
    #[test]
    fn bnd_007_report_distinguishes_collected_from_missing() {
        let manifest = manifest_with(vec![
            make_entry("exists", "/tmp/existing.mp4"),
            make_entry("missing", "/tmp/ghost.mp4"),
            make_entry("also_exists", "/tmp/another.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        let report = build_bundle_report(&plan, |p| {
            p == "/tmp/existing.mp4" || p == "/tmp/another.mp4"
        });
        assert_eq!(report.collected, vec!["exists", "also_exists"]);
        assert_eq!(report.missing, vec!["missing"]);
        assert_eq!(report.total_entries, 3);
    }

    // ── BND-007: No missing means empty missing list ──
    #[test]
    fn bnd_007_all_collected_no_missing() {
        let manifest = manifest_with(vec![
            make_entry("a", "/tmp/a.mp4"),
            make_entry("b", "/tmp/b.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        let report = build_bundle_report(&plan, |_| true);
        assert!(!report.has_missing());
        assert_eq!(report.collected.len(), 2);
        assert_eq!(report.missing.len(), 0);
    }

    // ── Empty manifest plan ──
    #[test]
    fn empty_manifest_plan() {
        let manifest = manifest_with(vec![]);
        let plan = plan_bundle_export(&manifest, "media");
        assert!(plan.entries_to_copy.is_empty());
        assert!(plan.rewritten_manifest.entries.is_empty());
    }

    // ── Project-relative entries pass through unchanged ──
    #[test]
    fn project_entries_pass_through() {
        let manifest = manifest_with(vec![make_project_entry("p1", "generated/clip.mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        // No copy needed for project-relative entries
        assert!(plan.entries_to_copy.is_empty());
        // Rewritten manifest keeps the original project source
        assert_eq!(plan.rewritten_manifest.entries.len(), 1);
        match &plan.rewritten_manifest.entries[0].source {
            MediaSource::Project { relative_path } => {
                assert_eq!(relative_path, "generated/clip.mp4");
            }
            _ => panic!("expected Project source"),
        }
    }

    // ── Mixed external and project entries ──
    #[test]
    fn mixed_external_and_project_entries() {
        let manifest = manifest_with(vec![
            make_entry("ext", "/external/video.mp4"),
            make_project_entry("proj", "generated/video.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        assert_eq!(plan.entries_to_copy.len(), 1); // only external
        assert_eq!(plan.entries_to_copy[0].entry_id, "ext");
        assert_eq!(plan.rewritten_manifest.entries.len(), 2);
    }

    // ── Deduplication counts in report ──
    #[test]
    fn dedup_count_in_report() {
        let manifest = manifest_with(vec![
            make_entry("a", "/shared/file.mp4"),
            make_entry("b", "/shared/file.mp4"),
            make_entry("c", "/unique/file.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        let report = build_bundle_report(&plan, |_| true);
        // a is first seen, b is deduplicated (same path seen before), c is unique
        assert_eq!(report.deduplicated_files, 1);
        assert_eq!(report.total_entries, 3);
        assert_eq!(report.collected.len(), 3);
    }

    // ── Missing with some existing ──
    #[test]
    fn partial_missing() {
        let manifest = manifest_with(vec![
            make_entry("a", "/exists/a.mp4"),
            make_entry("b", "/missing/b.mp4"),
        ]);
        let plan = plan_bundle_export(&manifest, "media");
        let report = build_bundle_report(&plan, |p| p == "/exists/a.mp4");
        assert!(report.has_missing());
        assert_eq!(report.collected, vec!["a"]);
        assert_eq!(report.missing, vec!["b"]);
    }

    // ── Filename generation handles special chars ──
    #[test]
    fn filename_from_path_with_special_chars() {
        let manifest = manifest_with(vec![make_entry("a", "C:\\Users\\test\\my video (1).mp4")]);
        let plan = plan_bundle_export(&manifest, "media");
        let filename = &plan.entries_to_copy[0].target_filename;
        assert!(
            filename.contains("my video (1).mp4"),
            "filename should preserve special chars, got: {filename}"
        );
        // Windows paths should be sanitized
        assert!(!filename.contains('\\'), "backslashes should be replaced");
        assert!(!filename.contains(':'), "colons should be replaced");
    }
}
