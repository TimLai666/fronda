//! Recent-project registry persistence (Fronda-owned state, not part of
//! the .palmier compatibility contract). Pure std + core_model — no gpui.

use std::path::{Path, PathBuf};

use core_model::project_registry::ProjectRegistry;

/// Platform config directory for Fronda-owned state.
pub fn fronda_config_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from(".")).join("Fronda")
}

/// Default registry file location.
pub fn default_registry_path() -> PathBuf {
    fronda_config_dir().join("projects.json")
}

/// Load a registry; missing or corrupt files yield an empty registry.
pub fn load_from(path: &Path) -> ProjectRegistry {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_to(path: &Path, registry: &ProjectRegistry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(registry).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Record a project open/save at `project_path` in the registry file.
pub fn record_opened_at(registry_path: &Path, project_path: &Path) -> Result<(), String> {
    let mut registry = load_from(registry_path);
    registry.register(project_path.to_path_buf(), chrono::Utc::now());
    save_to(registry_path, &registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-registry-store-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn repeat_record_updates_without_duplicating() {
        let path = temp_registry("repeat.json");
        let project = PathBuf::from("C:/projects/demo.palmier");
        record_opened_at(&path, &project).unwrap();
        let first = load_from(&path);
        let first_opened = first.sorted_entries()[0].last_opened_date;

        record_opened_at(&path, &project).unwrap();
        let second = load_from(&path);
        let entries = second.sorted_entries();
        assert_eq!(entries.len(), 1, "no duplicate entry");
        assert!(entries[0].last_opened_date >= first_opened);
    }

    #[test]
    fn sorted_by_last_opened_desc() {
        let path = temp_registry("sorted.json");
        record_opened_at(&path, &PathBuf::from("C:/projects/a.palmier")).unwrap();
        record_opened_at(&path, &PathBuf::from("C:/projects/b.palmier")).unwrap();
        let registry = load_from(&path);
        let entries = registry.sorted_entries();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].url.to_string_lossy().contains("b.palmier"));
    }

    #[test]
    fn corrupt_file_recovers_empty() {
        let path = temp_registry("corrupt.json");
        std::fs::write(&path, "{not json").unwrap();
        let registry = load_from(&path);
        assert!(registry.sorted_entries().is_empty());
        // Next record overwrites the corrupt file.
        record_opened_at(&path, &PathBuf::from("C:/projects/x.palmier")).unwrap();
        assert_eq!(load_from(&path).sorted_entries().len(), 1);
    }
}
