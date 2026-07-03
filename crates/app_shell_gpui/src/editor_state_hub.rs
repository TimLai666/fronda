//! Single shared editor state for the shell (Swift: EditorViewModel access).
//!
//! Owns the one `ToolExecutor` both the UI and the MCP server operate on.
//! Pure std + workspace crates — no gpui.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use agent_contract::ToolExecutor;
use core_model::{MediaManifest, Timeline};
use project_io::ProjectBundle;

/// Process-wide holder of the current project state.
pub struct EditorStateHub {
    executor: Arc<Mutex<ToolExecutor>>,
    project_root: Mutex<Option<PathBuf>>,
}

impl EditorStateHub {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(Mutex::new(ToolExecutor::new(
                Timeline::default(),
                MediaManifest::default(),
            ))),
            project_root: Mutex::new(None),
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
        }
        if let Ok(mut root) = self.project_root.lock() {
            *root = Some(bundle.root);
        }
        Ok(())
    }

    /// Root directory of the currently loaded project, if any.
    pub fn project_root(&self) -> Option<PathBuf> {
        self.project_root.lock().ok().and_then(|r| r.clone())
    }

    /// Snapshot the shared timeline and manifest under the lock.
    fn snapshot(&self) -> Result<(Timeline, MediaManifest), String> {
        let exec = self
            .executor
            .lock()
            .map_err(|_| "Editor state lock poisoned".to_string())?;
        Ok((exec.timeline().clone(), exec.media_manifest().clone()))
    }

    /// Write the shared timeline and manifest back to the open project.
    /// Clones the state under the lock, writes to disk outside it.
    pub fn save(&self) -> Result<(), String> {
        let Some(root) = self.project_root() else {
            return Err("No project open: nothing to save".into());
        };
        let (timeline, manifest) = self.snapshot()?;
        project_io::save_project_state(&root, &timeline, &manifest).map_err(|e| e.to_string())
    }

    /// Write the current state to a new directory and make it the
    /// project root. On write failure the root is left unchanged.
    pub fn save_as(&self, root: &Path) -> Result<(), String> {
        let (timeline, manifest) = self.snapshot()?;
        project_io::save_project_state(root, &timeline, &manifest).map_err(|e| e.to_string())?;
        if let Ok(mut current) = self.project_root.lock() {
            *current = Some(root.to_path_buf());
        }
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
            exec.execute("create_folder", &serde_json::json!({"name": "B-roll"}))
                .unwrap();
        }
        let before = hub.revision();
        assert_eq!(before, 1);

        let timeline = Timeline {
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
                .execute("create_folder", &serde_json::json!({"name": "B-roll"}))
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
            .execute("create_folder", &serde_json::json!({"name": "B-roll"}))
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
    fn save_without_open_project_fails() {
        let hub = EditorStateHub::new();
        let err = hub.save().unwrap_err();
        assert!(err.contains("No project open"), "err={err}");
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
