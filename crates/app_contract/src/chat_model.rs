//! Chat panel model — pure logic for the Agent/Chat panel state.
//!
//! Covers CHAT-001 through CHAT-010.

use serde::{Deserialize, Serialize};

/// A single chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: MessageStatus,
}

/// Chat participant role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

/// Delivery status of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Failed(String),
}

/// Chat input state.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatInput {
    pub text: String,
    pub cursor_position: usize,
}

impl Default for ChatInput {
    fn default() -> Self {
        Self {
            text: String::new(),
            cursor_position: 0,
        }
    }
}

impl ChatInput {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
    }
}

/// Full chat panel state.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatPanelModel {
    pub messages: Vec<ChatMessage>,
    pub input: ChatInput,
    pub is_agent_running: bool,
    pub show_mention_picker: bool,
    pub mention_query: String,
}

impl Default for ChatPanelModel {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            input: ChatInput::default(),
            is_agent_running: false,
            show_mention_picker: false,
            mention_query: String::new(),
        }
    }
}

impl ChatPanelModel {
    /// Add a message to the chat log.
    pub fn add_message(&mut self, role: ChatRole, text: String) {
        self.messages.push(ChatMessage {
            role,
            text,
            timestamp: chrono::Utc::now(),
            status: MessageStatus::Sent,
        });
    }

    /// Send user message — clears input and appends user + placeholder assistant.
    pub fn send_message(&mut self) -> Option<String> {
        let text = self.input.text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.add_message(ChatRole::User, text.clone());
        self.input.clear();
        self.is_agent_running = true;
        Some(text)
    }

    /// Mark agent as done/idle.
    pub fn agent_finished(&mut self) {
        self.is_agent_running = false;
    }

    /// Toggle mention picker.
    pub fn toggle_mention_picker(&mut self) {
        self.show_mention_picker = !self.show_mention_picker;
        if !self.show_mention_picker {
            self.mention_query.clear();
        }
    }

    /// Update mention filter query.
    pub fn set_mention_query(&mut self, query: String) {
        self.mention_query = query;
        self.show_mention_picker = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_default_state() {
        let model = ChatPanelModel::default();
        assert!(model.messages.is_empty());
        assert!(model.input.is_empty());
        assert!(!model.is_agent_running);
        assert!(!model.show_mention_picker);
    }

    #[test]
    fn chat_send_message() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Hello, agent!".into();
        let text = model.send_message();
        assert_eq!(text, Some("Hello, agent!".into()));
        assert_eq!(model.messages.len(), 1);
        assert!(model.input.is_empty());
        assert!(model.is_agent_running);
    }

    #[test]
    fn chat_send_empty_does_nothing() {
        let mut model = ChatPanelModel::default();
        assert!(model.send_message().is_none());
        assert!(model.messages.is_empty());
    }

    #[test]
    fn chat_agent_finished() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Test".into();
        model.send_message();
        assert!(model.is_agent_running);
        model.agent_finished();
        assert!(!model.is_agent_running);
    }

    #[test]
    fn chat_toggle_mention_picker() {
        let mut model = ChatPanelModel::default();
        assert!(!model.show_mention_picker);
        model.toggle_mention_picker();
        assert!(model.show_mention_picker);
        model.toggle_mention_picker();
        assert!(!model.show_mention_picker);
    }

    #[test]
    fn chat_mention_query() {
        let mut model = ChatPanelModel::default();
        model.set_mention_query("@med".into());
        assert!(model.show_mention_picker);
        assert_eq!(model.mention_query, "@med");
    }

    #[test]
    fn chat_input_clear() {
        let mut input = ChatInput::default();
        input.text = "test".into();
        input.cursor_position = 4;
        input.clear();
        assert!(input.text.is_empty());
        assert_eq!(input.cursor_position, 0);
    }

    #[test]
    fn chat_message_roles() {
        let msg = ChatMessage {
            role: ChatRole::User,
            text: "Hello".into(),
            timestamp: chrono::Utc::now(),
            status: MessageStatus::Sent,
        };
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.text, "Hello");
        assert_eq!(msg.status, MessageStatus::Sent);
    }

    #[test]
    fn chat_message_status_failed() {
        let msg = ChatMessage {
            role: ChatRole::Assistant,
            text: "Error".into(),
            timestamp: chrono::Utc::now(),
            status: MessageStatus::Failed("timeout".into()),
        };
        match &msg.status {
            MessageStatus::Failed(reason) => assert_eq!(reason, "timeout"),
            _ => panic!("expected Failed"),
        }
    }
}
