use chrono::{DateTime, Utc};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn default_manifest_version_for_write() -> i64 {
    2
}

fn default_manifest_version_for_decode() -> i64 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaManifest {
    #[serde(default = "default_manifest_version_for_decode")]
    pub version: i64,
    #[serde(default)]
    pub entries: Vec<MediaManifestEntry>,
    #[serde(default)]
    pub folders: Vec<MediaFolder>,
}

impl Default for MediaManifest {
    fn default() -> Self {
        Self {
            version: default_manifest_version_for_write(),
            entries: Vec::new(),
            folders: Vec::new(),
        }
    }
}

impl MediaManifest {
    /// Find entry by id (RES-001).
    pub fn entry_for(&self, id: &str) -> Option<&MediaManifestEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Reconstruct expected file URL even if currently missing (RES-002).
    ///
    /// For external entries, returns the absolute path. For project entries,
    /// returns None since relative paths need a project root.
    pub fn expected_url_for(&self, id: &str) -> Option<String> {
        self.entry_for(id).and_then(|e| match &e.source {
            MediaSource::External { absolute_path } => Some(absolute_path.clone()),
            MediaSource::Project { .. } => None,
        })
    }

    /// Check if entry exists and its file is on disk (RES-003).
    ///
    /// Returns `None` if no entry found, `Some(true)` if file exists,
    /// `Some(false)` if file does not exist.
    pub fn resolve_url_for(&self, id: &str, file_exists: impl Fn(&str) -> bool) -> Option<bool> {
        let entry = self.entry_for(id)?;
        // Entries with cached_remote_url are always resolvable
        if entry.cached_remote_url.is_some() {
            return Some(true);
        }
        let path = match &entry.source {
            MediaSource::External { absolute_path } => absolute_path.clone(),
            MediaSource::Project { relative_path } => relative_path.clone(),
        };
        Some(file_exists(&path))
    }

    /// Returns true when expected file does not exist or entry is missing (RES-004).
    pub fn is_missing_for(&self, id: &str, file_exists: impl Fn(&str) -> bool) -> bool {
        self.entry_for(id).is_none_or(|entry| {
            if entry.cached_remote_url.is_some() {
                return false;
            }
            let path = match &entry.source {
                MediaSource::External { absolute_path } => absolute_path.clone(),
                MediaSource::Project { relative_path } => relative_path.clone(),
            };
            !file_exists(&path)
        })
    }

    /// Returns display name for an entry, falling back to "Offline" when
    /// no entry exists (RES-005).
    pub fn display_name_for(&self, id: &str) -> String {
        self.entry_for(id)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| "Offline".to_string())
    }

    /// Returns IDs of entries whose local files are missing.
    ///
    /// Entries with `cached_remote_url` populated are never considered missing
    /// (they can be re-downloaded). The `is_missing` callback receives each
    /// entry and returns `true` if the underlying file does not exist on disk.
    pub fn missing_entry_ids(
        &self,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
    ) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.cached_remote_url.is_none() && is_missing(e))
            .map(|e| e.id.clone())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaFolder {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentFolderId")]
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationInput {
    pub prompt: String,
    pub model: String,
    pub duration: i64,
    pub aspect_ratio: String,
    pub resolution: Option<String>,
    pub quality: Option<String>,
    pub image_urls: Option<Vec<String>>,
    pub num_images: Option<i64>,
    pub voice: Option<String>,
    pub lyrics: Option<String>,
    pub style_instructions: Option<String>,
    pub instrumental: Option<bool>,
    pub generate_audio: Option<bool>,
    pub reference_image_urls: Option<Vec<String>>,
    pub reference_video_urls: Option<Vec<String>>,
    pub reference_audio_urls: Option<Vec<String>>,
    pub image_url_asset_ids: Option<Vec<String>>,
    pub reference_image_asset_ids: Option<Vec<String>>,
    pub reference_video_asset_ids: Option<Vec<String>>,
    pub reference_audio_asset_ids: Option<Vec<String>>,
    #[serde(default, with = "crate::date_serde::option_foundation_date")]
    pub created_at: Option<DateTime<Utc>>,
}

impl Default for GenerationInput {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            model: String::new(),
            duration: 0,
            aspect_ratio: "16:9".to_string(),
            resolution: None,
            quality: None,
            image_urls: None,
            num_images: None,
            voice: None,
            lyrics: None,
            style_instructions: None,
            instrumental: None,
            generate_audio: None,
            reference_image_urls: None,
            reference_video_urls: None,
            reference_audio_urls: None,
            image_url_asset_ids: None,
            reference_image_asset_ids: None,
            reference_video_asset_ids: None,
            reference_audio_asset_ids: None,
            created_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaManifestEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: crate::timeline::ClipType,
    pub source: MediaSource,
    pub duration: f64,
    pub generation_input: Option<GenerationInput>,
    pub source_width: Option<i64>,
    pub source_height: Option<i64>,
    pub source_fps: Option<f64>,
    pub has_audio: Option<bool>,
    pub folder_id: Option<String>,
    pub cached_remote_url: Option<String>,
    #[serde(default, with = "crate::date_serde::option_foundation_date")]
    pub cached_remote_url_expires_at: Option<DateTime<Utc>>,
    /// Source timecode frame from QuickTime `tmcd` track. Upstream PR #136.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_timecode_frame: Option<i64>,
    /// Frame quanta of the `tmcd` track (its own rate, often 30 DF even on 60p).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_timecode_quanta: Option<i64>,
    /// Drop-frame flag of the `tmcd` track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_timecode_drop_frame: Option<bool>,
    /// On-device Vision classification tags (Issue #118).
    ///
    /// Populated by `VNClassifyImageRequest` on macOS. Runs locally, no cost.
    /// Example: ["outdoor", "nature", "sky", "person"]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_tags: Option<Vec<String>>,
    /// AI-generated one-line content description (Issue #118).
    ///
    /// Produced by a vision-LLM call on representative scene frames.
    /// Credit-metered action. Example: "Person hiking through a forest trail."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_description: Option<String>,
    /// Label generation status (Issue #118).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_label_status: Option<AiLabelStatus>,
}

/// Status of AI content label generation for a media asset (Issue #118).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiLabelStatus {
    /// Not yet labeled.
    None,
    /// On-device tagging complete; AI description pending.
    TagsOnly,
    /// Both tags and AI description are available.
    Complete,
    /// Label generation failed.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource {
    External { absolute_path: String },
    Project { relative_path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExternalPayload {
    #[serde(rename = "absolutePath")]
    absolute_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProjectPayload {
    #[serde(rename = "relativePath")]
    relative_path: String,
}

impl Serialize for MediaSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::External { absolute_path } => {
                map.serialize_entry(
                    "external",
                    &ExternalPayload {
                        absolute_path: absolute_path.clone(),
                    },
                )?;
            }
            Self::Project { relative_path } => {
                map.serialize_entry(
                    "project",
                    &ProjectPayload {
                        relative_path: relative_path.clone(),
                    },
                )?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for MediaSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            NestedExternal {
                external: ExternalPayload,
            },
            NestedProject {
                project: ProjectPayload,
            },
            FlatExternal {
                #[serde(rename = "absolutePath")]
                absolute_path: String,
            },
            FlatProject {
                #[serde(rename = "relativePath")]
                relative_path: String,
            },
        }

        match Repr::deserialize(deserializer)? {
            Repr::NestedExternal { external } => Ok(Self::External {
                absolute_path: external.absolute_path,
            }),
            Repr::NestedProject { project } => Ok(Self::Project {
                relative_path: project.relative_path,
            }),
            Repr::FlatExternal { absolute_path } => Ok(Self::External { absolute_path }),
            Repr::FlatProject { relative_path } => Ok(Self::Project { relative_path }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, cached_url: Option<&str>) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: format!("entry-{id}"),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: format!("/tmp/{id}.mp4"),
            },
            duration: 10.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: cached_url.map(String::from),
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
        }
    }

    #[test]
    fn med_001_missing_entry_ids_all_exist() {
        let manifest = MediaManifest {
            entries: vec![entry("a", None), entry("b", None)],
            ..Default::default()
        };
        let missing = manifest.missing_entry_ids(|_| false);
        assert!(missing.is_empty(), "no entries should be missing");
    }

    #[test]
    fn med_002_missing_entry_ids_returns_missing() {
        let manifest = MediaManifest {
            entries: vec![entry("a", None), entry("b", None)],
            ..Default::default()
        };
        let missing = manifest.missing_entry_ids(|e| e.id == "a");
        assert_eq!(missing, vec!["a"]);
    }

    #[test]
    fn med_003_missing_entry_ids_cached_url_excludes() {
        let manifest = MediaManifest {
            entries: vec![
                entry("a", Some("https://cache.example.com/a.mp4")),
                entry("b", None),
            ],
            ..Default::default()
        };
        // Both are "missing" per callback, but "a" has cached_remote_url so excluded.
        let missing = manifest.missing_entry_ids(|_| true);
        assert_eq!(missing, vec!["b"]);
    }

    #[test]
    fn med_004_missing_entry_ids_all_cached_not_missing() {
        let manifest = MediaManifest {
            entries: vec![
                entry("a", Some("https://cache.example.com/a.mp4")),
                entry("b", Some("https://cache.example.com/b.mp4")),
            ],
            ..Default::default()
        };
        let missing = manifest.missing_entry_ids(|_| true);
        assert!(missing.is_empty(), "cached entries should not be missing");
    }

    #[test]
    fn med_005_missing_entry_ids_empty_manifest() {
        let manifest = MediaManifest::default();
        let missing = manifest.missing_entry_ids(|_| true);
        assert!(missing.is_empty(), "empty manifest has no missing entries");
    }

    #[test]
    fn med_006_missing_entry_ids_mixed() {
        let manifest = MediaManifest {
            entries: vec![
                entry("online", None),              // exists -> not missing
                entry("offline", None),             // missing
                entry("cached", Some("https://c")), // cached -> not missing
                entry("also_offline", None),        // missing
            ],
            ..Default::default()
        };
        let missing = manifest.missing_entry_ids(|e| e.id != "online");
        assert_eq!(missing, vec!["offline", "also_offline"]);
    }

    #[test]
    fn res_001_entry_for_found() {
        let manifest = MediaManifest {
            entries: vec![entry("a", None), entry("b", None)],
            ..Default::default()
        };
        let found = manifest.entry_for("a");
        assert!(found.is_some(), "RES-001: entry found");
        assert_eq!(found.unwrap().id, "a");
    }

    #[test]
    fn res_001_entry_for_not_found() {
        let manifest = MediaManifest::default();
        let found = manifest.entry_for("nonexistent");
        assert!(found.is_none(), "RES-001: not found returns None");
    }

    #[test]
    fn res_002_expected_url_for_external() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "ext".to_string(),
            name: "ext".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/file.mp4".to_string(),
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
        });
        let url = manifest.expected_url_for("ext");
        assert_eq!(url, Some("/path/to/file.mp4".to_string()));
    }

    #[test]
    fn res_002_expected_url_for_missing_entry() {
        let manifest = MediaManifest::default();
        let url = manifest.expected_url_for("nonexistent");
        assert_eq!(url, None, "RES-002: missing entry returns None");
    }

    #[test]
    fn res_003_resolve_url_file_exists() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "vid".to_string(),
            name: "vid".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/vid.mp4".to_string(),
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
        });
        let result = manifest.resolve_url_for("vid", |p| p == "/path/to/vid.mp4");
        assert_eq!(result, Some(true), "RES-003: file exists");
    }

    #[test]
    fn res_003_resolve_url_file_missing() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "vid".to_string(),
            name: "vid".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/vid.mp4".to_string(),
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
        });
        let result = manifest.resolve_url_for("vid", |_| false);
        assert_eq!(result, Some(false), "RES-003: file missing");
    }

    #[test]
    fn res_003_resolve_url_nonexistent_entry() {
        let manifest = MediaManifest::default();
        let result = manifest.resolve_url_for("nonexistent", |_| true);
        assert_eq!(result, None, "RES-003: no entry returns None");
    }

    #[test]
    fn res_003_resolve_url_cached_always_ok() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "cached".to_string(),
            name: "cached".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/cached.mp4".to_string(),
            },
            duration: 10.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: Some("https://cache.example.com/vid.mp4".to_string()),
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
        });
        let result = manifest.resolve_url_for("cached", |_| false);
        assert_eq!(result, Some(true), "RES-003: cached is always resolvable");
    }

    #[test]
    fn res_004_is_missing_missing_file() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "vid".to_string(),
            name: "vid".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/vid.mp4".to_string(),
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
        });
        assert!(
            manifest.is_missing_for("vid", |_| false),
            "RES-004: missing file"
        );
    }

    #[test]
    fn res_004_is_missing_entry_not_found() {
        let manifest = MediaManifest::default();
        assert!(
            manifest.is_missing_for("nonexistent", |_| true),
            "RES-004: missing entry"
        );
    }

    #[test]
    fn res_004_is_not_missing_when_file_exists() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "vid".to_string(),
            name: "vid".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/path/to/vid.mp4".to_string(),
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
        });
        assert!(
            !manifest.is_missing_for("vid", |_| true),
            "RES-004: not missing when file exists"
        );
    }

    #[test]
    fn res_005_display_name_found() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "vid".to_string(),
            name: "My Video.mp4".to_string(),
            r#type: crate::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/v.mp4".to_string(),
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
        });
        assert_eq!(
            manifest.display_name_for("vid"),
            "My Video.mp4",
            "RES-005: returns name"
        );
    }

    #[test]
    fn res_005_display_name_fallback_offline() {
        let manifest = MediaManifest::default();
        assert_eq!(
            manifest.display_name_for("nonexistent"),
            "Offline",
            "RES-005: falls back to Offline"
        );
    }

    // ── Issue #118: AI content labels ──────────────────────────────────────

    fn make_entry_with_ai(id: &str) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: "clip.mp4".to_string(),
            r#type: super::super::timeline::ClipType::Video,
            source: MediaSource::External {
                absolute_path: "/clip.mp4".to_string(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: Some(vec!["outdoor".into(), "nature".into()]),
            ai_description: Some("Person hiking through a forest trail.".into()),
            ai_label_status: Some(AiLabelStatus::Complete),
        }
    }

    #[test]
    fn issue_118_ai_tags_stored_and_retrieved() {
        let entry = make_entry_with_ai("e1");
        let tags = entry.ai_tags.unwrap();
        assert!(tags.contains(&"outdoor".to_string()));
        assert!(tags.contains(&"nature".to_string()));
    }

    #[test]
    fn issue_118_ai_description_stored() {
        let entry = make_entry_with_ai("e1");
        assert!(entry.ai_description.unwrap().contains("hiking"));
    }

    #[test]
    fn issue_118_ai_label_status_complete() {
        let entry = make_entry_with_ai("e1");
        assert_eq!(entry.ai_label_status, Some(AiLabelStatus::Complete));
    }

    #[test]
    fn issue_118_ai_fields_default_to_none() {
        let manifest = MediaManifest::default();
        // A freshly imported entry should have no AI labels
        assert!(manifest.entries.is_empty()); // no entries = no labels
    }

    #[test]
    fn issue_118_ai_label_status_variants() {
        assert_ne!(AiLabelStatus::None, AiLabelStatus::TagsOnly);
        assert_ne!(AiLabelStatus::TagsOnly, AiLabelStatus::Complete);
        assert_ne!(AiLabelStatus::Complete, AiLabelStatus::Failed);
    }

    #[test]
    fn issue_118_ai_fields_roundtrip_serde() {
        let entry = make_entry_with_ai("e1");
        let json = serde_json::to_string(&entry).unwrap();
        let restored: MediaManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.ai_tags, entry.ai_tags);
        assert_eq!(restored.ai_description, entry.ai_description);
        assert_eq!(restored.ai_label_status, entry.ai_label_status);
    }

    #[test]
    fn issue_118_none_ai_fields_skip_serialization() {
        let mut entry = make_entry_with_ai("e1");
        entry.ai_tags = None;
        entry.ai_description = None;
        entry.ai_label_status = None;
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("aiTags"), "json={json}");
        assert!(!json.contains("aiDescription"), "json={json}");
        assert!(!json.contains("aiLabelStatus"), "json={json}");
    }
}
