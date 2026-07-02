//! Single shared editor state for the shell (Swift: EditorViewModel access).
//!
//! Owns the one `ToolExecutor` both the UI and the MCP server operate on.
//! Pure std + workspace crates — no gpui.

use std::sync::{Arc, Mutex, OnceLock};

use agent_contract::ToolExecutor;
use core_model::{MediaManifest, Timeline};

/// Process-wide holder of the current project state.
pub struct EditorStateHub {
    executor: Arc<Mutex<ToolExecutor>>,
}

impl EditorStateHub {
    pub fn new() -> Self {
        Self {
            executor: Arc::new(Mutex::new(ToolExecutor::new(
                Timeline::default(),
                MediaManifest::default(),
            ))),
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
}
