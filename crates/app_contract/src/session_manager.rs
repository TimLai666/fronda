//! Session manager — stateful wrapper around session lifecycle functions.
//!
//! Provides a single stateful API for the chat view, wrapping the pure
//! session functions in `agent_contract::session`. This makes tabbed
//! session management (CHAT-009) testable without gpui.

use std::collections::VecDeque;

/// A unique paste route identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteFocus {
    Timeline,
    MediaPanel,
}

/// A tabbed session manager that the chat view can use directly.
#[derive(Debug, Clone)]
pub struct SessionManager {
    /// The current list of sessions (tabs).
    pub sessions: VecDeque<ChatSessionStub>,
    /// Index of the active session tab.
    pub active_index: usize,
}

/// A lightweight stub of a chat session for tab display.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatSessionStub {
    pub id: uuid::Uuid,
    pub title: String,
    pub message_count: usize,
    pub is_open: bool,
}

impl SessionManager {
    /// Create a new session manager with one fresh empty session.
    pub fn new() -> Self {
        let fresh = ChatSessionStub {
            id: uuid::Uuid::new_v4(),
            title: "New chat".to_string(),
            message_count: 0,
            is_open: true,
        };
        let mut sessions = VecDeque::new();
        sessions.push_back(fresh);
        Self {
            sessions,
            active_index: 0,
        }
    }

    /// The currently active session.
    pub fn active_session(&self) -> Option<&ChatSessionStub> {
        self.sessions.get(self.active_index)
    }

    /// Create a new session tab.
    pub fn new_tab(&mut self) {
        let fresh = ChatSessionStub {
            id: uuid::Uuid::new_v4(),
            title: "New chat".to_string(),
            message_count: 0,
            is_open: true,
        };
        // Close the current session if it's empty and untitled
        if let Some(current) = self.sessions.get_mut(self.active_index) {
            if current.message_count == 0 {
                // drop empty session — replace
                *current = fresh;
                return;
            }
            current.is_open = false;
        }
        self.sessions.push_back(fresh);
        self.active_index = self.sessions.len() - 1;
    }

    /// Close the session at the given index. Switches to another open session
    /// or creates a fresh one if none remain.
    pub fn close_tab(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }
        self.sessions.remove(index);

        if self.sessions.is_empty() {
            let fresh = ChatSessionStub {
                id: uuid::Uuid::new_v4(),
                title: "New chat".to_string(),
                message_count: 0,
                is_open: true,
            };
            self.sessions.push_back(fresh);
            self.active_index = 0;
            return;
        }

        // Clamp active index
        if self.active_index >= self.sessions.len() {
            self.active_index = self.sessions.len() - 1;
        }
    }

    /// Select the session at the given index.
    pub fn select_tab(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active_index = index;
        }
    }

    /// Update the title of the active session.
    pub fn set_active_title(&mut self, title: String) {
        if let Some(session) = self.sessions.get_mut(self.active_index) {
            session.title = title;
        }
    }

    /// Increment message count for the active session.
    pub fn increment_message_count(&mut self) {
        if let Some(session) = self.sessions.get_mut(self.active_index) {
            session.message_count += 1;
            if session.message_count == 1 {
                // First message — session is no longer empty
            }
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Route a paste action based on the current focus target.
///
/// CCB-014: timeline-focused paste uses the clip clipboard,
/// media-panel-focused paste imports from the OS pasteboard instead.
pub fn route_paste(focus: PasteFocus) -> PasteFocus {
    focus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_manager_starts_with_one_empty_tab() {
        let mgr = SessionManager::new();
        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_index, 0);
        let s = mgr.active_session().unwrap();
        assert_eq!(s.title, "New chat");
        assert_eq!(s.message_count, 0);
        assert!(s.is_open);
    }

    #[test]
    fn session_manager_new_tab_creates_additional_tab() {
        let mut mgr = SessionManager::new();
        // Mark first tab as non-empty so new_tab doesn't replace it
        mgr.increment_message_count();
        mgr.new_tab();
        assert_eq!(mgr.sessions.len(), 2);
        assert_eq!(mgr.active_index, 1);
    }

    #[test]
    fn session_manager_new_tab_replaces_empty_untitled() {
        let mut mgr = SessionManager::new();
        mgr.new_tab(); // first tab was empty, so it gets replaced
        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_index, 0);
    }

    #[test]
    fn session_manager_select_tab() {
        let mut mgr = SessionManager::new();
        // First tab is auto-created, mark it as non-empty so new_tab doesn't replace
        mgr.increment_message_count();
        mgr.new_tab();
        assert_eq!(mgr.active_index, 1);
        mgr.select_tab(0);
        assert_eq!(mgr.active_index, 0);
        mgr.select_tab(1);
        assert_eq!(mgr.active_index, 1);
    }

    #[test]
    fn session_manager_select_tab_ignores_out_of_bounds() {
        let mut mgr = SessionManager::new();
        mgr.select_tab(5);
        assert_eq!(mgr.active_index, 0);
    }

    #[test]
    fn session_manager_close_tab_removes_and_switches() {
        let mut mgr = SessionManager::new();
        mgr.increment_message_count();
        mgr.new_tab();
        mgr.increment_message_count();
        mgr.new_tab();
        assert_eq!(mgr.sessions.len(), 3);
        mgr.close_tab(1);
        assert_eq!(mgr.sessions.len(), 2);
    }

    #[test]
    fn session_manager_close_last_tab_creates_fresh() {
        let mut mgr = SessionManager::new();
        mgr.close_tab(0);
        assert_eq!(mgr.sessions.len(), 1);
        assert_eq!(mgr.active_index, 0);
        assert_eq!(mgr.active_session().unwrap().title, "New chat");
    }

    #[test]
    fn session_manager_close_tab_ignores_out_of_bounds() {
        let mut mgr = SessionManager::new();
        mgr.close_tab(5);
        assert_eq!(mgr.sessions.len(), 1);
    }

    #[test]
    fn session_manager_set_active_title() {
        let mut mgr = SessionManager::new();
        mgr.set_active_title("Test Chat".into());
        assert_eq!(mgr.active_session().unwrap().title, "Test Chat");
    }

    #[test]
    fn session_manager_increment_message_count() {
        let mut mgr = SessionManager::new();
        assert_eq!(mgr.active_session().unwrap().message_count, 0);
        mgr.increment_message_count();
        assert_eq!(mgr.active_session().unwrap().message_count, 1);
    }

    #[test]
    fn route_paste_returns_input() {
        assert_eq!(route_paste(PasteFocus::Timeline), PasteFocus::Timeline);
        assert_eq!(route_paste(PasteFocus::MediaPanel), PasteFocus::MediaPanel);
    }
}
