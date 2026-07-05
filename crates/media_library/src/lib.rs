mod media_import;
mod pasteboard_import;
pub mod sample_projects;
mod save_as_media;

pub use media_import::*;
pub use pasteboard_import::*;
pub use save_as_media::*;

use core_model::{ClipType, MediaFolder, MediaManifest, MediaManifestEntry, MediaSource, Timeline};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum FolderError {
    SelfParenting,
    DescendantCycle,
    FolderNotFound,
    AssetNotFound,
    EmptyName,
}

#[derive(Debug, PartialEq)]
pub enum RelinkError {
    DifferentMediaType { expected: String, got: String },
}

// ---------------------------------------------------------------------------
// Folder deletion effect (FLD-014..017)
// ---------------------------------------------------------------------------

/// Effects returned by `delete_folder_with_timeline_effects`.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderDeleteEffect {
    /// All folder IDs that were logically deleted.
    pub deleted_folder_ids: Vec<String>,
    /// Asset IDs that were part of the deleted folder subtree.
    pub deleted_asset_ids: Vec<String>,
    /// Clip IDs that were removed from the timeline.
    pub removed_clip_ids: Vec<String>,
    /// Track IDs that were pruned because they became empty.
    pub pruned_track_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Folder operations  (FLD-001 through FLD-022)
// ---------------------------------------------------------------------------

pub struct FolderOps;

impl FolderOps {
    /// FLD-001/002: Returns immediate children of a folder (or root when parent_id is None).
    /// FLD-003: Results are sorted case-insensitively by name.
    pub fn subfolders<'a>(
        manifest: &'a MediaManifest,
        parent_id: Option<&'a str>,
    ) -> Vec<&'a MediaFolder> {
        let mut result: Vec<&MediaFolder> = manifest
            .folders
            .iter()
            .filter(|f| f.parent_folder_id.as_deref() == parent_id)
            .collect();
        result.sort_by_key(|a| a.name.to_lowercase());
        result
    }

    /// FLD-004: Returns root-to-target order of folder IDs.
    /// FLD-005: Terminates safely on cyclic/corrupt metadata.
    pub fn folder_path(
        manifest: &MediaManifest,
        folder_id: &str,
    ) -> Result<Vec<String>, FolderError> {
        let folder = manifest
            .folders
            .iter()
            .find(|f| f.id == folder_id)
            .ok_or(FolderError::FolderNotFound)?;

        let mut path: Vec<String> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();
        let mut current: Option<&str> = Some(&folder.id);

        while let Some(id) = current {
            if !visited.insert(id.to_string()) {
                // Cycle detected — terminate safely per FLD-005
                return Err(FolderError::DescendantCycle);
            }
            path.push(id.to_string());

            let f = manifest
                .folders
                .iter()
                .find(|f| f.id == id)
                .ok_or(FolderError::FolderNotFound)?;
            current = f.parent_folder_id.as_deref();
        }

        // Reverse to produce root-to-target order
        path.reverse();
        Ok(path)
    }

    /// FLD-006: Create a folder with the given name and optional parent.
    pub fn create_folder(
        manifest: &mut MediaManifest,
        name: String,
        parent_id: Option<String>,
    ) -> Result<String, FolderError> {
        if name.trim().is_empty() {
            return Err(FolderError::EmptyName);
        }
        if let Some(ref pid) = parent_id {
            if !manifest.folders.iter().any(|f| f.id == *pid) {
                return Err(FolderError::FolderNotFound);
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        manifest.folders.push(MediaFolder {
            id: id.clone(),
            name,
            parent_folder_id: parent_id,
        });
        Ok(id)
    }

    /// FLD-007: Rename a folder (logical metadata only).
    pub fn rename_folder(
        manifest: &mut MediaManifest,
        folder_id: &str,
        new_name: String,
    ) -> Result<(), FolderError> {
        if new_name.trim().is_empty() {
            return Err(FolderError::EmptyName);
        }
        let folder = manifest
            .folders
            .iter_mut()
            .find(|f| f.id == folder_id)
            .ok_or(FolderError::FolderNotFound)?;
        folder.name = new_name;
        Ok(())
    }

    /// FLD-008/009: Move a folder under a new parent (or root).
    /// FLD-010: Reject self-parenting.
    /// FLD-011: Reject moving under own descendant.
    pub fn move_folder(
        manifest: &mut MediaManifest,
        folder_id: &str,
        new_parent_id: Option<String>,
    ) -> Result<(), FolderError> {
        // Verify the folder exists
        let _folder = manifest
            .folders
            .iter()
            .find(|f| f.id == folder_id)
            .ok_or(FolderError::FolderNotFound)?;

        if new_parent_id.as_deref() == Some(folder_id) {
            return Err(FolderError::SelfParenting);
        }

        if let Some(ref pid) = new_parent_id {
            // Verify the target parent exists
            if !manifest.folders.iter().any(|f| f.id == *pid) {
                return Err(FolderError::FolderNotFound);
            }
            // Reject if new_parent is a descendant of folder_id (would create a cycle)
            if Self::is_descendant_of(manifest, pid, folder_id) {
                return Err(FolderError::DescendantCycle);
            }
        }

        let folder = manifest
            .folders
            .iter_mut()
            .find(|f| f.id == folder_id)
            .ok_or(FolderError::FolderNotFound)?;
        folder.parent_folder_id = new_parent_id;
        Ok(())
    }

    /// FLD-012/013: Delete a folder and all its descendants recursively.
    /// Returns the list of all deleted folder IDs.
    pub fn delete_folder(
        manifest: &mut MediaManifest,
        folder_id: &str,
    ) -> Result<Vec<String>, FolderError> {
        if !manifest.folders.iter().any(|f| f.id == folder_id) {
            return Err(FolderError::FolderNotFound);
        }

        let mut to_delete: HashSet<String> = HashSet::new();
        to_delete.insert(folder_id.to_string());
        for id in Self::descendant_ids(manifest, folder_id) {
            to_delete.insert(id);
        }

        manifest.folders.retain(|f| !to_delete.contains(&f.id));

        let mut deleted: Vec<String> = to_delete.into_iter().collect();
        deleted.sort();
        Ok(deleted)
    }

    /// FLD-014/015/016/017: Delete a folder with full timeline cleanup.
    ///
    /// Effectively calls `delete_folder` for the logical deletion, then:
    /// - FLD-014: Removes timeline clips referencing deleted assets
    /// - FLD-015: Prunes newly empty tracks after clip removal
    /// - FLD-016: Returns asset ids for preview-tab cleanup (caller must close tabs)
    /// - FLD-017: Removes deleted folder/asset ids from timeline selection state
    pub fn delete_folder_with_timeline_effects(
        manifest: &mut MediaManifest,
        timeline: &mut Timeline,
        folder_id: &str,
    ) -> Result<FolderDeleteEffect, FolderError> {
        // First find all assets in the subtree before deleting the folder
        let affected_asset_ids: HashSet<String> = Self::asset_ids_in_subtree(manifest, folder_id)
            .into_iter()
            .collect();

        // Delete folders logically
        let deleted_folder_ids = Self::delete_folder(manifest, folder_id)?;
        let deleted_folder_set: HashSet<String> = deleted_folder_ids.iter().cloned().collect();

        // FLD-013: Remove affected manifest entries from the library
        manifest
            .entries
            .retain(|e| !affected_asset_ids.contains(&e.id));

        // FLD-014: Remove clips referencing deleted assets from timeline
        let removed_clip_ids = Self::remove_clips_by_media_ref(timeline, &affected_asset_ids);

        // FLD-015: Prune newly empty tracks after clip removal
        let pruned_track_ids = Self::prune_empty_tracks(timeline);

        // FLD-017: Remove deleted folder/asset/clip ids from selection state
        timeline
            .selected_clip_ids
            .retain(|id| !removed_clip_ids.contains(id));

        // FLD-016: Signal which assets need preview tab closure (returned to caller)
        Ok(FolderDeleteEffect {
            deleted_folder_ids: deleted_folder_set.into_iter().collect(),
            deleted_asset_ids: affected_asset_ids.into_iter().collect(),
            removed_clip_ids,
            pruned_track_ids,
        })
    }

    /// Remove all clips whose `media_ref` is in the given set. Returns removed clip ids.
    fn remove_clips_by_media_ref(
        timeline: &mut Timeline,
        media_refs: &HashSet<String>,
    ) -> Vec<String> {
        let mut removed = Vec::new();
        for track in &mut timeline.tracks {
            track.clips.retain(|clip| {
                if media_refs.contains(&clip.media_ref) {
                    removed.push(clip.id.clone());
                    false
                } else {
                    true
                }
            });
        }
        removed
    }

    /// Remove tracks that have no clips. Returns ids of pruned tracks.
    fn prune_empty_tracks(timeline: &mut Timeline) -> Vec<String> {
        let mut pruned = Vec::new();
        timeline.tracks.retain(|track| {
            if track.clips.is_empty() {
                pruned.push(track.id.clone());
                false
            } else {
                true
            }
        });
        pruned
    }

    /// FLD-018/019: Move an asset to a folder (or root when folder_id is None).
    pub fn move_asset(entry: &mut MediaManifestEntry, folder_id: Option<String>) {
        entry.folder_id = folder_id;
    }

    /// FLD-020: Rename an asset (manifest metadata only, not the disk file).
    pub fn rename_asset(entry: &mut MediaManifestEntry, new_name: String) {
        entry.name = new_name;
    }

    /// FLD-021/022: Delete an asset from the manifest (not source bytes on disk).
    pub fn delete_asset(manifest: &mut MediaManifest, asset_id: &str) -> Result<(), FolderError> {
        let idx = manifest
            .entries
            .iter()
            .position(|e| e.id == asset_id)
            .ok_or(FolderError::AssetNotFound)?;
        manifest.entries.remove(idx);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Check if `folder_id` is a descendant of `potential_ancestor_id`
    /// by following parent_folder_id links upward.
    fn is_descendant_of(
        manifest: &MediaManifest,
        folder_id: &str,
        potential_ancestor_id: &str,
    ) -> bool {
        let mut visited: HashSet<String> = HashSet::new();
        let mut current: Option<String> = Some(folder_id.to_string());
        while let Some(id) = current {
            if id == potential_ancestor_id {
                return true;
            }
            if !visited.insert(id.clone()) {
                // Cycle — break to avoid infinite loop
                return false;
            }
            let f = match manifest.folders.iter().find(|f| f.id == id) {
                Some(f) => f,
                None => return false,
            };
            current = f.parent_folder_id.clone();
        }
        false
    }

    /// Get all descendant folder IDs (excluding the folder itself).
    fn descendant_ids(manifest: &MediaManifest, folder_id: &str) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let mut queue: Vec<&str> = vec![folder_id];
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(folder_id.to_string());

        while let Some(id) = queue.pop() {
            for child in &manifest.folders {
                if child.parent_folder_id.as_deref() == Some(id) && visited.insert(child.id.clone())
                {
                    result.push(child.id.clone());
                    queue.push(&child.id);
                }
            }
        }
        result
    }

    /// Get all asset IDs that belong to the folder subtree (folder + all descendants).
    pub fn asset_ids_in_subtree(manifest: &MediaManifest, folder_id: &str) -> Vec<String> {
        let mut folder_ids: HashSet<String> = HashSet::new();
        folder_ids.insert(folder_id.to_string());
        for id in Self::descendant_ids(manifest, folder_id) {
            folder_ids.insert(id);
        }

        manifest
            .entries
            .iter()
            .filter(|e| e.folder_id.as_ref().is_some_and(|f| folder_ids.contains(f)))
            .map(|e| e.id.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Relink operations  (RLK-001 through RLK-008)
// ---------------------------------------------------------------------------

pub struct RelinkOps;

#[derive(Debug, PartialEq)]
pub struct BatchRelinkResult {
    pub relinked: usize,
    pub total_offline: usize,
}

impl RelinkOps {
    /// RLK-001/002: Single-asset relink — update source path and re-finalize metadata.
    /// RLK-003: Reject if the new extension maps to a different ClipType.
    pub fn single_relink(
        entry: &mut MediaManifestEntry,
        new_absolute_path: &str,
    ) -> Result<(), RelinkError> {
        let new_type =
            clip_type_from_extension(new_absolute_path).ok_or(RelinkError::DifferentMediaType {
                expected: format!("{:?}", entry.r#type),
                got: "unknown".to_string(),
            })?;

        if new_type != entry.r#type {
            return Err(RelinkError::DifferentMediaType {
                expected: format!("{:?}", entry.r#type),
                got: format!("{:?}", new_type),
            });
        }

        entry.source = MediaSource::External {
            absolute_path: new_absolute_path.to_string(),
        };
        // Re-finalize: clear cached remote URL since the file is now local
        entry.cached_remote_url = None;
        entry.cached_remote_url_expires_at = None;

        Ok(())
    }

    /// RLK-004 through RLK-008: Batch relink — matches offline assets against
    /// candidate files by lowercased filename.  First-match-wins for duplicates.
    pub fn batch_relink(
        manifest: &mut MediaManifest,
        candidate_files: &[String],
    ) -> BatchRelinkResult {
        // Build a map: lowercased filename -> first candidate path
        let mut filename_map: HashMap<String, String> = HashMap::new();
        for path in candidate_files {
            let fname = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase());
            if let Some(fname) = fname {
                filename_map.entry(fname).or_insert_with(|| path.clone());
            }
        }

        // Identify offline assets (no fresh cached_remote_url, and marked missing).
        let offline_ids: HashSet<String> = manifest
            .missing_entry_ids(chrono::Utc::now(), |_| true)
            .into_iter()
            .collect();
        let total_offline = offline_ids.len();
        let mut relinked: usize = 0;

        for entry in &mut manifest.entries {
            if !offline_ids.contains(&entry.id) {
                continue;
            }
            let lower_name = entry.name.to_lowercase();
            if let Some(candidate_path) = filename_map.get(&lower_name) {
                // Verify type compatibility before relinking
                if let Some(new_type) = clip_type_from_extension(candidate_path) {
                    if new_type == entry.r#type {
                        entry.source = MediaSource::External {
                            absolute_path: candidate_path.clone(),
                        };
                        entry.cached_remote_url = None;
                        entry.cached_remote_url_expires_at = None;
                        relinked += 1;
                    }
                }
            }
        }

        BatchRelinkResult {
            relinked,
            total_offline,
        }
    }
}

/// Determine the ClipType from a file path's extension.
/// Delegates to `ClipType::from_extension`.
fn clip_type_from_extension(path: &str) -> Option<ClipType> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ClipType::from_extension(ext)
}

// ---------------------------------------------------------------------------
// Supported extensions  (MED-001)
// ---------------------------------------------------------------------------

pub struct SupportedExtensions;

impl SupportedExtensions {
    pub const VIDEO: &'static [&'static str] = &["mov", "mp4", "m4v"];
    pub const AUDIO: &'static [&'static str] =
        &["mp3", "wav", "aac", "m4a", "aiff", "aif", "aifc", "flac"];
    pub const IMAGE: &'static [&'static str] = &["png", "jpg", "jpeg", "tiff", "heic", "webp"];
    pub const LOTTIE: &'static [&'static str] = &["json", "lottie"];

    /// Check if a file extension is supported.
    pub fn is_supported(ext: &str) -> bool {
        let ext = ext.to_lowercase();
        Self::VIDEO.contains(&ext.as_str())
            || Self::AUDIO.contains(&ext.as_str())
            || Self::IMAGE.contains(&ext.as_str())
            || Self::LOTTIE.contains(&ext.as_str())
    }

    /// Get the ClipType for a supported extension.
    pub fn clip_type_for(ext: &str) -> Option<ClipType> {
        ClipType::from_extension(ext)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Clip, Crop, Interpolation, Track, Transform};

    // -----------------------------------------------------------------------
    // Helper: build a MediaManifestEntry with defaults
    // -----------------------------------------------------------------------
    fn entry(
        id: &str,
        clip_type: ClipType,
        name: &str,
        folder_id: Option<&str>,
    ) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.to_string(),
            name: name.to_string(),
            r#type: clip_type,
            source: MediaSource::External {
                absolute_path: format!("/fake/{name}"),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: folder_id.map(String::from),
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    fn entry_cached(id: &str, name: &str) -> MediaManifestEntry {
        let mut e = entry(id, ClipType::Video, name, None);
        e.cached_remote_url = Some("https://cache.example.com/vid.mp4".to_string());
        e
    }

    // -----------------------------------------------------------------------
    // Helper: build a MediaFolder
    // -----------------------------------------------------------------------
    fn folder(id: &str, name: &str, parent_id: Option<&str>) -> MediaFolder {
        MediaFolder {
            id: id.to_string(),
            name: name.to_string(),
            parent_folder_id: parent_id.map(String::from),
        }
    }

    // =======================================================================
    // FLD-001: Root representation (nil parentFolderId)
    // =======================================================================
    #[test]
    fn fld_001_root_has_nil_parent() {
        let manifest = MediaManifest::default();
        let root = FolderOps::subfolders(&manifest, None);
        assert!(root.is_empty(), "empty manifest has no root folders");
    }

    // =======================================================================
    // FLD-002: Subfolders returns immediate children only
    // =======================================================================
    #[test]
    fn fld_002_subfolders_immediate_children_only() {
        let manifest = MediaManifest {
            folders: vec![
                folder("root1", "Root One", None),
                folder("root2", "Root Two", None),
                folder("child1", "Child One", Some("root1")),
                folder("child2", "Child Two", Some("root1")),
                folder("grandchild", "Grandchild", Some("child1")),
            ],
            ..Default::default()
        };

        let root_folders: Vec<&str> = FolderOps::subfolders(&manifest, None)
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(root_folders, vec!["root1", "root2"]);

        let root1_children: Vec<&str> = FolderOps::subfolders(&manifest, Some("root1"))
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(root1_children, vec!["child1", "child2"]);

        // child1 has only "grandchild" as its child, not child2
        let child1_children: Vec<&str> = FolderOps::subfolders(&manifest, Some("child1"))
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(child1_children, vec!["grandchild"]);
    }

    // =======================================================================
    // FLD-003: Case-insensitive sort
    // =======================================================================
    #[test]
    fn fld_003_case_insensitive_sort() {
        let manifest = MediaManifest {
            folders: vec![
                folder("b", "Beta", None),
                folder("a", "alpha", None),
                folder("c", "CHARLIE", None),
            ],
            ..Default::default()
        };

        let names: Vec<&str> = FolderOps::subfolders(&manifest, None)
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "Beta", "CHARLIE"]);
    }

    // =======================================================================
    // FLD-004: Folder path root-to-target
    // =======================================================================
    #[test]
    fn fld_004_folder_path_root_to_target() {
        let manifest = MediaManifest {
            folders: vec![
                folder("a", "A", None),
                folder("b", "B", Some("a")),
                folder("c", "C", Some("b")),
            ],
            ..Default::default()
        };

        let path = FolderOps::folder_path(&manifest, "c").unwrap();
        assert_eq!(path, vec!["a", "b", "c"]);

        let path = FolderOps::folder_path(&manifest, "a").unwrap();
        assert_eq!(path, vec!["a"]);
    }

    // =======================================================================
    // FLD-005: Cyclic metadata terminates safely
    // =======================================================================
    #[test]
    fn fld_005_cycle_terminates_safely() {
        let manifest = MediaManifest {
            folders: vec![
                folder("a", "A", Some("b")), // a's parent is b
                folder("b", "B", Some("a")), // b's parent is a → cycle
            ],
            ..Default::default()
        };

        let result = FolderOps::folder_path(&manifest, "a");
        assert_eq!(result, Err(FolderError::DescendantCycle));

        let result = FolderOps::folder_path(&manifest, "b");
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    #[test]
    fn fld_005_self_cycle_terminates_safely() {
        let manifest = MediaManifest {
            folders: vec![folder("a", "A", Some("a"))], // self-loop
            ..Default::default()
        };

        let result = FolderOps::folder_path(&manifest, "a");
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    // =======================================================================
    // FLD-006: Create folder
    // =======================================================================
    #[test]
    fn fld_006_create_folder_at_root() {
        let mut manifest = MediaManifest::default();
        let id = FolderOps::create_folder(&mut manifest, "New Folder".to_string(), None).unwrap();
        assert_eq!(manifest.folders.len(), 1);
        assert_eq!(manifest.folders[0].id, id);
        assert_eq!(manifest.folders[0].name, "New Folder");
        assert_eq!(manifest.folders[0].parent_folder_id, None);
    }

    #[test]
    fn fld_006_create_folder_with_parent() {
        let mut manifest = MediaManifest::default();
        let parent_id =
            FolderOps::create_folder(&mut manifest, "Parent".to_string(), None).unwrap();
        let child_id =
            FolderOps::create_folder(&mut manifest, "Child".to_string(), Some(parent_id.clone()))
                .unwrap();

        assert_eq!(manifest.folders.len(), 2);
        let child = manifest.folders.iter().find(|f| f.id == child_id).unwrap();
        assert_eq!(child.parent_folder_id, Some(parent_id));
    }

    #[test]
    fn fld_006_create_folder_empty_name_rejected() {
        let mut manifest = MediaManifest::default();
        let result = FolderOps::create_folder(&mut manifest, "  ".to_string(), None);
        assert_eq!(result, Err(FolderError::EmptyName));
    }

    #[test]
    fn fld_006_create_folder_bad_parent_rejected() {
        let mut manifest = MediaManifest::default();
        let result = FolderOps::create_folder(
            &mut manifest,
            "Orphan".to_string(),
            Some("nonexistent".to_string()),
        );
        assert_eq!(result, Err(FolderError::FolderNotFound));
    }

    // =======================================================================
    // FLD-007: Rename folder
    // =======================================================================
    #[test]
    fn fld_007_rename_folder() {
        let mut manifest = MediaManifest::default();
        let id = FolderOps::create_folder(&mut manifest, "Old Name".to_string(), None).unwrap();
        FolderOps::rename_folder(&mut manifest, &id, "New Name".to_string()).unwrap();
        let folder = manifest.folders.iter().find(|f| f.id == id).unwrap();
        assert_eq!(folder.name, "New Name");
    }

    #[test]
    fn fld_007_rename_nonexistent_folder() {
        let mut manifest = MediaManifest::default();
        let result = FolderOps::rename_folder(&mut manifest, "no-such-id", "Name".to_string());
        assert_eq!(result, Err(FolderError::FolderNotFound));
    }

    #[test]
    fn fld_007_rename_to_empty_rejected() {
        let mut manifest = MediaManifest::default();
        let id = FolderOps::create_folder(&mut manifest, "Name".to_string(), None).unwrap();
        let result = FolderOps::rename_folder(&mut manifest, &id, "".to_string());
        assert_eq!(result, Err(FolderError::EmptyName));
    }

    // =======================================================================
    // FLD-008/009: Move folder
    // =======================================================================
    #[test]
    fn fld_008_move_folder_under_other() {
        let mut manifest = MediaManifest {
            folders: vec![
                folder("a", "A", None),
                folder("b", "B", None),
                folder("c", "C", Some("a")),
            ],
            ..Default::default()
        };

        FolderOps::move_folder(&mut manifest, "c", Some("b".to_string())).unwrap();
        let c = manifest.folders.iter().find(|f| f.id == "c").unwrap();
        assert_eq!(c.parent_folder_id, Some("b".to_string()));
    }

    #[test]
    fn fld_009_move_folder_to_root() {
        let mut manifest = MediaManifest {
            folders: vec![folder("a", "A", None), folder("b", "B", Some("a"))],
            ..Default::default()
        };

        FolderOps::move_folder(&mut manifest, "b", None).unwrap();
        let b = manifest.folders.iter().find(|f| f.id == "b").unwrap();
        assert_eq!(b.parent_folder_id, None);
    }

    // =======================================================================
    // FLD-010: Reject self-parenting
    // =======================================================================
    #[test]
    fn fld_010_reject_self_parenting() {
        let mut manifest = MediaManifest {
            folders: vec![folder("a", "A", None)],
            ..Default::default()
        };

        let result = FolderOps::move_folder(&mut manifest, "a", Some("a".to_string()));
        assert_eq!(result, Err(FolderError::SelfParenting));
    }

    // =======================================================================
    // FLD-011: Reject descendant cycle
    // =======================================================================
    #[test]
    fn fld_011_reject_move_under_descendant() {
        let mut manifest = MediaManifest {
            folders: vec![
                folder("a", "A", None),
                folder("b", "B", Some("a")),
                folder("c", "C", Some("b")),
            ],
            ..Default::default()
        };

        // Moving "a" under "c" would create A → B → C → A cycle
        let result = FolderOps::move_folder(&mut manifest, "a", Some("c".to_string()));
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    #[test]
    fn fld_011_reject_move_parent_under_child() {
        let mut manifest = MediaManifest {
            folders: vec![folder("p", "Parent", None), folder("c", "Child", Some("p"))],
            ..Default::default()
        };

        let result = FolderOps::move_folder(&mut manifest, "p", Some("c".to_string()));
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    // =======================================================================
    // FLD-012/013: Delete folder recursively
    // =======================================================================
    #[test]
    fn fld_012_delete_folder_recursive() {
        let mut manifest = MediaManifest {
            folders: vec![
                folder("a", "A", None),
                folder("b", "B", Some("a")),
                folder("c", "C", Some("b")),
                folder("other", "Other", None),
            ],
            entries: vec![
                entry("e1", ClipType::Video, "vid1.mp4", Some("a")),
                entry("e2", ClipType::Video, "vid2.mp4", Some("b")),
                entry("e3", ClipType::Video, "vid3.mp4", None),
            ],
            ..Default::default()
        };

        let deleted = FolderOps::delete_folder(&mut manifest, "a").unwrap();
        let mut deleted_sorted = deleted.clone();
        deleted_sorted.sort();
        assert_eq!(deleted_sorted, vec!["a", "b", "c"]);

        // The "other" folder should remain
        assert_eq!(manifest.folders.len(), 1);
        assert_eq!(manifest.folders[0].id, "other");

        // Entries with deleted folder_id remain (folders are logical only)
        assert_eq!(manifest.entries.len(), 3);
    }

    #[test]
    fn fld_013_delete_nonexistent_folder() {
        let mut manifest = MediaManifest::default();
        let result = FolderOps::delete_folder(&mut manifest, "no-such-id");
        assert_eq!(result, Err(FolderError::FolderNotFound));
    }

    // =======================================================================
    // FLD-018: Move asset to folder
    // =======================================================================
    #[test]
    fn fld_018_move_asset_to_folder() {
        let mut e = entry("e1", ClipType::Video, "vid.mp4", None);
        assert_eq!(e.folder_id, None);

        FolderOps::move_asset(&mut e, Some("folder-a".to_string()));
        assert_eq!(e.folder_id, Some("folder-a".to_string()));

        // Move back to root
        FolderOps::move_asset(&mut e, None);
        assert_eq!(e.folder_id, None);
    }

    // =======================================================================
    // FLD-020: Rename asset
    // =======================================================================
    #[test]
    fn fld_020_rename_asset() {
        let mut e = entry("e1", ClipType::Video, "old.mp4", None);
        FolderOps::rename_asset(&mut e, "new.mp4".to_string());
        assert_eq!(e.name, "new.mp4");
    }

    // =======================================================================
    // FLD-021/022: Delete asset (not source bytes)
    // =======================================================================
    #[test]
    fn fld_021_delete_asset() {
        let mut manifest = MediaManifest {
            entries: vec![
                entry("e1", ClipType::Video, "vid1.mp4", None),
                entry("e2", ClipType::Video, "vid2.mp4", None),
            ],
            ..Default::default()
        };

        FolderOps::delete_asset(&mut manifest, "e1").unwrap();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].id, "e2");
    }

    #[test]
    fn fld_022_delete_nonexistent_asset() {
        let mut manifest = MediaManifest::default();
        let result = FolderOps::delete_asset(&mut manifest, "no-such-id");
        assert_eq!(result, Err(FolderError::AssetNotFound));
    }

    // =======================================================================
    // asset_ids_in_subtree helper
    // =======================================================================
    #[test]
    fn asset_ids_in_subtree_returns_asset_ids() {
        let manifest = MediaManifest {
            folders: vec![folder("a", "A", None), folder("b", "B", Some("a"))],
            entries: vec![
                entry("e1", ClipType::Video, "vid1.mp4", Some("a")),
                entry("e2", ClipType::Video, "vid2.mp4", Some("b")),
                entry("e3", ClipType::Video, "vid3.mp4", None),
            ],
            ..Default::default()
        };

        let mut ids = FolderOps::asset_ids_in_subtree(&manifest, "a");
        ids.sort();
        assert_eq!(ids, vec!["e1", "e2"]);
    }

    // =======================================================================
    // RLK-001: Single-asset relink updates source
    // =======================================================================
    #[test]
    fn rlk_001_single_relink_updates_source() {
        let mut e = entry("e1", ClipType::Video, "old.mp4", None);
        RelinkOps::single_relink(&mut e, "/new/path/video.mov").unwrap();
        assert_eq!(
            e.source,
            MediaSource::External {
                absolute_path: "/new/path/video.mov".to_string()
            }
        );
        assert_eq!(e.cached_remote_url, None);
    }

    // =======================================================================
    // RLK-002: Re-finalize (cached_remote_url cleared)
    // =======================================================================
    #[test]
    fn rlk_002_single_relink_clears_cache() {
        let mut e = entry("e1", ClipType::Video, "old.mp4", None);
        e.cached_remote_url = Some("https://old.cache/vid.mp4".to_string());
        e.cached_remote_url_expires_at =
            Some(chrono::DateTime::from_timestamp(9999999999, 0).unwrap());

        RelinkOps::single_relink(&mut e, "/new/video.mp4").unwrap();
        assert_eq!(e.cached_remote_url, None);
        assert_eq!(e.cached_remote_url_expires_at, None);
    }

    // =======================================================================
    // RLK-003: Reject different media type
    // =======================================================================
    #[test]
    fn rlk_003_reject_different_media_type() {
        let mut e = entry("e1", ClipType::Video, "video.mp4", None);
        let result = RelinkOps::single_relink(&mut e, "/new/audio.mp3");
        assert_eq!(
            result,
            Err(RelinkError::DifferentMediaType {
                expected: "Video".to_string(),
                got: "Audio".to_string(),
            })
        );
    }

    #[test]
    fn rlk_003_reject_unknown_extension() {
        let mut e = entry("e1", ClipType::Video, "video.mp4", None);
        let result = RelinkOps::single_relink(&mut e, "/new/script.txt");
        assert_eq!(
            result,
            Err(RelinkError::DifferentMediaType {
                expected: "Video".to_string(),
                got: "unknown".to_string(),
            })
        );
    }

    // =======================================================================
    // RLK-004: Batch relink offline-only
    // =======================================================================
    #[test]
    fn rlk_004_batch_relink_offline_only() {
        let mut manifest = MediaManifest {
            entries: vec![
                entry("off1", ClipType::Video, "vid1.mp4", None),
                entry_cached("cached1", "vid2.mp4"),
            ],
            ..Default::default()
        };

        let candidates: Vec<String> = vec!["/new/vid1.mp4".to_string()];
        let result = RelinkOps::batch_relink(&mut manifest, &candidates);
        // Only the offline vid1.mp4 was relinked
        assert_eq!(result.relinked, 1);
        assert_eq!(result.total_offline, 1);

        // cached entry's source should be unchanged
        let cached = manifest.entries.iter().find(|e| e.id == "cached1").unwrap();
        assert!(matches!(cached.source, MediaSource::External { .. }));
    }

    // =======================================================================
    // RLK-005: Recursive candidate indexing
    // =======================================================================
    #[test]
    fn rlk_005_recursive_candidate_indexing() {
        let mut manifest = MediaManifest {
            entries: vec![entry("e1", ClipType::Video, "vid1.mp4", None)],
            ..Default::default()
        };

        // Candidates from different directories are all indexed
        let candidates: Vec<String> = vec![
            "/dir1/vid1.mp4".to_string(),
            "/dir2/sub/vid1.mp4".to_string(),
        ];
        let result = RelinkOps::batch_relink(&mut manifest, &candidates);
        assert_eq!(result.relinked, 1);
        // First-match-wins: /dir1/vid1.mp4 is used
        let entry = &manifest.entries[0];
        assert_eq!(
            entry.source,
            MediaSource::External {
                absolute_path: "/dir1/vid1.mp4".to_string()
            }
        );
    }

    // =======================================================================
    // RLK-006: Match by lowercased filename
    // =======================================================================
    #[test]
    fn rlk_006_match_by_lowercased_filename() {
        let mut manifest = MediaManifest::default();
        let e = entry("e1", ClipType::Video, "My Video.MP4", None);
        manifest.entries.push(e);

        let candidates: Vec<String> = vec!["/path/my video.mp4".to_string()];
        let result = RelinkOps::batch_relink(&mut manifest, &candidates);
        assert_eq!(result.relinked, 1);

        let entry = &manifest.entries[0];
        assert_eq!(
            entry.source,
            MediaSource::External {
                absolute_path: "/path/my video.mp4".to_string()
            }
        );
    }

    // =======================================================================
    // RLK-007: First-match-wins for duplicates
    // =======================================================================
    #[test]
    fn rlk_007_first_match_wins() {
        let mut manifest = MediaManifest::default();
        let e = entry("e1", ClipType::Video, "vid.mp4", None);
        manifest.entries.push(e);

        let candidates: Vec<String> =
            vec!["/first/vid.mp4".to_string(), "/second/vid.mp4".to_string()];
        RelinkOps::batch_relink(&mut manifest, &candidates);
        let entry = &manifest.entries[0];
        assert_eq!(
            entry.source,
            MediaSource::External {
                absolute_path: "/first/vid.mp4".to_string()
            }
        );
    }

    // =======================================================================
    // RLK-008: Return (relinked, total_offline)
    // =======================================================================
    #[test]
    fn rlk_008_returns_relinked_and_total_offline() {
        let mut manifest = MediaManifest {
            entries: vec![
                entry("off1", ClipType::Video, "vid1.mp4", None),
                entry("off2", ClipType::Audio, "track.wav", None),
                entry_cached("cached1", "vid3.mp4"),
            ],
            ..Default::default()
        };

        let candidates: Vec<String> = vec![
            "/new/vid1.mp4".to_string(),
            // track.wav is not in candidates, so stays offline
        ];
        let result = RelinkOps::batch_relink(&mut manifest, &candidates);
        assert_eq!(result.relinked, 1); // only vid1.mp4 matched
        assert_eq!(result.total_offline, 2); // off1 + off2 (cached1 excluded)
    }

    // =======================================================================
    // MED-001: Supported extensions map correctly
    // =======================================================================
    #[test]
    fn med_001_supported_extensions_video() {
        for ext in SupportedExtensions::VIDEO {
            assert!(SupportedExtensions::is_supported(ext));
            assert_eq!(
                SupportedExtensions::clip_type_for(ext),
                Some(ClipType::Video)
            );
        }
    }

    #[test]
    fn med_001_supported_extensions_audio() {
        for ext in SupportedExtensions::AUDIO {
            assert!(SupportedExtensions::is_supported(ext));
            assert_eq!(
                SupportedExtensions::clip_type_for(ext),
                Some(ClipType::Audio)
            );
        }
    }

    #[test]
    fn med_001_supported_extensions_image() {
        for ext in SupportedExtensions::IMAGE {
            assert!(SupportedExtensions::is_supported(ext));
            assert_eq!(
                SupportedExtensions::clip_type_for(ext),
                Some(ClipType::Image)
            );
        }
    }

    #[test]
    fn med_001_supported_extensions_lottie() {
        for ext in SupportedExtensions::LOTTIE {
            assert!(SupportedExtensions::is_supported(ext));
            assert_eq!(
                SupportedExtensions::clip_type_for(ext),
                Some(ClipType::Lottie)
            );
        }
    }

    #[test]
    fn med_001_json_is_only_lottie() {
        assert!(SupportedExtensions::is_supported("json"));
        assert_eq!(
            SupportedExtensions::clip_type_for("json"),
            Some(ClipType::Lottie)
        );
    }

    #[test]
    fn med_001_unknown_extension_returns_none() {
        assert!(!SupportedExtensions::is_supported("exe"));
        assert!(!SupportedExtensions::is_supported("txt"));
        assert!(!SupportedExtensions::is_supported(""));
        assert_eq!(SupportedExtensions::clip_type_for("exe"), None);
    }

    // =======================================================================
    // clip_type_from_extension utility
    // =======================================================================
    #[test]
    fn clip_type_from_path_video() {
        assert_eq!(
            clip_type_from_extension("/path/to/video.MOV"),
            Some(ClipType::Video)
        );
        assert_eq!(
            clip_type_from_extension("C:\\Users\\test\\video.mp4"),
            Some(ClipType::Video)
        );
    }

    #[test]
    fn clip_type_from_path_audio() {
        assert_eq!(
            clip_type_from_extension("/path/to/sound.wav"),
            Some(ClipType::Audio)
        );
        assert_eq!(
            clip_type_from_extension("/path/to/sound.aifc"),
            Some(ClipType::Audio)
        );
        assert_eq!(
            clip_type_from_extension("/path/to/sound.flac"),
            Some(ClipType::Audio)
        );
    }

    #[test]
    fn clip_type_from_path_unknown() {
        assert_eq!(clip_type_from_extension("/path/to/file.txt"), None);
        assert_eq!(clip_type_from_extension("/path/with.no.ext"), None);
    }

    // =======================================================================
    // FLD-014..017: Folder deletion with timeline effects
    // =======================================================================

    /// Helper: create a minimal Clip with required fields.
    fn make_clip(id: &str, media_ref: &str, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.into(),
            media_ref: media_ref.into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform {
                center_x: 0.5,
                center_y: 0.5,
                width: 1.0,
                height: 1.0,
                rotation: 0.0,
                flip_horizontal: false,
                flip_vertical: false,
            },
            crop: Crop {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: Default::default(),
            chroma_key: None,
            text_animation: None,
            word_timings: None,
        }
    }

    /// Helper: create a minimal Timeline with one track containing one clip.
    fn timeline_with_clip(clip_id: &str, media_ref: &str) -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: HashSet::from([clip_id.to_string()]),
            tracks: vec![Track {
                id: "track-1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![make_clip(clip_id, media_ref, 0, 30)],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn fld_014_delete_folder_removes_timeline_clips() {
        let mut manifest = MediaManifest::default();
        let folder_id =
            FolderOps::create_folder(&mut manifest, "Test Folder".into(), None).unwrap();
        manifest.entries.push(entry(
            "asset-1",
            ClipType::Video,
            "video.mp4",
            Some(&folder_id),
        ));

        let mut timeline = timeline_with_clip("clip-1", "asset-1");

        let effect = FolderOps::delete_folder_with_timeline_effects(
            &mut manifest,
            &mut timeline,
            &folder_id,
        )
        .unwrap();

        assert!(effect.removed_clip_ids.contains(&"clip-1".to_string()));
        assert!(effect.deleted_asset_ids.contains(&"asset-1".to_string()));
        // FLD-013: Manifest entries in deleted folder are removed
        assert!(
            manifest.entries.iter().all(|e| e.id != "asset-1"),
            "FLD-013: asset in deleted folder is removed from manifest"
        );
        // Clip removed — track may also be pruned (FLD-015)
        assert!(timeline.tracks.is_empty() || timeline.tracks[0].clips.is_empty());
    }

    #[test]
    fn fld_013_delete_folder_removes_manifest_entries() {
        let mut manifest = MediaManifest::default();
        let folder_id = FolderOps::create_folder(&mut manifest, "Folder".into(), None).unwrap();
        manifest.entries.push(entry(
            "asset-in-folder",
            ClipType::Video,
            "vid.mp4",
            Some(&folder_id),
        ));
        manifest
            .entries
            .push(entry("asset-orphan", ClipType::Image, "img.png", None));
        let mut timeline = Timeline::default();

        FolderOps::delete_folder_with_timeline_effects(&mut manifest, &mut timeline, &folder_id)
            .unwrap();

        // FLD-013: Assets inside deleted folder are removed from manifest
        assert!(
            manifest.entries.iter().all(|e| e.id != "asset-in-folder"),
            "FLD-013: asset in deleted folder removed"
        );
        // Orphan assets outside the folder are preserved
        assert!(
            manifest.entries.iter().any(|e| e.id == "asset-orphan"),
            "FLD-013: orphan asset preserved"
        );
    }

    #[test]
    fn fld_014_unaffected_clips_preserved() {
        let mut manifest = MediaManifest::default();
        let folder_id = FolderOps::create_folder(&mut manifest, "Folder".into(), None).unwrap();
        manifest.entries.push(entry(
            "asset-del",
            ClipType::Image,
            "img.png",
            Some(&folder_id),
        ));
        manifest
            .entries
            .push(entry("asset-keep", ClipType::Video, "vid.mp4", None));

        let mut timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: HashSet::new(),
            tracks: vec![Track {
                id: "track-1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: true,
               display_height: 50.0,
                clips: vec![
                    make_clip("clip-del", "asset-del", 0, 30),
                    make_clip("clip-keep", "asset-keep", 30, 30),
                ],
            }],
            ..Default::default()
        };

        FolderOps::delete_folder_with_timeline_effects(&mut manifest, &mut timeline, &folder_id)
            .unwrap();

        // Deleted clip removed, keep clip preserved
        assert!(!timeline.tracks[0].clips.iter().any(|c| c.id == "clip-del"));
        assert!(timeline.tracks[0].clips.iter().any(|c| c.id == "clip-keep"));
    }

    #[test]
    fn fld_015_prunes_empty_tracks() {
        let mut manifest = MediaManifest::default();
        let folder_id = FolderOps::create_folder(&mut manifest, "Folder".into(), None).unwrap();
        manifest.entries.push(entry(
            "asset-1",
            ClipType::Video,
            "vid.mp4",
            Some(&folder_id),
        ));

        let mut timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: HashSet::new(),
            tracks: vec![
                Track {
                    id: "track-del".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: true,
                   display_height: 50.0,
                    clips: vec![make_clip("clip-1", "asset-1", 0, 30)],
                },
                Track {
                    id: "track-keep".into(),
                    r#type: ClipType::Audio,
                    muted: false,
                    hidden: false,
                    sync_locked: true,
                   display_height: 50.0,
                    clips: vec![make_clip("clip-2", "asset-2", 0, 60)],
                },
            ],
            ..Default::default()
        };

        let effect = FolderOps::delete_folder_with_timeline_effects(
            &mut manifest,
            &mut timeline,
            &folder_id,
        )
        .unwrap();

        assert!(effect.pruned_track_ids.contains(&"track-del".to_string()));
        assert!(!effect.pruned_track_ids.contains(&"track-keep".to_string()));
        assert_eq!(timeline.tracks.len(), 1);
        assert_eq!(timeline.tracks[0].id, "track-keep");
    }

    #[test]
    fn fld_017_removes_deleted_ids_from_selection() {
        let mut manifest = MediaManifest::default();
        let folder_id = FolderOps::create_folder(&mut manifest, "Folder".into(), None).unwrap();
        manifest.entries.push(entry(
            "asset-1",
            ClipType::Video,
            "vid.mp4",
            Some(&folder_id),
        ));

        let mut timeline = timeline_with_clip("clip-1", "asset-1");
        // Add an unrelated selected clip id that should survive
        timeline.selected_clip_ids.insert("other-clip".into());

        FolderOps::delete_folder_with_timeline_effects(&mut manifest, &mut timeline, &folder_id)
            .unwrap();

        assert!(!timeline.selected_clip_ids.contains("clip-1"));
        assert!(timeline.selected_clip_ids.contains("other-clip"));
    }

    #[test]
    fn fld_016_preview_tab_ids_returned() {
        let mut manifest = MediaManifest::default();
        let folder_id = FolderOps::create_folder(&mut manifest, "Folder".into(), None).unwrap();
        manifest
            .entries
            .push(entry("asset-a", ClipType::Video, "a.mp4", Some(&folder_id)));
        manifest
            .entries
            .push(entry("asset-b", ClipType::Image, "b.png", Some(&folder_id)));

        let mut timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: HashSet::new(),
            tracks: vec![],
            ..Default::default()
        };

        let effect = FolderOps::delete_folder_with_timeline_effects(
            &mut manifest,
            &mut timeline,
            &folder_id,
        )
        .unwrap();

        // FLD-016: Caller should close preview tabs for these asset ids
        assert!(effect.deleted_asset_ids.contains(&"asset-a".to_string()));
        assert!(effect.deleted_asset_ids.contains(&"asset-b".to_string()));
    }

    #[test]
    fn fld_014_017_subtree_nested_folders() {
        let mut manifest = MediaManifest::default();
        let root_id = FolderOps::create_folder(&mut manifest, "Root".into(), None).unwrap();
        let child_id =
            FolderOps::create_folder(&mut manifest, "Child".into(), Some(root_id.clone())).unwrap();
        manifest.entries.push(entry(
            "asset-child",
            ClipType::Audio,
            "sound.mp3",
            Some(&child_id),
        ));
        manifest.entries.push(entry(
            "asset-root",
            ClipType::Video,
            "root.mp4",
            Some(&root_id),
        ));

        let mut timeline = timeline_with_clip("clip-child", "asset-child");
        timeline.tracks.push(Track {
            id: "track-2".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: true,
           display_height: 50.0,
            clips: vec![make_clip("clip-root", "asset-root", 30, 30)],
        });

        // Delete the root folder — should cascade to child
        let effect =
            FolderOps::delete_folder_with_timeline_effects(&mut manifest, &mut timeline, &root_id)
                .unwrap();

        assert!(effect.deleted_folder_ids.contains(&root_id));
        assert!(effect.deleted_folder_ids.contains(&child_id));
        assert!(effect
            .deleted_asset_ids
            .contains(&"asset-child".to_string()));
        assert!(effect.deleted_asset_ids.contains(&"asset-root".to_string()));
        assert!(effect.removed_clip_ids.contains(&"clip-child".to_string()));
        assert!(effect.removed_clip_ids.contains(&"clip-root".to_string()));
        // Both tracks should be empty and thus pruned
        assert_eq!(effect.pruned_track_ids.len(), 2);
        assert!(timeline.tracks.is_empty());
    }
}
