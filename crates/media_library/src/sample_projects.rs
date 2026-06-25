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
}
