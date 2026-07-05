//! Host `ProjectNavigator` for open_project/new_project: loads or creates a
//! `.palmier` package and returns the full replacement state. Runs INSIDE the
//! executor lock, so it must never touch the executor — it updates the hub's
//! shared project-root handle directly and hands back freshly-built seams.

use agent_contract::{OpenedProject, ProjectNavigator, ProjectSeams};
use core_model::Timeline;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct AppProjectNavigator {
    registry_path: PathBuf,
    /// Shared with the hub; updated on a successful open/create.
    project_root: Arc<Mutex<Option<PathBuf>>>,
}

impl AppProjectNavigator {
    pub fn new(registry_path: PathBuf, project_root: Arc<Mutex<Option<PathBuf>>>) -> Self {
        Self {
            registry_path,
            project_root,
        }
    }

    fn seams_for(&self, root: &Path) -> ProjectSeams {
        ProjectSeams {
            matte_writer: Arc::new(crate::matte_writer::ProjectMatteWriter::new(
                root.to_path_buf(),
            )),
            audio_source: Arc::new(crate::audio_source::ProjectAudioSource::new(
                root.to_path_buf(),
            )),
            export_host: Arc::new(crate::export_host::AgentExportHost::new(root.to_path_buf())),
            project_lister: Arc::new(crate::project_lister::AgentProjectLister::new(
                self.registry_path.clone(),
                Some(root.to_path_buf()),
            )),
        }
    }

    fn finish(&self, root: PathBuf, bundle: project_io::ProjectBundle) -> OpenedProject {
        if let Err(reason) =
            crate::project_registry_store::record_opened_at(&self.registry_path, &root)
        {
            eprintln!("Failed to record recent project: {reason}");
        }
        if let Ok(mut current) = self.project_root.lock() {
            *current = Some(root.clone());
        }
        let seams = self.seams_for(&root);
        OpenedProject {
            name: root
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Project")
                .to_string(),
            root: root.display().to_string(),
            timeline: bundle.timeline,
            sibling_timelines: bundle.multi.siblings,
            manifest: bundle.manifest.unwrap_or_default(),
            seams,
        }
    }
}

impl ProjectNavigator for AppProjectNavigator {
    fn open(&self, id: Option<&str>, path: Option<&str>) -> Result<OpenedProject, String> {
        let root = match (id, path) {
            (_, Some(p)) => PathBuf::from(p),
            (Some(id), None) => {
                let registry = crate::project_registry_store::load_from(&self.registry_path);
                registry
                    .sorted_entries()
                    .into_iter()
                    .find(|e| e.id == id)
                    .map(|e| e.url.clone())
                    .ok_or_else(|| {
                        format!("No known project with id '{id}'. get_projects lists them.")
                    })?
            }
            (None, None) => return Err("open_project requires 'id' or 'path'.".to_string()),
        };
        let bundle = project_io::ProjectBundle::open(&root).map_err(|e| e.to_string())?;
        Ok(self.finish(bundle.root.clone(), bundle))
    }

    fn create(&self, name: Option<&str>) -> Result<OpenedProject, String> {
        let name = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Untitled Project");
        let base = std::env::home_dir()
            .map(|h| h.join("Documents").join("Palmier Pro"))
            .ok_or_else(|| "Could not resolve the user's home directory.".to_string())?;
        std::fs::create_dir_all(&base)
            .map_err(|e| format!("Could not create the Palmier Pro folder: {e}"))?;
        let root = base.join(format!("{name}.palmier"));
        if root.exists() {
            return Err(format!(
                "A project named '{name}' already exists \u{2014} pick another name."
            ));
        }
        project_io::save_project_state(&root, &Timeline::default(), &Default::default())
            .map_err(|e| e.to_string())?;
        let bundle = project_io::ProjectBundle::open(&root).map_err(|e| e.to_string())?;
        Ok(self.finish(root, bundle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env(name: &str) -> (PathBuf, Arc<Mutex<Option<PathBuf>>>, AppProjectNavigator) {
        let dir = std::env::temp_dir().join("fronda-project-navigator").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let registry = dir.join("registry.json");
        let root_handle = Arc::new(Mutex::new(None));
        let nav = AppProjectNavigator::new(registry, Arc::clone(&root_handle));
        (dir, root_handle, nav)
    }

    #[test]
    fn open_by_path_loads_and_updates_the_shared_root() {
        let (dir, root_handle, nav) = temp_env("open");
        let pkg = dir.join("Demo.palmier");
        project_io::save_project_state(
            &pkg,
            &core_model::Timeline {
                fps: 24,
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap();

        let opened = nav.open(None, Some(&pkg.display().to_string())).unwrap();
        assert_eq!(opened.name, "Demo");
        assert_eq!(opened.timeline.fps, 24);
        assert_eq!(
            root_handle.lock().unwrap().as_deref(),
            Some(pkg.as_path()),
            "hub root updated without touching the executor lock"
        );

        // The registry recorded it, so open-by-id now resolves too.
        let registry = crate::project_registry_store::load_from(
            &dir.join("registry.json"),
        );
        let id = registry.sorted_entries()[0].id.clone();
        let reopened = nav.open(Some(&id), None).unwrap();
        assert_eq!(reopened.name, "Demo");
        assert!(nav.open(Some("ghost"), None).is_err());
    }

    #[test]
    fn create_refuses_duplicates() {
        // create() writes under ~/Documents/Palmier Pro; exercise only the
        // duplicate guard against a fabricated existing package to avoid
        // touching the real user folder in tests beyond one marker dir.
        let (_dir, _root, nav) = temp_env("create");
        let base = std::env::home_dir().unwrap().join("Documents").join("Palmier Pro");
        std::fs::create_dir_all(&base).unwrap();
        let marker = base.join("fronda-test-existing.palmier");
        std::fs::create_dir_all(&marker).unwrap();
        let err = match nav.create(Some("fronda-test-existing")) {
            Err(e) => e,
            Ok(_) => panic!("duplicate create must refuse"),
        };
        assert!(err.contains("already exists"), "{err}");
        let _ = std::fs::remove_dir_all(&marker);
    }
}
