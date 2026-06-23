//! Chat session lifecycle management (SES-001 to SES-016).
//!
//! Pure functions for session CRUD, selection, ordering, and title derivation.
//! Persistence (load/save from disk) lives in project_io.

use core_model::{AgentMessage, AgentMessageRole, ChatSession};
use uuid::Uuid;

/// SES-016: Derive a chat title from the first user message text, truncated
/// to 40 characters.
pub fn derive_title(messages: &[AgentMessage]) -> String {
    for msg in messages {
        if msg.role == AgentMessageRole::User {
            let text = msg
                .blocks
                .iter()
                .filter_map(|block| {
                    if let core_model::AgentContentBlock::Text { ref text } = block {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<&str>>()
                .join(" ");
            if !text.is_empty() {
                if text.len() <= 40 {
                    return text.to_string();
                }
                return text[..40].to_string();
            }
        }
    }
    "New chat".to_string()
}

/// SES-012: Sync the current session (save pending state) then create a
/// fresh empty session.
///
/// If `current` has no messages and is a "New chat", it should be dropped
/// rather than persisted.
pub fn new_chat(current: Option<ChatSession>) -> (Option<ChatSession>, ChatSession) {
    // The caller is responsible for persisting the synced current session.
    // If the current session is empty and untitled, drop it.
    let synced = current.filter(|s| !should_drop_empty_session(s));
    let fresh = ChatSession {
        id: Uuid::new_v4(),
        title: "New chat".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
        is_open: true,
    };
    (synced, fresh)
}

/// Returns true if a session is empty and has the default "New chat" title.
fn should_drop_empty_session(session: &ChatSession) -> bool {
    session.messages.is_empty() && session.title == "New chat"
}

/// SES-009: Sort sessions descending by `updated_at`.
pub fn sort_sessions(sessions: &mut [ChatSession]) {
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
}

/// SES-010: Force-close all loaded sessions and insert a fresh empty
/// "New chat" at the front.
pub fn prepare_loaded_sessions(sessions: &mut Vec<ChatSession>) {
    for session in sessions.iter_mut() {
        session.is_open = false;
    }
    let fresh = ChatSession {
        id: Uuid::new_v4(),
        title: "New chat".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
        is_open: true,
    };
    sessions.insert(0, fresh);
}

/// SES-013: Before selecting a new session, sync the current one. Returns
/// the (synced_current, newly_selected) pair.
pub fn select_session(
    current: Option<ChatSession>,
    target_id: Uuid,
    sessions: &[ChatSession],
) -> (Option<ChatSession>, Option<ChatSession>) {
    let synced = current.filter(|s| !should_drop_empty_session(s));
    let selected = sessions.iter().find(|s| s.id == target_id).cloned();
    (synced, selected)
}

/// SES-014: Close the current tab. Switch to another open session, or
/// create a fresh empty one if none remain.
pub fn close_tab(current_id: Uuid, sessions: &[ChatSession]) -> (Vec<ChatSession>, ChatSession) {
    let mut remaining: Vec<ChatSession> = sessions
        .iter()
        .filter(|s| s.id != current_id)
        .cloned()
        .collect();

    // Find another open session
    let open_idx = remaining.iter().position(|s| s.is_open);
    if let Some(idx) = open_idx {
        let next = remaining[idx].clone();
        (remaining, next)
    } else {
        let fresh = ChatSession {
            id: Uuid::new_v4(),
            title: "New chat".to_string(),
            updated_at: chrono::Utc::now(),
            messages: vec![],
            is_open: true,
        };
        remaining.push(fresh.clone());
        (remaining, fresh)
    }
}

/// SES-015: Delete the current session. Switch to another open session, or
/// create a fresh empty one if none remain.
pub fn delete_session(
    target_id: Uuid,
    sessions: Vec<ChatSession>,
) -> (Vec<ChatSession>, ChatSession) {
    close_tab(target_id, &sessions)
}

/// SES-008: When loading old session JSON without `isOpen`, the default
/// remains `true`. This is handled by the serde default on ChatSession.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::AgentContentBlock;

    fn text_block(text: &str) -> AgentContentBlock {
        AgentContentBlock::Text {
            text: text.to_string(),
        }
    }

    fn user_message(text: &str) -> AgentMessage {
        AgentMessage {
            id: Uuid::new_v4(),
            role: AgentMessageRole::User,
            blocks: vec![text_block(text)],
            mentions: vec![],
            context_hint: None,
        }
    }

    fn assistant_message(text: &str) -> AgentMessage {
        AgentMessage {
            id: Uuid::new_v4(),
            role: AgentMessageRole::Assistant,
            blocks: vec![text_block(text)],
            mentions: vec![],
            context_hint: None,
        }
    }

    fn make_session(id: Uuid, title: &str, is_open: bool) -> ChatSession {
        ChatSession {
            id,
            title: title.to_string(),
            updated_at: chrono::Utc::now(),
            messages: vec![],
            is_open,
        }
    }

    #[test]
    fn ses_001_009_sessions_sort_descending_by_updated_at() {
        let older = ChatSession {
            id: Uuid::new_v4(),
            title: "older".into(),
            updated_at: chrono::Utc::now() - chrono::Duration::hours(2),
            messages: vec![],
            is_open: false,
        };
        let newer = ChatSession {
            id: Uuid::new_v4(),
            title: "newer".into(),
            updated_at: chrono::Utc::now(),
            messages: vec![],
            is_open: false,
        };
        let mut sessions = vec![older.clone(), newer.clone()];
        sort_sessions(&mut sessions);
        assert_eq!(sessions[0].id, newer.id, "SES-009: newest first");
        assert_eq!(sessions[1].id, older.id);
    }

    #[test]
    fn ses_010_loaded_sessions_forced_closed_fresh_at_front() {
        let mut sessions = vec![make_session(Uuid::new_v4(), "Existing", true)];
        prepare_loaded_sessions(&mut sessions);
        // Original session is now closed
        assert!(!sessions[1].is_open, "SES-010: forced closed");
        // Fresh session is at front
        assert_eq!(sessions[0].title, "New chat", "SES-010: fresh at front");
        assert!(sessions[0].is_open, "SES-010: fresh is open");
    }

    #[test]
    fn ses_012_new_chat_drops_empty() {
        let empty = ChatSession {
            id: Uuid::new_v4(),
            title: "New chat".into(),
            updated_at: chrono::Utc::now(),
            messages: vec![],
            is_open: true,
        };
        let (synced, fresh) = new_chat(Some(empty));
        assert!(synced.is_none(), "SES-012: empty new chat dropped");
        assert_eq!(fresh.title, "New chat");
    }

    #[test]
    fn ses_012_new_chat_preserves_non_empty() {
        let session = ChatSession {
            id: Uuid::new_v4(),
            title: "My Chat".into(),
            updated_at: chrono::Utc::now(),
            messages: vec![user_message("hello")],
            is_open: true,
        };
        let (synced, fresh) = new_chat(Some(session.clone()));
        assert!(synced.is_some(), "SES-012: non-empty synced");
        assert_eq!(synced.unwrap().id, session.id);
        assert_ne!(fresh.id, session.id);
    }

    #[test]
    fn ses_013_select_session_syncs_current() {
        let current = ChatSession {
            id: Uuid::new_v4(),
            title: "Current".into(),
            updated_at: chrono::Utc::now(),
            messages: vec![user_message("hello")],
            is_open: true,
        };
        let target = ChatSession {
            id: Uuid::new_v4(),
            title: "Target".into(),
            updated_at: chrono::Utc::now(),
            messages: vec![],
            is_open: false,
        };
        let (synced, selected) =
            select_session(Some(current.clone()), target.id, &[current, target.clone()]);
        assert!(synced.is_some(), "SES-013: current synced");
        assert_eq!(selected.unwrap().id, target.id);
    }

    #[test]
    fn ses_014_close_tab_switches_to_open_session() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let sessions = vec![
            make_session(id1, "Tab 1", true),
            make_session(id2, "Tab 2", true),
        ];
        let (remaining, next) = close_tab(id1, &sessions);
        assert_eq!(remaining.len(), 1, "SES-014: one tab removed");
        assert_eq!(next.id, id2, "SES-014: switched to other open tab");
    }

    #[test]
    fn ses_014_close_tab_creates_fresh_when_none_open() {
        let id = Uuid::new_v4();
        let sessions = vec![make_session(id, "Last Tab", true)];
        let (remaining, next) = close_tab(id, &sessions);
        // remaining contains the fresh session
        assert_eq!(
            remaining.len(),
            1,
            "SES-014: one fresh session in remaining"
        );
        assert_eq!(
            remaining[0].title, "New chat",
            "SES-014: fresh session added"
        );
        assert_eq!(next.title, "New chat", "SES-014: fresh created");
        assert!(next.is_open);
    }

    #[test]
    fn ses_015_delete_same_as_close_tab() {
        let id = Uuid::new_v4();
        let sessions = vec![make_session(id, "To Delete", true)];
        let (remaining, next) = delete_session(id, sessions);
        // remaining contains the fresh session
        assert_eq!(remaining.len(), 1, "SES-015: fresh session in remaining");
        assert_eq!(next.title, "New chat");
    }

    #[test]
    fn ses_016_title_derived_from_first_user_message() {
        let msgs = vec![
            assistant_message("I'll help you edit"),
            user_message("Can you add a title card?"),
            user_message("Make it blue"),
        ];
        let title = derive_title(&msgs);
        assert_eq!(
            title, "Can you add a title card?",
            "SES-016: first user message"
        );
    }

    #[test]
    fn ses_016_title_truncated_to_40_chars() {
        let long = "a".repeat(100);
        let msgs = vec![user_message(&long)];
        let title = derive_title(&msgs);
        assert_eq!(title.len(), 40, "SES-016: truncated to 40");
    }

    #[test]
    fn ses_016_title_new_chat_when_no_user_messages() {
        let msgs = vec![assistant_message("Hello")];
        let title = derive_title(&msgs);
        assert_eq!(title, "New chat", "SES-016: no user message");
    }

    #[test]
    fn ses_016_title_empty_text_in_user_message_returns_new_chat() {
        let msgs = vec![user_message("")];
        let title = derive_title(&msgs);
        assert_eq!(title, "New chat");
    }
}
