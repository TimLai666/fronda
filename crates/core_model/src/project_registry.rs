use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// A single entry in the recent-project registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectEntry {
    /// Unique identifier for this entry (UUID).
    pub id: String,
    /// File URL path to the .palmier package.
    pub url: PathBuf,
    /// When the project was first registered.
    pub created_date: DateTime<Utc>,
    /// When the project was last opened or registered.
    pub last_opened_date: DateTime<Utc>,
}

impl ProjectEntry {
    /// Derive the display name from the package filename stem (REC-010).
    pub fn name(&self) -> String {
        self.url
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string()
    }

    /// Check whether the stored file path currently exists (REC-011).
    /// Uses an injected `path_exists` callback for testability.
    pub fn is_accessible(&self, path_exists: impl Fn(&PathBuf) -> bool) -> bool {
        path_exists(&self.url)
    }
}

/// The recent-project registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectRegistry {
    /// All entries, unsorted. Serialized as-is.
    pub entries: Vec<ProjectEntry>,
}

impl ProjectRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Find an entry by its file URL (for dedup, REC-002).
    pub fn find_by_url(&self, url: &PathBuf) -> Option<&ProjectEntry> {
        self.entries.iter().find(|e| e.url == *url)
    }

    /// Find an entry by id (REC-003 lookup variant).
    pub fn find_by_id(&self, id: &str) -> Option<&ProjectEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Find mutable entry by id.
    pub fn find_by_id_mut(&mut self, id: &str) -> Option<&mut ProjectEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Register a project. If already known by URL, update lastOpenedDate (REC-003).
    /// If new, create a fresh entry (REC-004).
    /// Returns the entry id.
    pub fn register(&mut self, url: PathBuf, now: DateTime<Utc>) -> String {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.url == url) {
            entry.last_opened_date = now;
            entry.id.clone()
        } else {
            let id = Uuid::new_v4().to_string();
            self.entries.push(ProjectEntry {
                id: id.clone(),
                url,
                created_date: now,
                last_opened_date: now,
            });
            id
        }
    }

    /// Remove a recent project entry by id (REC-005).
    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    /// Find an entry by its file URL (mutable, for internal use).
    pub fn find_by_url_mut(&mut self, url: &PathBuf) -> Option<&mut ProjectEntry> {
        self.entries.iter_mut().find(|e| e.url == *url)
    }

    /// Update a project's URL (REC-008).
    pub fn update_url(&mut self, id: &str, new_url: PathBuf, now: DateTime<Utc>) -> bool {
        if let Some(entry) = self.find_by_id_mut(id) {
            entry.url = new_url;
            entry.last_opened_date = now;
            true
        } else {
            false
        }
    }

    /// PRJ-015: Update the registry when a project file is renamed or moved.
    ///
    /// Finds the entry matching `old_url`, updates it to `new_url`,
    /// and updates `last_opened_date`. Returns the entry id if found.
    pub fn rename_project(
        &mut self,
        old_url: &PathBuf,
        new_url: PathBuf,
        now: DateTime<Utc>,
    ) -> Option<String> {
        let entry = self.entries.iter_mut().find(|e| e.url == *old_url)?;
        entry.url = new_url;
        entry.last_opened_date = now;
        Some(entry.id.clone())
    }

    /// Get entries sorted by descending lastOpenedDate (REC-009).
    pub fn sorted_entries(&self) -> Vec<&ProjectEntry> {
        let mut sorted: Vec<&ProjectEntry> = self.entries.iter().collect();
        sorted.sort_by_key(|entry| std::cmp::Reverse(entry.last_opened_date));
        sorted
    }
}

impl Default for ProjectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn rec_004_register_new_creates_entry() {
        let mut reg = ProjectRegistry::new();
        let now = utc(1000);
        let id = reg.register(PathBuf::from("/path/to/project.palmier"), now);
        assert_eq!(reg.entries.len(), 1);
        assert_eq!(reg.entries[0].id, id);
        assert_eq!(reg.entries[0].created_date, now);
        assert_eq!(reg.entries[0].last_opened_date, now);
    }

    #[test]
    fn rec_003_register_known_updates_date() {
        let mut reg = ProjectRegistry::new();
        let t1 = utc(1000);
        let t2 = utc(2000);
        let url = PathBuf::from("/path/to/project.palmier");
        let id = reg.register(url.clone(), t1);
        assert_eq!(reg.entries[0].last_opened_date, t1);
        let same_id = reg.register(url.clone(), t2);
        assert_eq!(id, same_id, "same id preserved");
        assert_eq!(reg.entries.len(), 1, "no duplicate entry");
        assert_eq!(reg.entries[0].last_opened_date, t2);
    }

    #[test]
    fn rec_002_dedup_by_url() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/a.palmier"), utc(1000));
        reg.register(PathBuf::from("/b.palmier"), utc(1000));
        reg.register(PathBuf::from("/a.palmier"), utc(2000));
        assert_eq!(reg.entries.len(), 2);
    }

    #[test]
    fn rec_005_remove_does_not_check_disk() {
        let mut reg = ProjectRegistry::new();
        let id = reg.register(PathBuf::from("/nonexistent.palmier"), utc(1000));
        assert!(reg.remove(&id));
        assert_eq!(reg.entries.len(), 0);
    }

    #[test]
    fn rec_005_remove_missing_id_returns_false() {
        let mut reg = ProjectRegistry::new();
        assert!(!reg.remove("nonexistent-id"));
    }

    #[test]
    fn rec_007_remove_missing_package_still_removes_entry() {
        let mut reg = ProjectRegistry::new();
        let id = reg.register(PathBuf::from("/gone.palmier"), utc(1000));
        assert!(reg.remove(&id));
        assert_eq!(reg.entries.len(), 0);
    }

    #[test]
    fn rec_008_update_url() {
        let mut reg = ProjectRegistry::new();
        let id = reg.register(PathBuf::from("/old.palmier"), utc(1000));
        let t2 = utc(2000);
        assert!(reg.update_url(&id, PathBuf::from("/new.palmier"), t2));
        assert_eq!(reg.entries[0].url, PathBuf::from("/new.palmier"));
        assert_eq!(reg.entries[0].last_opened_date, t2);
    }

    #[test]
    fn rec_008_update_url_unknown_id_returns_false() {
        let mut reg = ProjectRegistry::new();
        assert!(!reg.update_url("bad-id", PathBuf::from("/x.palmier"), utc(1000)));
    }

    #[test]
    fn rec_009_sorted_by_descending_last_opened() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/a.palmier"), utc(100));
        let b_id = reg.register(PathBuf::from("/b.palmier"), utc(300));
        reg.register(PathBuf::from("/c.palmier"), utc(200));
        // Re-register b to update its date to latest
        reg.register(PathBuf::from("/b.palmier"), utc(400));
        let sorted = reg.sorted_entries();
        assert_eq!(sorted[0].id, b_id);
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn rec_010_name_from_filename_stem() {
        let entry = ProjectEntry {
            id: "test-id".into(),
            url: PathBuf::from("/path/to/My Project.palmier"),
            created_date: utc(1000),
            last_opened_date: utc(1000),
        };
        assert_eq!(entry.name(), "My Project");
    }

    #[test]
    fn rec_010_name_fallback_for_no_stem() {
        let entry = ProjectEntry {
            id: "test-id".into(),
            url: PathBuf::from("/"),
            created_date: utc(1000),
            last_opened_date: utc(1000),
        };
        assert_eq!(entry.name(), "Unknown");
    }

    #[test]
    fn rec_011_is_accessible_with_callback() {
        let entry = ProjectEntry {
            id: "test".into(),
            url: PathBuf::from("/exists.palmier"),
            created_date: utc(1000),
            last_opened_date: utc(1000),
        };
        assert!(entry.is_accessible(|p| p.to_str() == Some("/exists.palmier")));
        assert!(!entry.is_accessible(|_| false));
    }

    #[test]
    fn rec_012_inaccessible_entries_remain_in_registry() {
        let mut reg = ProjectRegistry::new();
        let id = reg.register(PathBuf::from("/missing.palmier"), utc(1000));
        // The entry is still in the registry even if the file doesn't exist
        assert!(reg.find_by_id(&id).is_some());
        assert!(reg.remove(&id)); // can still be removed
    }

    #[test]
    fn find_by_url_returns_none_for_unknown() {
        let reg = ProjectRegistry::new();
        assert!(reg.find_by_url(&PathBuf::from("/x.palmier")).is_none());
    }

    #[test]
    fn find_by_id_returns_none_for_unknown() {
        let reg = ProjectRegistry::new();
        assert!(reg.find_by_id("bad-id").is_none());
    }

    #[test]
    fn empty_registry_sorted_returns_empty() {
        let reg = ProjectRegistry::new();
        assert!(reg.sorted_entries().is_empty());
    }

    #[test]
    fn serde_round_trip() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/a.palmier"), utc(100));
        reg.register(PathBuf::from("/b.palmier"), utc(200));
        let json = serde_json::to_string_pretty(&reg).unwrap();
        let restored: ProjectRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(reg, restored);
    }

    // ── PRJ-015: Rename/move project ────────────────────────────

    #[test]
    fn prj_015_rename_project_updates_url() {
        let mut reg = ProjectRegistry::new();
        let id = reg.register(PathBuf::from("/old/path.palmier"), utc(1000));
        let t2 = utc(2000);
        let result = reg.rename_project(
            &PathBuf::from("/old/path.palmier"),
            PathBuf::from("/new/path.palmier"),
            t2,
        );
        assert_eq!(result, Some(id.clone()));
        let entry = reg.find_by_id(&id).unwrap();
        assert_eq!(entry.url, PathBuf::from("/new/path.palmier"));
        assert_eq!(entry.last_opened_date, t2);
    }

    #[test]
    fn prj_015_rename_unknown_url_returns_none() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/exists.palmier"), utc(1000));
        let result = reg.rename_project(
            &PathBuf::from("/unknown.palmier"),
            PathBuf::from("/new.palmier"),
            utc(2000),
        );
        assert!(result.is_none());
    }

    #[test]
    fn prj_015_rename_empty_registry_returns_none() {
        let mut reg = ProjectRegistry::new();
        let result = reg.rename_project(
            &PathBuf::from("/any.palmier"),
            PathBuf::from("/new.palmier"),
            utc(1000),
        );
        assert!(result.is_none());
    }

    #[test]
    fn prj_015_rename_to_existing_url_allowed() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/a.palmier"), utc(100));
        let id_b = reg.register(PathBuf::from("/b.palmier"), utc(200));
        // Rename b to a's old path (simulating swap)
        let result = reg.rename_project(
            &PathBuf::from("/b.palmier"),
            PathBuf::from("/a_new.palmier"),
            utc(300),
        );
        assert_eq!(result, Some(id_b));
    }

    #[test]
    fn prj_015_rename_preserves_other_entries() {
        let mut reg = ProjectRegistry::new();
        reg.register(PathBuf::from("/keep.palmier"), utc(100));
        reg.register(PathBuf::from("/move.palmier"), utc(200));
        reg.rename_project(
            &PathBuf::from("/move.palmier"),
            PathBuf::from("/moved.palmier"),
            utc(300),
        );
        assert_eq!(reg.entries.len(), 2);
        assert!(reg.find_by_url(&PathBuf::from("/keep.palmier")).is_some());
        assert!(reg.find_by_url(&PathBuf::from("/moved.palmier")).is_some());
        assert!(reg.find_by_url(&PathBuf::from("/move.palmier")).is_none());
    }
}
