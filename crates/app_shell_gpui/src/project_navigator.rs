//! Host `ProjectNavigator` for manage_project open/create/close (upstream
//! #299): loads, creates, or saves-and-closes a `.palmier` package and
//! returns the full replacement state. Runs INSIDE the executor lock, so it
//! must never touch the executor — it updates the hub's shared project-root
//! handle directly and hands back freshly-built seams.

use agent_contract::{
    ActiveProjectState, ClosedProject, OpenedProject, ProjectNavigator, ProjectSeams,
};
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
            // Same construction as the hub's install point — the provider must
            // resolve project-relative media against the NEW root.
            #[cfg(feature = "transcribe-local")]
            transcription_provider: Some(Arc::new(crate::transcribe::WhisperTranscriber::new(
                root.to_path_buf(),
                crate::pane_prefs::default_prefs_path(),
            ))),
            #[cfg(not(feature = "transcribe-local"))]
            transcription_provider: None,
            // Same root-consistency requirement as the transcription provider.
            #[cfg(feature = "vad")]
            speech_analyzer: Some(Arc::new(crate::vad::VadSpeechAnalyzer::new(
                root.to_path_buf(),
            ))),
            #[cfg(not(feature = "vad"))]
            speech_analyzer: None,
        }
    }

    /// Resolve a name/id/path selector against the recents registry (name is
    /// case-insensitive and must be unique). Error messages mirror upstream
    /// #299 resolveProjectURL verbatim.
    fn resolve_target(
        &self,
        name: Option<&str>,
        id: Option<&str>,
        path: Option<&str>,
    ) -> Result<Option<PathBuf>, String> {
        if let Some(p) = path {
            return Ok(Some(PathBuf::from(p)));
        }
        if let Some(id) = id {
            let registry = crate::project_registry_store::load_from(&self.registry_path);
            return registry
                .sorted_entries()
                .into_iter()
                .find(|e| e.id == id)
                .map(|e| Some(e.url.clone()))
                .ok_or_else(|| {
                    format!(
                        "No project with id {id}. Call manage_project with action='list' for valid ids."
                    )
                });
        }
        let Some(name) = name else { return Ok(None) };
        let wanted = name.trim();
        let registry = crate::project_registry_store::load_from(&self.registry_path);
        let entries = registry.sorted_entries();
        let matches: Vec<PathBuf> = entries
            .iter()
            .map(|e| e.url.clone())
            .filter(|url| crate::project_lister::project_name(url).eq_ignore_ascii_case(wanted))
            .collect();
        match matches.len() {
            1 => Ok(Some(matches.into_iter().next().expect("len checked"))),
            0 => {
                let known = entries
                    .iter()
                    .take(15)
                    .map(|e| crate::project_lister::project_name(&e.url))
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(format!(
                    "No project named '{wanted}'. Known projects: {known}. Call manage_project with action='list' for the full list."
                ))
            }
            n => {
                let candidates = matches
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                Err(format!(
                    "{n} projects are named '{wanted}'. Pick one by path: {candidates}"
                ))
            }
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
            multicam_groups: bundle.multi.multicam_groups.unwrap_or_default(),
            seams,
        }
    }
}

impl ProjectNavigator for AppProjectNavigator {
    fn open(
        &self,
        name: Option<&str>,
        id: Option<&str>,
        path: Option<&str>,
    ) -> Result<OpenedProject, String> {
        let root = self.resolve_target(name, id, path)?.ok_or_else(|| {
            "manage_project action='open' needs a name, an id from action='list', or a path."
                .to_string()
        })?;
        if !root.exists() {
            return Err(format!("No project at {}.", root.display()));
        }
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

    /// manage_project action='close': Fronda holds ONE open project, so
    /// "open" means "the active one". Save-first; a save failure leaves the
    /// project open. No next project takes over — the Home state follows.
    fn close(
        &self,
        name: Option<&str>,
        id: Option<&str>,
        path: Option<&str>,
        active: ActiveProjectState,
    ) -> Result<ClosedProject, String> {
        let current = self.project_root.lock().ok().and_then(|g| g.clone());
        let target: PathBuf = match self.resolve_target(name, id, path)? {
            Some(t) => t,
            None => current
                .clone()
                .ok_or_else(|| "No project is open.".to_string())?,
        };
        let is_open = current
            .as_deref()
            .is_some_and(|c| crate::project_lister::same_path(c, &target));
        if !is_open {
            return Err(format!("Project at {} isn't open.", target.display()));
        }
        let name = crate::project_lister::project_name(&target);
        project_io::save_project_state_with_siblings_and_groups(
            &target,
            &active.timeline,
            &active.sibling_timelines,
            &active.manifest,
            Some(active.multicam_groups),
        )
        .map_err(|e| format!("Couldn't save '{name}' — project left open. {e}"))?;
        if let Ok(mut cur) = self.project_root.lock() {
            *cur = None;
        }
        Ok(ClosedProject {
            name,
            open_count: 0,
            was_active: true,
            next_active: None,
            lister: Some(Arc::new(crate::project_lister::AgentProjectLister::new(
                self.registry_path.clone(),
                None,
            ))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_env(name: &str) -> (PathBuf, Arc<Mutex<Option<PathBuf>>>, AppProjectNavigator) {
        let dir = std::env::temp_dir()
            .join("fronda-project-navigator")
            .join(name);
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

        let opened = nav
            .open(None, None, Some(&pkg.display().to_string()))
            .unwrap();
        assert_eq!(opened.name, "Demo");
        assert_eq!(opened.timeline.fps, 24);
        assert_eq!(
            root_handle.lock().unwrap().as_deref(),
            Some(pkg.as_path()),
            "hub root updated without touching the executor lock"
        );

        // The registry recorded it, so open-by-id now resolves too.
        let registry = crate::project_registry_store::load_from(&dir.join("registry.json"));
        let id = registry.sorted_entries()[0].id.clone();
        let reopened = nav.open(None, Some(&id), None).unwrap();
        assert_eq!(reopened.name, "Demo");
        let err = nav.open(None, Some("ghost"), None).unwrap_err();
        assert!(err.contains("No project with id ghost"), "{err}");
        // A missing package errors with the upstream message.
        let gone = dir.join("Gone.palmier");
        let err = nav
            .open(None, None, Some(&gone.display().to_string()))
            .unwrap_err();
        assert!(err.starts_with("No project at "), "{err}");
    }

    #[test]
    fn open_by_name_is_case_insensitive_and_refuses_ambiguity() {
        let (dir, _root, nav) = temp_env("open-by-name");
        let pkg = dir.join("Demo.palmier");
        project_io::save_project_state(&pkg, &core_model::Timeline::default(), &Default::default())
            .unwrap();
        crate::project_registry_store::record_opened_at(&dir.join("registry.json"), &pkg).unwrap();

        // Case differs from the registered name (upstream #299).
        let opened = nav.open(Some("demo"), None, None).unwrap();
        assert_eq!(opened.name, "Demo");

        // Unknown name errors with the upstream message.
        let err = nav.open(Some("Ghost"), None, None).unwrap_err();
        assert!(err.contains("No project named 'Ghost'"), "{err}");
        assert!(err.contains("Known projects: Demo"), "{err}");
        assert!(
            err.contains("Call manage_project with action='list' for the full list."),
            "{err}"
        );

        // A same-name project in another folder makes the name ambiguous.
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let twin = sub.join("Demo.palmier");
        project_io::save_project_state(
            &twin,
            &core_model::Timeline::default(),
            &Default::default(),
        )
        .unwrap();
        crate::project_registry_store::record_opened_at(&dir.join("registry.json"), &twin).unwrap();
        let err = nav.open(Some("DEMO"), None, None).unwrap_err();
        assert!(err.contains("2 projects are named 'DEMO'"), "{err}");
        assert!(err.contains("Pick one by path:"), "{err}");
    }

    fn empty_state() -> ActiveProjectState {
        ActiveProjectState {
            timeline: core_model::Timeline {
                fps: 50,
                ..Default::default()
            },
            sibling_timelines: Vec::new(),
            manifest: Default::default(),
            multicam_groups: Vec::new(),
        }
    }

    #[test]
    fn close_saves_first_then_clears_the_root() {
        let (dir, root_handle, nav) = temp_env("close");
        let pkg = dir.join("Demo.palmier");
        project_io::save_project_state(&pkg, &core_model::Timeline::default(), &Default::default())
            .unwrap();
        nav.open(None, None, Some(&pkg.display().to_string()))
            .unwrap();
        assert!(root_handle.lock().unwrap().is_some());

        let closed = nav.close(None, None, None, empty_state()).unwrap();
        assert_eq!(closed.name, "Demo");
        assert_eq!(closed.open_count, 0);
        assert!(closed.was_active);
        assert!(closed.next_active.is_none());
        assert!(
            closed.lister.is_some(),
            "rootless lister for manage_project action='list'"
        );
        assert!(root_handle.lock().unwrap().is_none(), "hub root cleared");
        // Save-first happened: the package now carries the fps-50 timeline.
        let bundle = project_io::ProjectBundle::open(&pkg).unwrap();
        assert_eq!(bundle.timeline.fps, 50);
    }

    #[test]
    fn close_refuses_projects_that_are_not_open() {
        let (dir, _root, nav) = temp_env("close-not-open");
        // No project open at all.
        let err = nav.close(None, None, None, empty_state()).unwrap_err();
        assert!(err.contains("No project is open"), "{err}");

        // Open A, then try to close B by path — B isn't open.
        let a = dir.join("A.palmier");
        project_io::save_project_state(&a, &core_model::Timeline::default(), &Default::default())
            .unwrap();
        nav.open(None, None, Some(&a.display().to_string()))
            .unwrap();
        let b = dir.join("B.palmier");
        let err = nav
            .close(None, None, Some(&b.display().to_string()), empty_state())
            .unwrap_err();
        assert!(err.contains("isn't open"), "{err}");
        // Unknown name errors before the open check.
        let err = nav
            .close(Some("Ghost"), None, None, empty_state())
            .unwrap_err();
        assert!(err.contains("No project named 'Ghost'"), "{err}");
    }

    #[test]
    fn close_resolves_name_case_insensitively() {
        let (dir, root_handle, nav) = temp_env("close-by-name");
        let pkg = dir.join("Demo.palmier");
        project_io::save_project_state(&pkg, &core_model::Timeline::default(), &Default::default())
            .unwrap();
        nav.open(None, None, Some(&pkg.display().to_string()))
            .unwrap();
        let closed = nav.close(Some("demo"), None, None, empty_state()).unwrap();
        assert_eq!(closed.name, "Demo");
        assert!(root_handle.lock().unwrap().is_none());
    }

    #[test]
    fn create_refuses_duplicates() {
        // create() writes under ~/Documents/Palmier Pro; exercise only the
        // duplicate guard against a fabricated existing package to avoid
        // touching the real user folder in tests beyond one marker dir.
        let (_dir, _root, nav) = temp_env("create");
        let base = std::env::home_dir()
            .unwrap()
            .join("Documents")
            .join("Palmier Pro");
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
