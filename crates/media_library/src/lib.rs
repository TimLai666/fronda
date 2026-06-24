use core_model::{ClipType, MediaFolder, MediaManifest, MediaManifestEntry, MediaSource};
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
        result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
            .filter(|e| {
                e.folder_id
                    .as_ref()
                    .map_or(false, |f| folder_ids.contains(f))
            })
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

        // Identify offline assets (no cached_remote_url, and marked missing)
        let offline_ids: HashSet<String> =
            manifest.missing_entry_ids(|_| true).into_iter().collect();
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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("root1", "Root One", None),
            folder("root2", "Root Two", None),
            folder("child1", "Child One", Some("root1")),
            folder("child2", "Child Two", Some("root1")),
            folder("grandchild", "Grandchild", Some("child1")),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("b", "Beta", None),
            folder("a", "alpha", None),
            folder("c", "CHARLIE", None),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("a", "A", None),
            folder("b", "B", Some("a")),
            folder("c", "C", Some("b")),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("a", "A", Some("b")), // a's parent is b
            folder("b", "B", Some("a")), // b's parent is a → cycle
        ];

        let result = FolderOps::folder_path(&manifest, "a");
        assert_eq!(result, Err(FolderError::DescendantCycle));

        let result = FolderOps::folder_path(&manifest, "b");
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    #[test]
    fn fld_005_self_cycle_terminates_safely() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![folder("a", "A", Some("a"))]; // self-loop

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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("a", "A", None),
            folder("b", "B", None),
            folder("c", "C", Some("a")),
        ];

        FolderOps::move_folder(&mut manifest, "c", Some("b".to_string())).unwrap();
        let c = manifest.folders.iter().find(|f| f.id == "c").unwrap();
        assert_eq!(c.parent_folder_id, Some("b".to_string()));
    }

    #[test]
    fn fld_009_move_folder_to_root() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![folder("a", "A", None), folder("b", "B", Some("a"))];

        FolderOps::move_folder(&mut manifest, "b", None).unwrap();
        let b = manifest.folders.iter().find(|f| f.id == "b").unwrap();
        assert_eq!(b.parent_folder_id, None);
    }

    // =======================================================================
    // FLD-010: Reject self-parenting
    // =======================================================================
    #[test]
    fn fld_010_reject_self_parenting() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![folder("a", "A", None)];

        let result = FolderOps::move_folder(&mut manifest, "a", Some("a".to_string()));
        assert_eq!(result, Err(FolderError::SelfParenting));
    }

    // =======================================================================
    // FLD-011: Reject descendant cycle
    // =======================================================================
    #[test]
    fn fld_011_reject_move_under_descendant() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("a", "A", None),
            folder("b", "B", Some("a")),
            folder("c", "C", Some("b")),
        ];

        // Moving "a" under "c" would create A → B → C → A cycle
        let result = FolderOps::move_folder(&mut manifest, "a", Some("c".to_string()));
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    #[test]
    fn fld_011_reject_move_parent_under_child() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![folder("p", "Parent", None), folder("c", "Child", Some("p"))];

        let result = FolderOps::move_folder(&mut manifest, "p", Some("c".to_string()));
        assert_eq!(result, Err(FolderError::DescendantCycle));
    }

    // =======================================================================
    // FLD-012/013: Delete folder recursively
    // =======================================================================
    #[test]
    fn fld_012_delete_folder_recursive() {
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![
            folder("a", "A", None),
            folder("b", "B", Some("a")),
            folder("c", "C", Some("b")),
            folder("other", "Other", None),
        ];
        manifest.entries = vec![
            entry("e1", ClipType::Video, "vid1.mp4", Some("a")),
            entry("e2", ClipType::Video, "vid2.mp4", Some("b")),
            entry("e3", ClipType::Video, "vid3.mp4", None),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("e1", ClipType::Video, "vid1.mp4", None),
            entry("e2", ClipType::Video, "vid2.mp4", None),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.folders = vec![folder("a", "A", None), folder("b", "B", Some("a"))];
        manifest.entries = vec![
            entry("e1", ClipType::Video, "vid1.mp4", Some("a")),
            entry("e2", ClipType::Video, "vid2.mp4", Some("b")),
            entry("e3", ClipType::Video, "vid3.mp4", None),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("off1", ClipType::Video, "vid1.mp4", None),
            entry_cached("cached1", "vid2.mp4"),
        ];

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
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![entry("e1", ClipType::Video, "vid1.mp4", None)];

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
        let mut manifest = MediaManifest::default();
        manifest.entries = vec![
            entry("off1", ClipType::Video, "vid1.mp4", None),
            entry("off2", ClipType::Audio, "track.wav", None),
            entry_cached("cached1", "vid3.mp4"),
        ];

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
}
