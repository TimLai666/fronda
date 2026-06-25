pub mod project_duplicate;

use core_model::{
    ChatSession, GenerationLog, MediaManifest, Timeline, CHAT_DIRECTORY_NAME,
    GENERATION_LOG_FILENAME, MANIFEST_FILENAME, MEDIA_DIRECTORY_NAME, THUMBNAIL_FILENAME,
    TIMELINE_FILENAME, TRANSCRIPTS_DIRECTORY_NAME, VISUAL_INDEXES_DIRECTORY_NAME,
};
use search_core::search_index::VisualIndex;
use search_core::transcript::Transcript;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("missing required file: {path}")]
    MissingRequiredFile { path: PathBuf },
    #[error("failed to read file: {path}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read directory: {path}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode JSON: {path}")]
    DecodeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to encode JSON: {path}")]
    EncodeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to create directory: {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write file: {path}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove path: {path}")]
    RemovePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to copy file from {from} to {to}")]
    CopyFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ProjectBundle {
    pub root: PathBuf,
    pub timeline: Timeline,
    pub manifest: Option<MediaManifest>,
    pub generation_log: Option<GenerationLog>,
    pub chat_sessions: Vec<ChatSession>,
    pub transcripts: HashMap<String, Transcript>,
    pub visual_indexes: HashMap<String, VisualIndex>,
    pub thumbnail_path: Option<PathBuf>,
    pub media_dir: Option<PathBuf>,
    pub chat_dir: Option<PathBuf>,
    pub transcripts_dir: Option<PathBuf>,
    pub visual_indexes_dir: Option<PathBuf>,
}

impl ProjectBundle {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BundleError> {
        let root = path.as_ref().to_path_buf();
        let timeline = read_required_json(&root.join(TIMELINE_FILENAME))?;
        let manifest = read_optional_json(&root.join(MANIFEST_FILENAME))?;
        let generation_log =
            read_optional_json_ignoring_decode_errors(&root.join(GENERATION_LOG_FILENAME))?;

        let chat_dir = root.join(CHAT_DIRECTORY_NAME);
        let chat_sessions = load_chat_sessions(&chat_dir)?;
        let chat_dir = chat_dir.is_dir().then_some(chat_dir);

        let media_dir = root.join(MEDIA_DIRECTORY_NAME);
        let media_dir = media_dir.is_dir().then_some(media_dir);

        let transcripts_dir = root.join(TRANSCRIPTS_DIRECTORY_NAME);
        let transcripts = load_json_map::<Transcript>(&transcripts_dir)?;
        let transcripts_dir = transcripts_dir.is_dir().then_some(transcripts_dir);

        let visual_indexes_dir = root.join(VISUAL_INDEXES_DIRECTORY_NAME);
        let visual_indexes = load_json_map::<VisualIndex>(&visual_indexes_dir)?;
        let visual_indexes_dir = visual_indexes_dir.is_dir().then_some(visual_indexes_dir);

        let thumbnail_path = root.join(THUMBNAIL_FILENAME);
        let thumbnail_path = thumbnail_path.is_file().then_some(thumbnail_path);

        Ok(Self {
            root,
            timeline,
            manifest,
            generation_log,
            chat_sessions,
            transcripts,
            visual_indexes,
            thumbnail_path,
            media_dir,
            chat_dir,
            transcripts_dir,
            visual_indexes_dir,
        })
    }

    pub fn save(&self) -> Result<(), BundleError> {
        self.save_to(&self.root)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), BundleError> {
        let root = path.as_ref();
        ensure_directory(root)?;

        write_json(&root.join(TIMELINE_FILENAME), &self.timeline)?;
        write_optional_json(&root.join(MANIFEST_FILENAME), self.manifest.as_ref())?;
        write_optional_json(
            &root.join(GENERATION_LOG_FILENAME),
            self.generation_log.as_ref(),
        )?;
        write_chat_sessions(&root.join(CHAT_DIRECTORY_NAME), &self.chat_sessions)?;
        write_json_map(&root.join(TRANSCRIPTS_DIRECTORY_NAME), &self.transcripts)?;
        write_json_map(
            &root.join(VISUAL_INDEXES_DIRECTORY_NAME),
            &self.visual_indexes,
        )?;
        sync_optional_file(
            self.thumbnail_path.as_deref(),
            &root.join(THUMBNAIL_FILENAME),
        )?;
        sync_optional_directory(self.media_dir.as_deref(), &root.join(MEDIA_DIRECTORY_NAME))?;

        Ok(())
    }

    pub fn project_path_for(&self, relative_path: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative_path)
    }
}

fn read_required_json<T>(path: &Path) -> Result<T, BundleError>
where
    T: DeserializeOwned,
{
    if !path.is_file() {
        return Err(BundleError::MissingRequiredFile {
            path: path.to_path_buf(),
        });
    }

    read_json(path)
}

fn read_optional_json<T>(path: &Path) -> Result<Option<T>, BundleError>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }

    read_json(path).map(Some)
}

fn read_optional_json_ignoring_decode_errors<T>(path: &Path) -> Result<Option<T>, BundleError>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(None);
    }

    match read_json(path) {
        Ok(value) => Ok(Some(value)),
        Err(BundleError::DecodeJson { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_json<T>(path: &Path) -> Result<T, BundleError>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(path).map_err(|source| BundleError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    serde_json::from_slice(&bytes).map_err(|source| BundleError::DecodeJson {
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_directory(path: &Path) -> Result<(), BundleError> {
    fs::create_dir_all(path).map_err(|source| BundleError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}

fn write_optional_json<T>(path: &Path, value: Option<&T>) -> Result<(), BundleError>
where
    T: Serialize,
{
    match value {
        Some(value) => write_json(path, value),
        None => remove_path_if_exists(path),
    }
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), BundleError>
where
    T: Serialize,
{
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|source| BundleError::EncodeJson {
        path: path.to_path_buf(),
        source,
    })?;
    bytes.push(b'\n');

    fs::write(path, bytes).map_err(|source| BundleError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn load_chat_sessions(path: &Path) -> Result<Vec<ChatSession>, BundleError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut json_files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|source| BundleError::ReadDirectory {
            path: path.to_path_buf(),
            source,
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|entry| entry.extension() == Some(OsStr::new("json")))
        .collect();

    json_files.sort();

    Ok(json_files
        .into_iter()
        .filter_map(|path| {
            let bytes = fs::read(&path).ok()?;
            serde_json::from_slice::<ChatSession>(&bytes).ok()
        })
        .collect())
}

fn load_json_map<T>(path: &Path) -> Result<HashMap<String, T>, BundleError>
where
    T: DeserializeOwned,
{
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let mut map = HashMap::new();
    for entry in fs::read_dir(path).map_err(|source| BundleError::ReadDirectory {
        path: path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| BundleError::ReadDirectory {
            path: path.to_path_buf(),
            source,
        })?;
        let entry_path = entry.path();
        if entry_path.extension() == Some(OsStr::new("json")) {
            if let Some(stem) = entry_path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(value) = read_json::<T>(&entry_path) {
                    map.insert(stem.to_string(), value);
                }
            }
        }
    }
    Ok(map)
}

fn write_json_map<T>(path: &Path, map: &HashMap<String, T>) -> Result<(), BundleError>
where
    T: Serialize,
{
    if map.is_empty() {
        return remove_path_if_exists(path);
    }

    remove_path_if_exists(path)?;
    ensure_directory(path)?;

    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = map.get(key) {
            write_json(&path.join(format!("{key}.json")), value)?;
        }
    }
    Ok(())
}

fn write_chat_sessions(path: &Path, sessions: &[ChatSession]) -> Result<(), BundleError> {
    if sessions.is_empty() {
        return remove_path_if_exists(path);
    }

    remove_path_if_exists(path)?;
    ensure_directory(path)?;

    let mut ordered_sessions: Vec<&ChatSession> = sessions.iter().collect();
    ordered_sessions.sort_by_key(|session| session.id);

    for session in ordered_sessions {
        write_json(&path.join(format!("{}.json", session.id)), session)?;
    }

    Ok(())
}

fn sync_optional_file(source: Option<&Path>, destination: &Path) -> Result<(), BundleError> {
    match source {
        Some(source) => {
            if source == destination && source.exists() {
                return Ok(());
            }

            remove_path_if_exists(destination)?;
            fs::copy(source, destination).map_err(|source_error| BundleError::CopyFile {
                from: source.to_path_buf(),
                to: destination.to_path_buf(),
                source: source_error,
            })?;
            Ok(())
        }
        None => remove_path_if_exists(destination),
    }
}

fn sync_optional_directory(source: Option<&Path>, destination: &Path) -> Result<(), BundleError> {
    match source {
        Some(source) => {
            if source == destination && source.exists() {
                return Ok(());
            }

            remove_path_if_exists(destination)?;
            copy_dir_recursive(source, destination)
        }
        None => remove_path_if_exists(destination),
    }
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), BundleError> {
    ensure_directory(destination)?;

    for entry in fs::read_dir(source).map_err(|source_error| BundleError::ReadDirectory {
        path: source.to_path_buf(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| BundleError::ReadDirectory {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|source_error| {
                BundleError::CopyFile {
                    from: source_path.clone(),
                    to: destination_path.clone(),
                    source: source_error,
                }
            })?;
        }
    }

    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<(), BundleError> {
    if !path.exists() {
        return Ok(());
    }

    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|source| BundleError::RemovePath {
            path: path.to_path_buf(),
            source,
        })
    } else {
        fs::remove_file(path).map_err(|source| BundleError::RemovePath {
            path: path.to_path_buf(),
            source,
        })
    }
}
