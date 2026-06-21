use core_model::{
    ChatSession, GenerationLog, MediaManifest, Timeline, CHAT_DIRECTORY_NAME,
    GENERATION_LOG_FILENAME, MANIFEST_FILENAME, MEDIA_DIRECTORY_NAME, THUMBNAIL_FILENAME,
    TIMELINE_FILENAME,
};
use serde::de::DeserializeOwned;
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
    #[error("failed to decode JSON: {path}")]
    DecodeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, Clone)]
pub struct ProjectBundle {
    pub root: PathBuf,
    pub timeline: Timeline,
    pub manifest: Option<MediaManifest>,
    pub generation_log: Option<GenerationLog>,
    pub chat_sessions: Vec<ChatSession>,
    pub thumbnail_path: Option<PathBuf>,
    pub media_dir: Option<PathBuf>,
    pub chat_dir: Option<PathBuf>,
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

        let thumbnail_path = root.join(THUMBNAIL_FILENAME);
        let thumbnail_path = thumbnail_path.is_file().then_some(thumbnail_path);

        Ok(Self {
            root,
            timeline,
            manifest,
            generation_log,
            chat_sessions,
            thumbnail_path,
            media_dir,
            chat_dir,
        })
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

fn load_chat_sessions(path: &Path) -> Result<Vec<ChatSession>, BundleError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let mut json_files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|source| BundleError::ReadFile {
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
