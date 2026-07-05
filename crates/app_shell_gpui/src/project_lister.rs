//! Host `ProjectLister` for the `get_projects` agent tool: reads the recents
//! registry and reports the active project (read-only).

use agent_contract::{KnownProject, ProjectLister};
use std::path::PathBuf;

pub struct AgentProjectLister {
    registry_path: PathBuf,
    active_root: Option<PathBuf>,
}

impl AgentProjectLister {
    pub fn new(registry_path: PathBuf, active_root: Option<PathBuf>) -> Self {
        Self {
            registry_path,
            active_root,
        }
    }
}

/// Same-path check tolerant of Windows case/separator differences: compare
/// canonical forms when both resolve, else fall back to direct equality.
fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}

fn project_name(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Project")
        .to_string()
}

impl ProjectLister for AgentProjectLister {
    fn list(&self) -> Result<(Vec<KnownProject>, Option<(String, String)>), String> {
        let registry = crate::project_registry_store::load_from(&self.registry_path);
        let projects = registry
            .sorted_entries()
            .into_iter()
            .map(|e| {
                let is_active = self
                    .active_root
                    .as_deref()
                    .is_some_and(|root| same_path(root, &e.url));
                KnownProject {
                    id: e.id.clone(),
                    name: project_name(&e.url),
                    path: e.url.display().to_string(),
                    // Fronda holds one open project: open == active.
                    is_open: is_active,
                    is_active,
                }
            })
            .collect();
        let active = self
            .active_root
            .as_ref()
            .map(|root| (project_name(root), root.display().to_string()));
        Ok((projects, active))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_recents_and_flags_the_active_project() {
        let dir = std::env::temp_dir().join("fronda-project-lister-tests");
        let _ = std::fs::create_dir_all(&dir);
        let registry_path = dir.join("registry.json");
        let _ = std::fs::remove_file(&registry_path);

        let a = dir.join("A.palmier");
        let b = dir.join("B.palmier");
        crate::project_registry_store::record_opened_at(&registry_path, &a).unwrap();
        crate::project_registry_store::record_opened_at(&registry_path, &b).unwrap();

        let lister = AgentProjectLister::new(registry_path.clone(), Some(b.clone()));
        let (projects, active) = lister.list().unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "B", "most recently opened first");
        assert!(projects[0].is_active && projects[0].is_open);
        assert!(!projects[1].is_active);
        let (name, path) = active.unwrap();
        assert_eq!(name, "B");
        assert!(path.ends_with("B.palmier"));

        // No open project: no active entry, nothing flagged.
        let lister = AgentProjectLister::new(registry_path, None);
        let (projects, active) = lister.list().unwrap();
        assert!(active.is_none());
        assert!(projects.iter().all(|p| !p.is_active));
    }
}
