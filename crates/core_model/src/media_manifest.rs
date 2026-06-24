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
