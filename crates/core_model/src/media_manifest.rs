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
        }
    }

    #[test]
    fn med_001_missing_entry_ids_all_exist() {
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![entry("a", None), entry("b", None)];
        let missing = manifest.missing_entry_ids(|_| false);
        assert!(missing.is_empty(), "no entries should be missing");
    }

    #[test]
    fn med_002_missing_entry_ids_returns_missing() {
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![entry("a", None), entry("b", None)];
        let missing = manifest.missing_entry_ids(|e| e.id == "a");
        assert_eq!(missing, vec!["a"]);
    }

    #[test]
    fn med_003_missing_entry_ids_cached_url_excludes() {
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("a", Some("https://cache.example.com/a.mp4")),
            entry("b", None),
        ];
        // Both are "missing" per callback, but "a" has cached_remote_url so excluded.
        let missing = manifest.missing_entry_ids(|_| true);
        assert_eq!(missing, vec!["b"]);
    }

    #[test]
    fn med_004_missing_entry_ids_all_cached_not_missing() {
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("a", Some("https://cache.example.com/a.mp4")),
            entry("b", Some("https://cache.example.com/b.mp4")),
        ];
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
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("online", None),              // exists -> not missing
            entry("offline", None),             // missing
            entry("cached", Some("https://c")), // cached -> not missing
            entry("also_offline", None),        // missing
        ];
        let missing = manifest.missing_entry_ids(|e| e.id != "online");
        assert_eq!(missing, vec!["offline", "also_offline"]);
    }
}
