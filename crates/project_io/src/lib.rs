pub mod project_duplicate;

use core_model::{
    AnimPair, ChatSession, Crop, GenerationLog, KeyframeTrack, MediaManifest, ProjectFile,
    Timeline, TimelineViewState, CHAT_DIRECTORY_NAME, GENERATION_LOG_FILENAME, MANIFEST_FILENAME,
    MEDIA_DIRECTORY_NAME, THUMBNAIL_FILENAME, TIMELINE_FILENAME, TRANSCRIPTS_DIRECTORY_NAME,
    VISUAL_INDEXES_DIRECTORY_NAME,
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

/// Multi-timeline project state carried alongside the active timeline
/// (upstream #255). `siblings` are the project's OTHER timelines in file
/// order; `active_index` is where the active timeline sits among all of them,
/// so saving reassembles `ProjectFile.timelines` in the original order.
#[derive(Debug, Clone, Default)]
pub struct MultiTimelineState {
    pub active_index: usize,
    pub siblings: Vec<Timeline>,
    pub open_timeline_ids: Option<Vec<String>>,
    pub view_states: Option<HashMap<String, TimelineViewState>>,
}

impl MultiTimelineState {
    /// Recompose the on-disk `ProjectFile` around the (possibly edited) active
    /// timeline.
    pub fn to_project_file(&self, active: &Timeline) -> ProjectFile {
        let mut timelines = Vec::with_capacity(self.siblings.len() + 1);
        timelines.extend(self.siblings.iter().cloned());
        let at = self.active_index.min(timelines.len());
        timelines.insert(at, active.clone());
        ProjectFile {
            timelines,
            active_timeline_id: Some(active.id.clone()),
            open_timeline_ids: self.open_timeline_ids.clone(),
            view_states: self.view_states.clone(),
        }
    }

    /// Split a decoded `ProjectFile` into (active timeline, everything else).
    pub fn from_project_file(mut file: ProjectFile) -> (Timeline, Self) {
        let active_index = file.active_index();
        let active = file.timelines.remove(active_index);
        (
            active,
            Self {
                active_index,
                siblings: file.timelines,
                open_timeline_ids: file.open_timeline_ids,
                view_states: file.view_states,
            },
        )
    }
}

#[derive(Debug, Clone)]
pub struct ProjectBundle {
    pub root: PathBuf,
    pub timeline: Timeline,
    /// The project's other timelines + per-timeline UI state (upstream #255).
    pub multi: MultiTimelineState,
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
        let project_file = read_project_file(&root.join(TIMELINE_FILENAME))?;
        let (timeline, multi) = MultiTimelineState::from_project_file(project_file);
        // A corrupt media.json degrades to an empty manifest (media offline)
        // rather than failing the whole open. Upstream palmier-pro #224.
        let manifest =
            read_optional_json_defaulting_on_decode_error(&root.join(MANIFEST_FILENAME))?;
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
            multi,
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

        write_project_file_json(
            &root.join(TIMELINE_FILENAME),
            &self.multi.to_project_file(&self.timeline),
        )?;
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

/// Write only `project.json` and `media.json` under `root`, leaving every
/// other file in the package untouched. The narrow save path for in-memory
/// editor state that does not hold the full bundle.
///
/// Holds only the ACTIVE timeline, so it re-reads the existing project.json
/// and swaps that timeline in place — a multi-timeline project's other
/// timelines survive the save (upstream #255).
pub fn save_project_state(
    root: &Path,
    timeline: &Timeline,
    manifest: &MediaManifest,
) -> Result<(), BundleError> {
    ensure_directory(root)?;
    let path = root.join(TIMELINE_FILENAME);
    let mut file = match read_project_file(&path) {
        Ok(existing) => existing,
        Err(_) => ProjectFile::wrapping(timeline.clone()),
    };
    let at = file
        .timelines
        .iter()
        .position(|t| t.id == timeline.id)
        .or_else(|| {
            // Legacy files decode with a fresh random id each read, so id match
            // fails; a single-timeline file unambiguously means "replace it".
            (file.timelines.len() == 1).then_some(0)
        })
        .unwrap_or_else(|| {
            // Unknown id in a multi-timeline file: replace the active slot.
            file.active_index()
        });
    file.timelines[at] = timeline.clone();
    file.active_timeline_id = Some(timeline.id.clone());
    write_project_file_json(&path, &file)?;
    write_json(&root.join(MANIFEST_FILENAME), manifest)?;
    Ok(())
}

/// [`save_project_state`] for a caller that holds the WHOLE timeline set
/// (upstream #255): the editor's active timeline + siblings are authoritative,
/// so the on-disk `timelines` becomes exactly that set — a timeline deleted in
/// the editor stays deleted (a plain upsert would resurrect it on autosave).
/// Disk array order is kept for surviving ids; new timelines append.
/// `openTimelineIds`/`viewStates` are preserved from disk, pruned to survivors.
pub fn save_project_state_with_siblings(
    root: &Path,
    timeline: &Timeline,
    siblings: &[Timeline],
    manifest: &MediaManifest,
) -> Result<(), BundleError> {
    ensure_directory(root)?;
    let path = root.join(TIMELINE_FILENAME);
    let (disk_order, open_ids, view_states) = match read_project_file(&path) {
        Ok(f) => (
            f.timelines.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            f.open_timeline_ids,
            f.view_states,
        ),
        Err(_) => (Vec::new(), None, None),
    };

    let mut by_id: HashMap<&str, &Timeline> =
        siblings.iter().map(|t| (t.id.as_str(), t)).collect();
    by_id.insert(timeline.id.as_str(), timeline);

    let mut timelines: Vec<Timeline> = Vec::with_capacity(by_id.len());
    for id in &disk_order {
        if let Some(t) = by_id.remove(id.as_str()) {
            timelines.push(t.clone());
        }
    }
    // New timelines (created this session): active first, then siblings in order.
    if by_id.remove(timeline.id.as_str()).is_some() {
        timelines.push(timeline.clone());
    }
    for sib in siblings {
        if by_id.remove(sib.id.as_str()).is_some() {
            timelines.push(sib.clone());
        }
    }

    let survivors: std::collections::HashSet<String> =
        timelines.iter().map(|t| t.id.clone()).collect();
    let file = ProjectFile {
        timelines,
        active_timeline_id: Some(timeline.id.clone()),
        open_timeline_ids: open_ids
            .map(|ids| ids.into_iter().filter(|i| survivors.contains(i)).collect())
            .filter(|ids: &Vec<String>| !ids.is_empty()),
        view_states: view_states
            .map(|vs| {
                vs.into_iter()
                    .filter(|(k, _)| survivors.contains(k))
                    .collect::<HashMap<_, _>>()
            })
            .filter(|vs| !vs.is_empty()),
    };
    write_project_file_json(&path, &file)?;
    write_json(&root.join(MANIFEST_FILENAME), manifest)?;
    Ok(())
}

/// Read `project.json` as a [`ProjectFile`], accepting the legacy bare-Timeline
/// form via [`ProjectFile::decode`]'s fallback (upstream #255).
fn read_project_file(path: &Path) -> Result<ProjectFile, BundleError> {
    if !path.is_file() {
        return Err(BundleError::MissingRequiredFile {
            path: path.to_path_buf(),
        });
    }
    let bytes = fs::read(path).map_err(|source| BundleError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    ProjectFile::decode(&bytes).map_err(|source| BundleError::DecodeJson {
        path: path.to_path_buf(),
        source,
    })
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

/// Reads optional JSON, but a decode failure yields `T::default()` instead of
/// propagating. Used for `media.json`, where a corrupt manifest
/// should open the project with media offline rather than block it entirely.
/// The original file is left untouched until the next save.
fn read_optional_json_defaulting_on_decode_error<T>(path: &Path) -> Result<Option<T>, BundleError>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(None);
    }

    match read_json(path) {
        Ok(value) => Ok(Some(value)),
        Err(BundleError::DecodeJson { .. }) => Ok(Some(T::default())),
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

    write_bytes_atomic(path, &bytes)
}

/// Write `bytes` to `path` atomically: write a sibling temp file, then rename it
/// over the target. A crash or power loss mid-write then leaves either the prior
/// good file or the complete new one — never a truncated/corrupt file. Rename over
/// an existing destination is atomic and replaces on both Unix and Windows.
fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), BundleError> {
    let mut tmp_os = path.as_os_str().to_owned();
    tmp_os.push(".tmp");
    let tmp = PathBuf::from(tmp_os);
    fs::write(&tmp, bytes).map_err(|source| BundleError::WriteFile {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, path).map_err(|source| {
        let _ = fs::remove_file(&tmp);
        BundleError::WriteFile {
            path: path.to_path_buf(),
            source,
        }
    })
}

/// Write `project.json` in the #255 ProjectFile form, sanitizing any non-finite
/// (NaN/Infinity) f64 in EVERY timeline first. serde_json serializes a non-finite
/// float as literal `null`, which then fails to deserialize into the non-`Option`
/// f64 fields on reopen — making the whole project permanently unopenable.
/// Sanitizing a save-time copy keeps the file re-openable.
fn write_project_file_json(path: &Path, file: &ProjectFile) -> Result<(), BundleError> {
    let mut sanitized = file.clone();
    for timeline in &mut sanitized.timelines {
        sanitize_non_finite(timeline);
    }
    write_json(path, &sanitized)
}

fn finite_or(v: f64, default: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        default
    }
}

fn sanitize_f64_track(track: &mut Option<KeyframeTrack<f64>>, default: f64) {
    if let Some(t) = track {
        for kf in &mut t.keyframes {
            kf.value = finite_or(kf.value, default);
        }
    }
}

fn sanitize_pair_track(track: &mut Option<KeyframeTrack<AnimPair>>) {
    if let Some(t) = track {
        for kf in &mut t.keyframes {
            kf.value.a = finite_or(kf.value.a, 0.0);
            kf.value.b = finite_or(kf.value.b, 0.0);
        }
    }
}

fn sanitize_crop_track(track: &mut Option<KeyframeTrack<Crop>>) {
    if let Some(t) = track {
        for kf in &mut t.keyframes {
            kf.value.left = finite_or(kf.value.left, 0.0);
            kf.value.top = finite_or(kf.value.top, 0.0);
            kf.value.right = finite_or(kf.value.right, 0.0);
            kf.value.bottom = finite_or(kf.value.bottom, 0.0);
        }
    }
}

/// Replace non-finite f64s in the timeline's arithmetic-computed numeric fields
/// (clip speed/volume/opacity, transform, crop, and every keyframe track) with a
/// safe finite default. These are the only fields that can become NaN/Infinity in
/// practice — style/effect floats come straight from validated JSON (which has no
/// non-finite literal) and are never arithmetic-derived.
pub fn sanitize_non_finite(timeline: &mut Timeline) {
    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            clip.speed = finite_or(clip.speed, 1.0);
            clip.volume = finite_or(clip.volume, 1.0);
            clip.opacity = finite_or(clip.opacity, 1.0);
            clip.transform.center_x = finite_or(clip.transform.center_x, 0.5);
            clip.transform.center_y = finite_or(clip.transform.center_y, 0.5);
            clip.transform.width = finite_or(clip.transform.width, 1.0);
            clip.transform.height = finite_or(clip.transform.height, 1.0);
            clip.transform.rotation = finite_or(clip.transform.rotation, 0.0);
            clip.crop.left = finite_or(clip.crop.left, 0.0);
            clip.crop.top = finite_or(clip.crop.top, 0.0);
            clip.crop.right = finite_or(clip.crop.right, 0.0);
            clip.crop.bottom = finite_or(clip.crop.bottom, 0.0);
            sanitize_f64_track(&mut clip.opacity_track, 1.0);
            sanitize_f64_track(&mut clip.volume_track, 0.0); // dB
            sanitize_f64_track(&mut clip.rotation_track, 0.0);
            sanitize_f64_track(&mut clip.stroke_progress_track, 1.0);
            sanitize_pair_track(&mut clip.position_track);
            sanitize_pair_track(&mut clip.scale_track);
            sanitize_crop_track(&mut clip.crop_track);
        }
    }
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
    let in_memory: std::collections::HashSet<String> =
        sessions.iter().map(|s| s.id.to_string()).collect();

    // Prune the chat directory per-file WITHOUT wiping it: remove only .json files
    // that parse as a ChatSession no longer held in memory (i.e. the user deleted
    // that session). Files that fail to parse (corrupt, or an unknown/newer format)
    // are PRESERVED — never silently delete chat data we cannot read. Previously the
    // whole directory was wiped and only in-memory sessions rewritten, so a session
    // dropped on load (parse failure) was permanently deleted on the next save.
    if path.exists() {
        for entry in fs::read_dir(path).map_err(|source| BundleError::ReadDirectory {
            path: path.to_path_buf(),
            source,
        })? {
            let entry = entry.map_err(|source| BundleError::ReadDirectory {
                path: path.to_path_buf(),
                source,
            })?;
            let file = entry.path();
            if file.extension() != Some(OsStr::new("json")) {
                continue;
            }
            let is_removed_valid_session = fs::read(&file)
                .ok()
                .and_then(|b| serde_json::from_slice::<ChatSession>(&b).ok())
                .map(|s| !in_memory.contains(&s.id.to_string()))
                .unwrap_or(false); // unparseable → preserve
            if is_removed_valid_session {
                fs::remove_file(&file).map_err(|source| BundleError::RemovePath {
                    path: file.clone(),
                    source,
                })?;
            }
        }
    }

    if sessions.is_empty() {
        // Nothing to write. If the directory is now empty (no preserved files), clean
        // it up; otherwise leave the preserved files in place.
        if path.exists()
            && fs::read_dir(path)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
        {
            remove_path_if_exists(path)?;
        }
        return Ok(());
    }

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
