//! Single shared editor state for the shell (Swift: EditorViewModel access).
//!
//! Owns the one `ToolExecutor` both the UI and the MCP server operate on.
//! Pure std + workspace crates — no gpui.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use agent_contract::ToolExecutor;
use core_model::{MediaManifest, Timeline};
use project_io::ProjectBundle;

/// Debounced-autosave trigger decision (upstream #211, mirroring Swift
/// `VideoProject.scheduleProjectCheckpointAutosave`): a coalesced checkpoint
/// save fires only when a project root exists AND the revision advanced past
/// the last saved revision. Rootless (unsaved) projects skip; no advance means
/// the previous save already covered it, so rapid edits between ticks collapse
/// to a single save.
pub fn autosave_should_fire(has_root: bool, current_revision: u64, last_saved_revision: u64) -> bool {
    has_root && current_revision > last_saved_revision
}

/// Process-wide holder of the current project state.
pub struct EditorStateHub {
    executor: Arc<Mutex<ToolExecutor>>,
    project_root: Arc<Mutex<Option<PathBuf>>>,
    /// Recent-project registry file (test constructors inject a temp path).
    registry_path: PathBuf,
    /// Revision persisted by the last successful save / load (autosave
    /// coalescing, #211). An autosave tick fires only when `revision()` has
    /// advanced past this.
    last_saved_revision: AtomicU64,
}

impl EditorStateHub {
    pub fn new() -> Self {
        Self::with_registry_path(crate::project_registry_store::default_registry_path())
    }

    fn with_registry_path(registry_path: PathBuf) -> Self {
        Self {
            executor: Arc::new(Mutex::new(ToolExecutor::new(
                Timeline::default(),
                MediaManifest::default(),
            ))),
            project_root: Arc::new(Mutex::new(None)),
            registry_path,
            last_saved_revision: AtomicU64::new(0),
        }
    }

    /// Record the current revision as the last-saved baseline so the next
    /// autosave tick coalesces (fires only after a further edit).
    fn mark_saved_revision(&self) {
        self.last_saved_revision
            .store(self.revision(), Ordering::Relaxed);
    }

    fn record_in_registry(&self, project_path: &Path) {
        if let Err(reason) =
            crate::project_registry_store::record_opened_at(&self.registry_path, project_path)
        {
            eprintln!("Failed to record recent project: {reason}");
        }
    }

    /// Process-wide instance — one current project per app.
    pub fn global() -> &'static EditorStateHub {
        static INSTANCE: OnceLock<EditorStateHub> = OnceLock::new();
        INSTANCE.get_or_init(EditorStateHub::new)
    }

    /// Shared executor for the MCP server (and any other consumer).
    pub fn executor(&self) -> Arc<Mutex<ToolExecutor>> {
        Arc::clone(&self.executor)
    }

    /// Change counter for UI invalidation. Returns 0 if the lock is poisoned.
    pub fn revision(&self) -> u64 {
        self.executor.lock().map(|e| e.revision()).unwrap_or(0)
    }

    /// Replace the current project state in place. A running MCP server
    /// serves the new state on its next request — no restart needed.
    pub fn load_project(&self, timeline: Timeline, media_manifest: MediaManifest) {
        if let Ok(mut exec) = self.executor.lock() {
            exec.load_project(timeline, media_manifest);
        }
        if let Ok(mut root) = self.project_root.lock() {
            *root = None;
        }
    }

    /// Open a `.palmier` package and load it into the shared state.
    /// On failure the shared state and revision are left untouched.
    pub fn load_bundle(&self, path: &Path) -> Result<(), String> {
        let bundle = ProjectBundle::open(path).map_err(|e| e.to_string())?;
        if let Ok(mut exec) = self.executor.lock() {
            exec.load_project(bundle.timeline, bundle.manifest.unwrap_or_default());
            exec.set_sibling_timelines(bundle.multi.siblings.clone());
            exec.set_multicam_groups(bundle.multi.multicam_groups.clone().unwrap_or_default());
        }
        self.record_in_registry(&bundle.root);
        self.install_matte_writer(bundle.root.clone());
        if let Ok(mut root) = self.project_root.lock() {
            *root = Some(bundle.root);
        }
        // A just-loaded project is already on disk — don't autosave until the
        // next edit (#211 coalescing baseline).
        self.mark_saved_revision();
        Ok(())
    }

    /// Point the project-scoped host seams at the given project package:
    /// `import_media` (#242 mattes) writes into its `media/` directory, and
    /// `remove_silence` (#174) decodes clip audio from it. Called whenever
    /// the project root changes.
    fn install_matte_writer(&self, root: PathBuf) {
        if let Ok(mut exec) = self.executor.lock() {
            exec.set_matte_writer(std::sync::Arc::new(
                crate::matte_writer::ProjectMatteWriter::new(root.clone()),
            ));
            exec.set_audio_source(std::sync::Arc::new(
                crate::audio_source::ProjectAudioSource::new(root.clone()),
            ));
            exec.set_export_host(std::sync::Arc::new(
                crate::export_host::AgentExportHost::new(root.clone()),
            ));
            exec.set_project_lister(std::sync::Arc::new(
                crate::project_lister::AgentProjectLister::new(
                    self.registry_path.clone(),
                    Some(root),
                ),
            ));
            exec.set_project_navigator(std::sync::Arc::new(
                crate::project_navigator::AppProjectNavigator::new(
                    self.registry_path.clone(),
                    Arc::clone(&self.project_root),
                ),
            ));
        }
        // Seam availability changed — tell live MCP sessions (#250).
        if let Ok(service) = crate::mcp_service::McpService::global().lock() {
            service.notify_tools_changed();
        }
    }

    /// Root directory of the currently loaded project, if any.
    pub fn project_root(&self) -> Option<PathBuf> {
        self.project_root.lock().ok().and_then(|r| r.clone())
    }

    /// Snapshot the shared timeline, siblings, manifest, live multicam groups,
    /// and the revision — all under one lock so the revision matches exactly
    /// the state being written (autosave coalescing must not record a revision
    /// newer than what reached disk, #211).
    #[allow(clippy::type_complexity)]
    fn snapshot(
        &self,
    ) -> Result<
        (
            Timeline,
            Vec<Timeline>,
            MediaManifest,
            Vec<core_model::MulticamSource>,
            u64,
        ),
        String,
    > {
        let exec = self
            .executor
            .lock()
            .map_err(|_| "Editor state lock poisoned".to_string())?;
        Ok((
            exec.timeline().clone(),
            exec.sibling_timelines().to_vec(),
            exec.media_manifest().clone(),
            exec.saved_multicam_groups().unwrap_or_default(),
            exec.revision(),
        ))
    }

    /// Write the shared timeline and manifest back to the open project.
    /// Clones the state under the lock, writes to disk outside it.
    pub fn save(&self) -> Result<(), String> {
        let Some(root) = self.project_root() else {
            return Err("No project open: nothing to save".into());
        };
        let (timeline, siblings, manifest, groups, revision) = self.snapshot()?;
        project_io::save_project_state_with_siblings_and_groups(
            &root,
            &timeline,
            &siblings,
            &manifest,
            Some(groups),
        )
        .map_err(|e| e.to_string())?;
        self.last_saved_revision.store(revision, Ordering::Relaxed);
        Ok(())
    }

    /// Persist the current project to its root now (the autosave / show-Home
    /// path, #211). A thin named wrapper over [`save`] the app's autosave timer
    /// and Home transition can call; errors when no project is open.
    pub fn save_now(&self) -> Result<(), String> {
        self.save()
    }

    /// Debounced-autosave tick (#211): saves iff a project root exists and the
    /// revision advanced since the last save/load. Returns `Ok(true)` when it
    /// saved, `Ok(false)` when it coalesced/skipped. Multiple rapid edits
    /// between ticks collapse into one save; a rootless project never autosaves.
    pub fn autosave_if_dirty(&self) -> Result<bool, String> {
        let has_root = self.project_root().is_some();
        let current = self.revision();
        let last = self.last_saved_revision.load(Ordering::Relaxed);
        if !autosave_should_fire(has_root, current, last) {
            return Ok(false);
        }
        self.save_now()?;
        Ok(true)
    }

    /// Write the current state to a new directory and make it the
    /// project root. On write failure the root is left unchanged.
    pub fn save_as(&self, root: &Path) -> Result<(), String> {
        let (timeline, siblings, manifest, groups, revision) = self.snapshot()?;
        project_io::save_project_state_with_siblings_and_groups(
            root,
            &timeline,
            &siblings,
            &manifest,
            Some(groups),
        )
        .map_err(|e| e.to_string())?;
        self.record_in_registry(root);
        self.install_matte_writer(root.to_path_buf());
        if let Ok(mut current) = self.project_root.lock() {
            *current = Some(root.to_path_buf());
        }
        self.last_saved_revision.store(revision, Ordering::Relaxed);
        Ok(())
    }
}

impl Default for EditorStateHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_project_replaces_state_and_bumps_revision() {
        let hub = EditorStateHub::new();
        {
            let exec = hub.executor();
            let mut exec = exec.lock().unwrap();
            exec.execute("organize_media", &serde_json::json!({"createFolders": ["B-roll"]}))
                .unwrap();
        }
        let before = hub.revision();
        assert_eq!(before, 1);

        let timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 60,
            ..Default::default()
        };
        hub.load_project(timeline, MediaManifest::default());

        assert_eq!(hub.revision(), before + 1);
        let exec = hub.executor();
        let exec = exec.lock().unwrap();
        assert_eq!(exec.timeline().fps, 60);
        assert!(exec.media_manifest().folders.is_empty());
        assert!(exec.undo_stack().is_empty());
    }

    #[test]
    fn executor_returns_same_shared_instance() {
        let hub = EditorStateHub::new();
        assert!(Arc::ptr_eq(&hub.executor(), &hub.executor()));
    }

    fn hub_with_temp_registry(name: &str) -> EditorStateHub {
        let dir = std::env::temp_dir().join("fronda-hub-registry-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        EditorStateHub::with_registry_path(path)
    }

    #[test]
    fn load_bundle_records_recent_project() {
        let dir = temp_bundle_dir("recents.palmier");
        std::fs::write(dir.join(core_model::TIMELINE_FILENAME), r#"{"fps":30}"#).unwrap();

        let hub = hub_with_temp_registry("recents.json");
        hub.load_bundle(&dir).unwrap();

        let registry = crate::project_registry_store::load_from(&hub.registry_path);
        let entries = registry.sorted_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].url, dir);
    }

    fn temp_bundle_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("fronda-editor-state-hub-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_bundle_success_loads_state_and_records_root() {
        let dir = temp_bundle_dir("ok.palmier");
        std::fs::write(
            dir.join(core_model::TIMELINE_FILENAME),
            r#"{"fps":60,"width":1920,"height":1080}"#,
        )
        .unwrap();

        let hub = EditorStateHub::new();
        let before = hub.revision();
        hub.load_bundle(&dir).unwrap();

        assert!(hub.revision() > before);
        assert_eq!(hub.project_root(), Some(dir.clone()));
        let exec = hub.executor();
        assert_eq!(exec.lock().unwrap().timeline().fps, 60);
    }

    #[test]
    fn save_round_trips_mcp_edits() {
        let dir = temp_bundle_dir("save.palmier");
        std::fs::write(dir.join(core_model::TIMELINE_FILENAME), r#"{"fps":60}"#).unwrap();

        let hub = EditorStateHub::new();
        hub.load_bundle(&dir).unwrap();
        {
            let exec = hub.executor();
            exec.lock()
                .unwrap()
                .execute("organize_media", &serde_json::json!({"createFolders": ["B-roll"]}))
                .unwrap();
        }
        hub.save().unwrap();

        let fresh = EditorStateHub::new();
        fresh.load_bundle(&dir).unwrap();
        let exec = fresh.executor();
        let exec = exec.lock().unwrap();
        assert_eq!(exec.timeline().fps, 60);
        assert!(exec
            .media_manifest()
            .folders
            .iter()
            .any(|f| f.name == "B-roll"));
    }

    #[test]
    fn save_as_switches_root_and_save_targets_it() {
        let dir = temp_bundle_dir("save-as.palmier");
        let hub = EditorStateHub::new();
        assert!(hub.project_root().is_none());

        hub.save_as(&dir).unwrap();
        assert!(dir.join(core_model::TIMELINE_FILENAME).is_file());
        assert!(dir.join(core_model::MANIFEST_FILENAME).is_file());
        assert_eq!(hub.project_root(), Some(dir.clone()));

        let exec = hub.executor();
        exec.lock()
            .unwrap()
            .execute("organize_media", &serde_json::json!({"createFolders": ["B-roll"]}))
            .unwrap();
        hub.save().unwrap();

        let fresh = EditorStateHub::new();
        fresh.load_bundle(&dir).unwrap();
        let exec = fresh.executor();
        assert!(exec
            .lock()
            .unwrap()
            .media_manifest()
            .folders
            .iter()
            .any(|f| f.name == "B-roll"));
    }

    #[test]
    fn undo_restores_clip_position_after_move() {
        let hub = EditorStateHub::new();
        let timeline: Timeline = serde_json::from_str(
            r#"{"fps":30,"tracks":[{"id":"t1","type":"video","clips":[
                {"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":100}
            ]}]}"#,
        )
        .unwrap();
        hub.load_project(timeline, MediaManifest::default());

        let exec = hub.executor();
        let mut exec = exec.lock().unwrap();
        exec.execute(
            "move_clips",
            &serde_json::json!({"clipIds":["c1"],"toTrack":0,"toFrame":90}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips[0].start_frame, 90);

        exec.execute("undo", &serde_json::json!({})).unwrap();
        assert_eq!(
            exec.timeline().tracks[0].clips[0].start_frame,
            0,
            "undo restores the pre-move position"
        );
    }

    #[test]
    fn trim_end_shrinks_duration_and_undo_restores() {
        let hub = EditorStateHub::new();
        let timeline: Timeline = serde_json::from_str(
            r#"{"fps":30,"tracks":[{"id":"t1","type":"video","clips":[
                {"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":100}
            ]}]}"#,
        )
        .unwrap();
        hub.load_project(timeline, MediaManifest::default());

        let exec = hub.executor();
        let mut exec = exec.lock().unwrap();
        exec.execute(
            "set_clip_properties",
            &serde_json::json!({"clipIds":["c1"],"properties":{"durationFrames":60}}),
        )
        .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(
            clip.start_frame + clip.duration_frames,
            60,
            "clip ends at the trimmed frame"
        );

        exec.execute("undo", &serde_json::json!({})).unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(clip.duration_frames, 100, "undo restores the length");
    }

    #[test]
    fn ripple_delete_shifts_later_clips_left() {
        let hub = EditorStateHub::new();
        let timeline: Timeline = serde_json::from_str(
            r#"{"fps":30,"tracks":[{"id":"t1","type":"video","clips":[
                {"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":100},
                {"id":"c2","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":300,"durationFrames":50}
            ]}]}"#,
        )
        .unwrap();
        hub.load_project(timeline, MediaManifest::default());

        let exec = hub.executor();
        let mut exec = exec.lock().unwrap();
        exec.execute(
            "ripple_delete_ranges",
            &serde_json::json!({"trackIndex":0,"ranges":[{"start":0,"end":100}]}),
        )
        .unwrap();
        let clips = &exec.timeline().tracks[0].clips;
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].id, "c2");
        assert_eq!(
            clips[0].start_frame, 200,
            "later clip shifts 100 frames left"
        );
    }

    #[test]
    fn save_without_open_project_fails() {
        let hub = EditorStateHub::new();
        let err = hub.save().unwrap_err();
        assert!(err.contains("No project open"), "err={err}");
    }

    // ── #211 autosave ────────────────────────────────────────────────────

    #[test]
    fn autosave_should_fire_trigger_logic() {
        // Injected counter (no timers): fires only with a root AND an advance.
        assert!(!autosave_should_fire(false, 5, 0), "rootless never autosaves");
        assert!(!autosave_should_fire(false, 5, 5), "rootless, no advance");
        assert!(autosave_should_fire(true, 1, 0), "root + advance fires");
        assert!(!autosave_should_fire(true, 3, 3), "no advance coalesces");
        assert!(!autosave_should_fire(true, 2, 5), "stale tick after save skips");
        assert!(autosave_should_fire(true, 6, 5), "further edit re-arms");
    }

    #[test]
    fn autosave_skips_rootless_project() {
        let hub = EditorStateHub::new();
        // An unsaved project has no root — an edit must not autosave.
        hub.load_project(
            Timeline {
                fps: 30,
                ..Default::default()
            },
            MediaManifest::default(),
        );
        {
            let exec = hub.executor();
            exec.lock()
                .unwrap()
                .execute("organize_media", &serde_json::json!({"createFolders": ["B-roll"]}))
                .unwrap();
        }
        assert!(hub.project_root().is_none());
        assert_eq!(hub.autosave_if_dirty(), Ok(false), "rootless: no autosave");
    }

    #[test]
    fn autosave_fires_after_edit_and_coalesces() {
        let dir = temp_bundle_dir("autosave.palmier");
        std::fs::write(dir.join(core_model::TIMELINE_FILENAME), r#"{"fps":30}"#).unwrap();

        let hub = EditorStateHub::new();
        hub.load_bundle(&dir).unwrap();
        // Freshly loaded → nothing dirty yet.
        assert_eq!(hub.autosave_if_dirty(), Ok(false), "just-loaded is clean");

        {
            let exec = hub.executor();
            exec.lock()
                .unwrap()
                .execute("organize_media", &serde_json::json!({"createFolders": ["B-roll"]}))
                .unwrap();
        }
        // First tick after the edit saves; the next tick coalesces (no advance).
        assert_eq!(hub.autosave_if_dirty(), Ok(true), "edit triggers autosave");
        assert_eq!(hub.autosave_if_dirty(), Ok(false), "no further edit → skip");

        // The autosaved folder round-trips from disk.
        let fresh = EditorStateHub::new();
        fresh.load_bundle(&dir).unwrap();
        let exec = fresh.executor();
        assert!(exec
            .lock()
            .unwrap()
            .media_manifest()
            .folders
            .iter()
            .any(|f| f.name == "B-roll"));
    }

    #[test]
    fn save_now_persists_current_state() {
        let dir = temp_bundle_dir("save-now.palmier");
        std::fs::write(dir.join(core_model::TIMELINE_FILENAME), r#"{"fps":30}"#).unwrap();

        let hub = EditorStateHub::new();
        hub.load_bundle(&dir).unwrap();
        {
            let exec = hub.executor();
            exec.lock()
                .unwrap()
                .execute("organize_media", &serde_json::json!({"createFolders": ["C-roll"]}))
                .unwrap();
        }
        hub.save_now().unwrap();
        // save_now marks the revision clean, so a follow-up autosave coalesces.
        assert_eq!(hub.autosave_if_dirty(), Ok(false), "save_now clears dirty");

        let fresh = EditorStateHub::new();
        fresh.load_bundle(&dir).unwrap();
        assert!(fresh
            .executor()
            .lock()
            .unwrap()
            .media_manifest()
            .folders
            .iter()
            .any(|f| f.name == "C-roll"));
    }

    #[test]
    fn save_now_without_project_errors() {
        let hub = EditorStateHub::new();
        assert!(hub.save_now().unwrap_err().contains("No project open"));
    }

    #[test]
    fn load_bundle_failure_leaves_state_untouched() {
        let dir = temp_bundle_dir("missing.palmier");
        // No project.json inside.
        let hub = EditorStateHub::new();
        let before = hub.revision();
        let err = hub.load_bundle(&dir).unwrap_err();
        assert!(err.contains("project.json"), "err={err}");
        assert_eq!(hub.revision(), before);
        assert!(hub.project_root().is_none());
    }
}
