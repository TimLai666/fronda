//! Multi-session (multi-window) state model (Issue #137).
//!
//! Each session represents one open Fronda project window. Sessions are
//! independent — they share no mutable state. The platform shell creates a
//! new gpui window per session and routes OS-level window events to it.
//!
//! This module is pure state — no gpui or platform dependencies.

use serde::{Deserialize, Serialize};

/// Unique identifier for an open session (window).
///
/// Monotonically increasing; never reused within a process lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u32);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

/// State of a single project window session (Issue #137).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub id: SessionId,
    /// Path to the open `.palmier` project, or `None` for an untitled new session.
    pub project_path: Option<String>,
    /// Human-readable title shown in the window title bar.
    pub title: String,
    /// Whether this session has unsaved changes.
    pub is_dirty: bool,
    /// Whether this session's window is currently focused.
    pub is_focused: bool,
}

impl SessionState {
    pub fn new(id: SessionId, project_path: Option<String>) -> Self {
        let title = project_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();
        Self {
            id,
            project_path,
            title,
            is_dirty: false,
            is_focused: false,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    pub fn mark_saved(&mut self) {
        self.is_dirty = false;
    }
}

/// Registry of all open sessions in the current process (Issue #137).
#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: Vec<SessionState>,
    next_id: u32,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a new session, optionally for a project at `path`.
    pub fn open(&mut self, project_path: Option<String>) -> SessionId {
        let id = SessionId(self.next_id);
        self.next_id += 1;
        let mut session = SessionState::new(id, project_path);
        // First session starts focused
        if self.sessions.is_empty() {
            session.is_focused = true;
        }
        self.sessions.push(session);
        id
    }

    /// Close a session by ID. Returns `true` if found and removed.
    pub fn close(&mut self, id: SessionId) -> bool {
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions.remove(pos);
            // Refocus the last remaining session if any
            if let Some(last) = self.sessions.last_mut() {
                last.is_focused = true;
            }
            true
        } else {
            false
        }
    }

    pub fn get(&self, id: SessionId) -> Option<&SessionState> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut SessionState> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn focused_id(&self) -> Option<SessionId> {
        self.sessions.iter().find(|s| s.is_focused).map(|s| s.id)
    }

    pub fn set_focused(&mut self, id: SessionId) {
        for s in &mut self.sessions {
            s.is_focused = s.id == id;
        }
    }

    pub fn all(&self) -> &[SessionState] {
        &self.sessions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Issue #137: Multiple sessions / tabs

    #[test]
    fn issue_137_registry_starts_empty() {
        let reg = SessionRegistry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.is_empty());
    }

    #[test]
    fn issue_137_open_new_session_returns_unique_id() {
        let mut reg = SessionRegistry::new();
        let a = reg.open(None);
        let b = reg.open(None);
        assert_ne!(a, b);
        assert_eq!(reg.len(), 2);
    }

    #[test]
    fn issue_137_first_session_is_focused() {
        let mut reg = SessionRegistry::new();
        let id = reg.open(None);
        assert!(reg.get(id).unwrap().is_focused);
    }

    #[test]
    fn issue_137_second_session_not_focused_by_default() {
        let mut reg = SessionRegistry::new();
        reg.open(None);
        let b = reg.open(None);
        assert!(!reg.get(b).unwrap().is_focused);
    }

    #[test]
    fn issue_137_close_session_removes_it() {
        let mut reg = SessionRegistry::new();
        let id = reg.open(None);
        assert!(reg.close(id));
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn issue_137_close_nonexistent_returns_false() {
        let mut reg = SessionRegistry::new();
        assert!(!reg.close(SessionId(99)));
    }

    #[test]
    fn issue_137_close_focused_refocuses_last() {
        let mut reg = SessionRegistry::new();
        let a = reg.open(None);
        let b = reg.open(None);
        reg.set_focused(a);
        reg.close(a);
        // b should now be focused
        assert!(reg.get(b).unwrap().is_focused);
    }

    #[test]
    fn issue_137_set_focused_switches_focus() {
        let mut reg = SessionRegistry::new();
        let a = reg.open(None);
        let b = reg.open(None);
        reg.set_focused(b);
        assert!(!reg.get(a).unwrap().is_focused);
        assert!(reg.get(b).unwrap().is_focused);
        assert_eq!(reg.focused_id(), Some(b));
    }

    #[test]
    fn issue_137_session_title_derived_from_path() {
        let mut reg = SessionRegistry::new();
        let id = reg.open(Some("/projects/my-film.palmier".into()));
        let title = &reg.get(id).unwrap().title;
        assert_eq!(title, "my-film");
    }

    #[test]
    fn issue_137_untitled_session_default_title() {
        let mut reg = SessionRegistry::new();
        let id = reg.open(None);
        assert_eq!(reg.get(id).unwrap().title, "Untitled");
    }

    #[test]
    fn issue_137_mark_dirty_and_saved() {
        let mut reg = SessionRegistry::new();
        let id = reg.open(None);
        reg.get_mut(id).unwrap().mark_dirty();
        assert!(reg.get(id).unwrap().is_dirty);
        reg.get_mut(id).unwrap().mark_saved();
        assert!(!reg.get(id).unwrap().is_dirty);
    }

    #[test]
    fn issue_137_session_id_display() {
        let id = SessionId(3);
        assert_eq!(id.to_string(), "session-3");
    }
}
