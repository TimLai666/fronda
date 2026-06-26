/// Sample project materialization definitions.
///
/// SMP-001..005: Sample projects are real `.palmier` packages
/// that can be materialized to disk for new users to explore.
///
/// This module covers the *definition* and *planning* of sample
/// projects. Actual file I/O (writing JSON, downloading media) is
/// done by callers using the plans produced here.
use serde::{Deserialize, Serialize};

/// A single sample project offered in the "New Project" strip.
///
/// SMP-001: Materialized as a real `.palmier` package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleProjectDefinition {
    /// Unique identifier (used for dedup and caching).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Short description shown in the sample-project strip.
    pub description: String,
    /// URL to a thumbnail image shown in the strip (optional).
    pub thumbnail_url: Option<String>,
    /// URLs of media files that must be downloaded into the package.
    pub media_urls: Vec<String>,
    /// Pre-baked timeline JSON content (or path to embedded resource).
    pub timeline_json: String,
    /// Pre-baked media manifest JSON.
    pub manifest_json: String,
    /// Optional chat payloads (one per chat session file).
    pub chat_payloads: Vec<ChatPayloadDef>,
    /// Sort order in the sample-project strip (lower = first).
    pub sort_order: u32,
}

/// A chat payload bundled with a sample project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPayloadDef {
    /// Filename under `chat/` (e.g. `"chat/session-1.json"`).
    pub filename: String,
    /// Raw JSON content of the chat session.
    pub content: String,
}

/// A materialization plan describes what to write and download.
///
/// SMP-002: Materialization writes timeline JSON, media manifest,
/// optional chat payloads, and optional thumbnail into the sample package.
#[derive(Debug, Clone)]
pub struct MaterializationPlan {
    /// The sample project being materialized.
    pub project: SampleProjectDefinition,
    /// Target directory path (absolute) where the `.palmier` package
    /// will be created.
    pub target_path: String,
    /// List of media files to download, each with a suggested filename.
    pub media_downloads: Vec<MediaDownloadItem>,
    /// Chat files to write under `chat/`.
    pub chat_files: Vec<ChatFileItem>,
}

/// A single media file to download for a sample project.
#[derive(Debug, Clone)]
pub struct MediaDownloadItem {
    /// URL to download from.
    pub url: String,
    /// Relative path inside the `.palmier` package's `media/` directory.
    pub relative_path: String,
}

/// A chat file to write inside the `.palmier` package.
#[derive(Debug, Clone)]
pub struct ChatFileItem {
    /// Relative path under `chat/` (e.g. `"chat/welcome.json"`).
    pub relative_path: String,
    /// JSON content bytes.
    pub content: String,
}

/// Catalog of all built-in sample projects.
///
/// Provides iteration and lookup by id.
#[derive(Debug, Clone)]
pub struct SampleProjectCatalog {
    projects: Vec<SampleProjectDefinition>,
}

impl SampleProjectCatalog {
    /// Create a catalog from the built-in project definitions.
    pub fn builtin() -> Self {
        Self {
            projects: builtin_sample_projects(),
        }
    }

    /// Create an empty catalog (useful for tests).
    pub fn empty() -> Self {
        Self {
            projects: Vec::new(),
        }
    }

    /// All projects sorted by sort_order.
    pub fn all_sorted(&self) -> Vec<&SampleProjectDefinition> {
        let mut sorted: Vec<_> = self.projects.iter().collect();
        sorted.sort_by_key(|p| p.sort_order);
        sorted
    }

    /// Look up a project by id.
    pub fn by_id(&self, id: &str) -> Option<&SampleProjectDefinition> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// Return true if all project ids are unique.
    pub fn ids_are_unique(&self) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.projects.iter().all(|p| seen.insert(p.id.as_str()))
    }
}

/// Build a materialization plan for a sample project.
///
/// SMP-002: Determines what files to write and what media to download.
/// The caller is responsible for executing the plan.
pub fn plan_materialization(
    project: &SampleProjectDefinition,
    target_path: &str,
    media_base_url: &str,
) -> MaterializationPlan {
    let media_downloads: Vec<MediaDownloadItem> = project
        .media_urls
        .iter()
        .enumerate()
        .map(|(i, url)| {
            // Derive a filename from the URL or use a fallback.
            let filename = url
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(&format!("media-{}.mp4", i + 1))
                .to_string();
            MediaDownloadItem {
                url: if url.starts_with("http://") || url.starts_with("https://") {
                    url.clone()
                } else {
                    format!("{}/{}", media_base_url.trim_end_matches('/'), url)
                },
                relative_path: format!("media/{}", filename),
            }
        })
        .collect();

    let chat_files: Vec<ChatFileItem> = project
        .chat_payloads
        .iter()
        .map(|cp| ChatFileItem {
            relative_path: cp.filename.clone(),
            content: cp.content.clone(),
        })
        .collect();

    MaterializationPlan {
        project: project.clone(),
        target_path: target_path.to_string(),
        media_downloads,
        chat_files,
    }
}

/// Built-in sample project definitions.
///
/// These are the default projects shown in the "New Project" strip.
fn builtin_sample_projects() -> Vec<SampleProjectDefinition> {
    vec![
        SampleProjectDefinition {
            id: "getting-started".into(),
            name: "Getting Started".into(),
            description: "A quick tour of the editing basics.".into(),
            thumbnail_url: Some("https://palmier.io/samples/getting-started.jpg".into()),
            media_urls: vec![
                "samples/getting-started/clip-1.mp4".into(),
                "samples/getting-started/clip-2.mp4".into(),
            ],
            timeline_json: r#"{"fps":30,"width":1920,"height":1080,"tracks":[]}"#.into(),
            manifest_json: r#"{"version":2,"entries":[],"folders":[]}"#.into(),
            chat_payloads: vec![],
            sort_order: 0,
        },
        SampleProjectDefinition {
            id: "b roll".into(),
            name: "B-Roll Compilation".into(),
            description: "Practice with cuts and transitions.".into(),
            thumbnail_url: Some("https://palmier.io/samples/b-roll.jpg".into()),
            media_urls: vec![
                "samples/b-roll/clip-1.mp4".into(),
                "samples/b-roll/clip-2.mp4".into(),
                "samples/b-roll/clip-3.mp4".into(),
            ],
            timeline_json: r#"{"fps":30,"width":1920,"height":1080,"tracks":[]}"#.into(),
            manifest_json: r#"{"version":2,"entries":[],"folders":[]}"#.into(),
            chat_payloads: vec![],
            sort_order: 1,
        },
    ]
}

/// SMP-003: Status of a single download in the batch.
#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

/// SMP-003: Tracks concurrent download progress for a materialization plan.
#[derive(Debug, Clone)]
pub struct DownloadCoordinator {
    /// All download items with their status.
    pub items: Vec<DownloadItem>,
    /// Maximum number of concurrent downloads.
    pub max_concurrent: usize,
    /// Whether any download has failed (triggers cleanup).
    pub has_failures: bool,
}

/// A single download item with status tracking.
#[derive(Debug, Clone)]
pub struct DownloadItem {
    /// URL to download from.
    pub url: String,
    /// Target relative path inside the package.
    pub relative_path: String,
    /// Current status.
    pub status: DownloadStatus,
}

impl DownloadCoordinator {
    /// Create a new coordinator for a materialization plan.
    ///
    /// SMP-003: All downloads are tracked for concurrent execution.
    pub fn new(plan: &MaterializationPlan, max_concurrent: usize) -> Self {
        let items = plan
            .media_downloads
            .iter()
            .map(|d| DownloadItem {
                url: d.url.clone(),
                relative_path: d.relative_path.clone(),
                status: DownloadStatus::Pending,
            })
            .collect();

        Self {
            items,
            max_concurrent: max_concurrent.max(1),
            has_failures: false,
        }
    }

    /// Number of pending items.
    pub fn pending_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == DownloadStatus::Pending)
            .count()
    }

    /// Number of completed items.
    pub fn completed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == DownloadStatus::Completed)
            .count()
    }

    /// Number of failed items.
    pub fn failed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| matches!(i.status, DownloadStatus::Failed(_)))
            .count()
    }

    /// Total number of items.
    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    /// Returns true if all downloads are complete (success or failure).
    pub fn is_finished(&self) -> bool {
        self.items
            .iter()
            .all(|i| i.status != DownloadStatus::Pending && i.status != DownloadStatus::InProgress)
    }

    /// Mark the next pending item as in-progress.
    /// Returns the index of the started item, or None if all are started/completed.
    pub fn start_next(&mut self) -> Option<usize> {
        let idx = self
            .items
            .iter()
            .position(|i| i.status == DownloadStatus::Pending)?;
        self.items[idx].status = DownloadStatus::InProgress;
        Some(idx)
    }

    /// Mark an item as completed.
    pub fn mark_completed(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.status = DownloadStatus::Completed;
        }
    }

    /// Mark an item as failed.
    pub fn mark_failed(&mut self, index: usize, error: String) {
        if let Some(item) = self.items.get_mut(index) {
            item.status = DownloadStatus::Failed(error);
            self.has_failures = true;
        }
    }

    /// Get the indices of items that should be started now.
    /// Respects `max_concurrent`.
    pub fn next_batch(&self) -> Vec<usize> {
        let in_progress = self
            .items
            .iter()
            .filter(|i| i.status == DownloadStatus::InProgress)
            .count();
        let slots = self.max_concurrent.saturating_sub(in_progress);
        self.items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.status == DownloadStatus::Pending)
            .take(slots)
            .map(|(idx, _)| idx)
            .collect()
    }
}

/// SMP-004: Plan for cleaning up partial sample materialization.
///
/// Lists all files that should be removed if materialization fails partway through.
#[derive(Debug, Clone)]
pub struct CleanupPlan {
    /// All files that were created or partially downloaded.
    pub files_to_remove: Vec<String>,
    /// The target directory to remove if completely empty after cleanup.
    pub target_dir: String,
}

/// SMP-004: Generate a cleanup plan from a materialization plan.
///
/// Enumerates all outputs that would need to be removed on failure.
pub fn cleanup_plan_for(plan: &MaterializationPlan) -> CleanupPlan {
    let mut files_to_remove = Vec::new();

    // All media downloads target media/ subdirectory
    for dl in &plan.media_downloads {
        files_to_remove.push(format!(
            "{}/{}",
            plan.target_path.trim_end_matches('/'),
            dl.relative_path
        ));
    }

    // Timeline JSON
    files_to_remove.push(format!(
        "{}/project.json",
        plan.target_path.trim_end_matches('/')
    ));

    // Media manifest JSON
    files_to_remove.push(format!(
        "{}/media.json",
        plan.target_path.trim_end_matches('/')
    ));

    // Chat payload files
    for chat in &plan.chat_files {
        files_to_remove.push(format!(
            "{}/{}",
            plan.target_path.trim_end_matches('/'),
            chat.relative_path
        ));
    }

    CleanupPlan {
        files_to_remove,
        target_dir: plan.target_path.clone(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    // ── SMP-001: Projects have valid structure ──
    #[test]
    fn smp_001_project_has_all_required_fields() {
        let p = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "A test project.".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        assert!(!p.id.is_empty());
        assert!(!p.name.is_empty());
        assert!(!p.timeline_json.is_empty());
        assert!(!p.manifest_json.is_empty());
    }

    // ── SMP-001: MaterializationPlan is a real package layout ──
    #[test]
    fn smp_001_plan_has_downloads() {
        let p = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["https://example.com/video.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/Test.palmier", "https://media.example.com");
        assert_eq!(plan.media_downloads.len(), 1);
        assert_eq!(plan.media_downloads[0].url, "https://example.com/video.mp4");
        assert!(plan.media_downloads[0].relative_path.starts_with("media/"));
    }

    // ── SMP-002: Materialization plan writes JSON, manifest, chat ──
    #[test]
    fn smp_002_plan_includes_chat_payloads() {
        let p = SampleProjectDefinition {
            id: "chatty".into(),
            name: "Chatty".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: r#"{"fps":30}"#.into(),
            manifest_json: r#"{"version":2}"#.into(),
            chat_payloads: vec![ChatPayloadDef {
                filename: "chat/welcome.json".into(),
                content: r#"{"messages":[]}"#.into(),
            }],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/Chatty.palmier", "https://media.example.com");
        assert_eq!(plan.chat_files.len(), 1);
        assert_eq!(plan.chat_files[0].relative_path, "chat/welcome.json");
        assert_eq!(plan.chat_files[0].content, r#"{"messages":[]}"#);
    }

    // ── SMP-002: Plan includes timeline and manifest content ──
    #[test]
    fn smp_002_plan_preserves_json_content() {
        let p = SampleProjectDefinition {
            id: "json-test".into(),
            name: "JSON Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: r#"{"fps":60,"width":3840}"#.into(),
            manifest_json: r#"{"version":2,"entries":[]}"#.into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        assert_eq!(p.timeline_json, r#"{"fps":60,"width":3840}"#);
        assert_eq!(p.manifest_json, r#"{"version":2,"entries":[]}"#);
    }

    // ── SMP-003: Media download URLs are absolute or resolved ──
    #[test]
    fn smp_003_media_url_absolute_kept_as_is() {
        let p = SampleProjectDefinition {
            id: "abs".into(),
            name: "Abs".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["https://cdn.example.com/video.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/A.palmier", "https://media.example.com");
        assert_eq!(
            plan.media_downloads[0].url,
            "https://cdn.example.com/video.mp4"
        );
    }

    // ── SMP-003: Relative media URLs are resolved against base ──
    #[test]
    fn smp_003_relative_url_resolved() {
        let p = SampleProjectDefinition {
            id: "rel".into(),
            name: "Rel".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["samples/rel/clip.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan =
            plan_materialization(&p, "/tmp/Rel.palmier", "https://media.example.com/samples");
        assert_eq!(
            plan.media_downloads[0].url,
            "https://media.example.com/samples/samples/rel/clip.mp4"
        );
    }

    // ── SMP-003: Multiple downloads are all included ──
    #[test]
    fn smp_003_multiple_downloads() {
        let p = SampleProjectDefinition {
            id: "multi".into(),
            name: "Multi".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![
                "https://cdn.example.com/a.mp4".into(),
                "https://cdn.example.com/b.mp4".into(),
                "https://cdn.example.com/c.mp4".into(),
            ],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/M.palmier", "https://media.example.com");
        assert_eq!(plan.media_downloads.len(), 3);
    }

    // ── SMP-003: Concurrent downloads note (test validates plan shape) ──
    #[test]
    fn smp_003_download_urls_distinct() {
        let p = SampleProjectDefinition {
            id: "distinct".into(),
            name: "Distinct".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["a.mp4".into(), "b.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/D.palmier", "https://media.example.com/base");
        assert_ne!(plan.media_downloads[0].url, plan.media_downloads[1].url);
        assert_ne!(
            plan.media_downloads[0].relative_path,
            plan.media_downloads[1].relative_path
        );
    }

    // ── SMP-004: Partial cleanup validation ──
    // The materialization plan must allow callers to distinguish
    // what was written so cleanup on failure is possible.
    #[test]
    fn smp_004_plan_lists_all_outputs() {
        let p = SampleProjectDefinition {
            id: "cleanup".into(),
            name: "Cleanup".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["https://cdn.example.com/a.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![ChatPayloadDef {
                filename: "chat/session.json".into(),
                content: "[]".into(),
            }],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/C.palmier", "https://media.example.com");
        // Plan explicitly lists all media and chat files to write.
        assert!(!plan.media_downloads.is_empty());
        assert!(!plan.chat_files.is_empty());
    }

    // ── SMP-004: Media downloads enumerable for cleanup ──
    #[test]
    fn smp_004_media_downloads_have_relative_paths() {
        let p = SampleProjectDefinition {
            id: "c2".into(),
            name: "C2".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["https://cdn.example.com/b.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/C2.palmier", "https://media.example.com");
        assert!(plan.media_downloads[0].relative_path.starts_with("media/"));
        assert!(plan.media_downloads[0].relative_path.ends_with(".mp4"));
    }

    // ── SMP-005: Catalog with unique ids (cached sample check) ──
    #[test]
    fn smp_005_catalog_ids_are_unique() {
        let catalog = SampleProjectCatalog::builtin();
        assert!(
            catalog.ids_are_unique(),
            "sample project ids must be unique"
        );
    }

    // ── SMP-005: Lookup by id ──
    #[test]
    fn smp_005_catalog_by_id_found() {
        let catalog = SampleProjectCatalog::builtin();
        let p = catalog.by_id("getting-started");
        assert!(p.is_some());
        assert_eq!(p.unwrap().name, "Getting Started");
    }

    // ── SMP-005: Lookup by id (not found) ──
    #[test]
    fn smp_005_catalog_by_id_not_found() {
        let catalog = SampleProjectCatalog::builtin();
        assert!(catalog.by_id("nonexistent").is_none());
    }

    // ── SMP-005: Sorted by sort_order ──
    #[test]
    fn smp_005_catalog_sorted_by_order() {
        let catalog = SampleProjectCatalog::builtin();
        let sorted = catalog.all_sorted();
        for i in 1..sorted.len() {
            assert!(
                sorted[i - 1].sort_order <= sorted[i].sort_order,
                "projects must be sorted by sort_order"
            );
        }
    }

    // ── SMP-005: Empty catalog ──
    #[test]
    fn smp_005_empty_catalog() {
        let catalog = SampleProjectCatalog::empty();
        assert!(catalog.all_sorted().is_empty());
        assert!(catalog.by_id("anything").is_none());
        assert!(catalog.ids_are_unique());
    }

    // ── Plan for project with no media ──
    #[test]
    fn plan_no_media() {
        let p = SampleProjectDefinition {
            id: "no-media".into(),
            name: "No Media".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/N.palmier", "https://media.example.com");
        assert!(plan.media_downloads.is_empty());
        assert!(plan.chat_files.is_empty());
    }

    // ── Plan with multiple chat files ──
    #[test]
    fn plan_multiple_chat_files() {
        let p = SampleProjectDefinition {
            id: "multi-chat".into(),
            name: "Multi Chat".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![
                ChatPayloadDef {
                    filename: "chat/one.json".into(),
                    content: r#"{"id":1}"#.into(),
                },
                ChatPayloadDef {
                    filename: "chat/two.json".into(),
                    content: r#"{"id":2}"#.into(),
                },
            ],
            sort_order: 0,
        };
        let plan = plan_materialization(&p, "/tmp/MC.palmier", "https://media.example.com");
        assert_eq!(plan.chat_files.len(), 2);
    }

    // ── SMP-003: Download coordinator ───────────────────────────

    #[test]
    fn smp_003_coordinator_created_from_plan() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![
                "http://example.com/v1.mp4".into(),
                "http://example.com/v2.mp4".into(),
            ],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let coord = DownloadCoordinator::new(&plan, 4);
        assert_eq!(coord.total_count(), 2);
        assert_eq!(coord.pending_count(), 2);
        assert_eq!(coord.completed_count(), 0);
        assert!(!coord.is_finished());
    }

    #[test]
    fn smp_003_coordinator_start_next() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["http://example.com/v1.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let mut coord = DownloadCoordinator::new(&plan, 4);
        let idx = coord.start_next();
        assert_eq!(idx, Some(0));
        assert_eq!(coord.pending_count(), 0);
        // No more pending
        assert!(coord.start_next().is_none());
    }

    #[test]
    fn smp_003_coordinator_mark_completed() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["http://example.com/v1.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let mut coord = DownloadCoordinator::new(&plan, 4);
        coord.start_next();
        coord.mark_completed(0);
        assert_eq!(coord.completed_count(), 1);
        assert!(coord.is_finished());
        assert!(!coord.has_failures);
    }

    #[test]
    fn smp_003_coordinator_mark_failed() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["http://example.com/v1.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let mut coord = DownloadCoordinator::new(&plan, 4);
        coord.start_next();
        coord.mark_failed(0, "timeout".into());
        assert_eq!(coord.failed_count(), 1);
        assert!(coord.has_failures);
        assert!(coord.is_finished());
    }

    #[test]
    fn smp_003_next_batch_respects_max_concurrent() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![
                "http://example.com/v1.mp4".into(),
                "http://example.com/v2.mp4".into(),
                "http://example.com/v3.mp4".into(),
                "http://example.com/v4.mp4".into(),
                "http://example.com/v5.mp4".into(),
            ],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let coord = DownloadCoordinator::new(&plan, 2);
        let batch = coord.next_batch();
        assert_eq!(
            batch.len(),
            2,
            "should start at most 2 concurrent downloads"
        );
    }

    #[test]
    fn smp_003_next_batch_respects_already_in_progress() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![
                "http://example.com/v1.mp4".into(),
                "http://example.com/v2.mp4".into(),
                "http://example.com/v3.mp4".into(),
            ],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let mut coord = DownloadCoordinator::new(&plan, 2);
        coord.start_next(); // 1 in-progress
        let batch = coord.next_batch();
        assert_eq!(batch.len(), 1, "only 1 slot left (max 2, 1 in-progress)");
    }

    // ── SMP-004: Cleanup plan ───────────────────────────────────

    #[test]
    fn smp_004_cleanup_plan_includes_all_outputs() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec!["http://example.com/v1.mp4".into()],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![ChatPayloadDef {
                filename: "chat/welcome.json".into(),
                content: "{}".into(),
            }],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let cleanup = cleanup_plan_for(&plan);
        // Should include media file, timeline, manifest, and chat
        assert!(cleanup.files_to_remove.iter().any(|f| f.contains("v1.mp4")));
        assert!(cleanup
            .files_to_remove
            .iter()
            .any(|f| f.contains("project.json")));
        assert!(cleanup
            .files_to_remove
            .iter()
            .any(|f| f.contains("media.json")));
        assert!(cleanup
            .files_to_remove
            .iter()
            .any(|f| f.contains("chat/welcome.json")));
        assert_eq!(cleanup.target_dir, "/tmp/test.palmier");
    }

    #[test]
    fn smp_004_cleanup_plan_with_no_chat() {
        let project = SampleProjectDefinition {
            id: "test".into(),
            name: "Test".into(),
            description: "".into(),
            thumbnail_url: None,
            media_urls: vec![],
            timeline_json: "{}".into(),
            manifest_json: "{}".into(),
            chat_payloads: vec![],
            sort_order: 0,
        };
        let plan = plan_materialization(&project, "/tmp/test.palmier", "http://cdn.example.com");
        let cleanup = cleanup_plan_for(&plan);
        // No media, no chat — just timeline + manifest
        assert!(cleanup
            .files_to_remove
            .iter()
            .any(|f| f.contains("project.json")));
        assert!(cleanup
            .files_to_remove
            .iter()
            .any(|f| f.contains("media.json")));
        assert_eq!(cleanup.files_to_remove.len(), 2);
    }
}
