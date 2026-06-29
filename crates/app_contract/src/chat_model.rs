//! Chat panel model — pure logic for the Agent/Chat panel state.
//!
//! Covers CHAT-001 through CHAT-010.

use serde::{Deserialize, Serialize};

/// Status of a tool call within an assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum ToolCallStatus {
    #[default]
    Running,
    Done,
    Failed,
}

/// A single tool invocation within an assistant message (Swift: AgentMessage block .toolUse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub status: ToolCallStatus,
    /// Pretty-printed JSON of the tool's input arguments (Swift: inputJSON).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_json: Option<String>,
    /// Output text returned by the tool (Swift: resultText).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_text: Option<String>,
}

/// A single chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: MessageStatus,
    /// Tool calls emitted by this assistant message (empty for user messages).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
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
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatInput {
    pub text: String,
    pub cursor_position: usize,
    /// CHAT-008: Asset IDs of media mentioned via drop/paste (for submission context).
    pub pending_mentions: Vec<String>,
}

impl ChatInput {
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor_position = 0;
        self.pending_mentions.clear();
    }
}

/// Full chat panel state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChatPanelModel {
    pub messages: Vec<ChatMessage>,
    pub input: ChatInput,
    pub is_agent_running: bool,
    pub show_mention_picker: bool,
    pub mention_query: String,
}

impl ChatPanelModel {
    /// Add a message to the chat log.
    pub fn add_message(&mut self, role: ChatRole, text: String) {
        self.messages.push(ChatMessage {
            role,
            text,
            timestamp: chrono::Utc::now(),
            status: MessageStatus::Sent,
            tool_calls: Vec::new(),
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

    /// Whether the send button should be enabled.
    ///
    /// CHAT-001: Send action is enabled when input is non-empty and agent is not running.
    pub fn can_send(&self) -> bool {
        !self.input.is_empty() && !self.is_agent_running
    }

    /// Stop the current agent generation.
    ///
    /// CHAT-002: Streaming stop action — sets agent to idle.
    pub fn stop_generation(&mut self) {
        self.is_agent_running = false;
    }

    /// Handle send action — returns the message text if sent, or None if blocked.
    ///
    /// CHAT-003: Enter sends the message; Shift+Enter inserts a newline.
    /// When `shift_held` is true, inserts a newline into the input instead of sending.
    pub fn handle_send_action(&mut self, shift_held: bool) -> Option<String> {
        if shift_held {
            self.input.text.push('\n');
            self.input.cursor_position = self.input.text.len();
            return None;
        }
        self.send_message()
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

    /// CHAT-008: Insert a media mention into the chat input when a media asset
    /// is dropped or pasted into the chat panel.
    ///
    /// Appends `@<asset_label>` at the current cursor position (with a leading
    /// space if the input is non-empty and doesn't already end in whitespace).
    /// The gpui-ce OnDrop handler calls this after resolving the dropped asset id.
    pub fn paste_media_mention(&mut self, asset_id: &str, label: &str) {
        let tag = format!("@{label}");
        if !self.input.text.is_empty()
            && !self.input.text.ends_with(|c: char| c.is_whitespace())
        {
            self.input.text.push(' ');
        }
        self.input.text.push_str(&tag);
        self.input.cursor_position = self.input.text.len();
        // Record which asset was mentioned (for submission context)
        self.input.pending_mentions.push(asset_id.to_string());
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
    fn chat_can_send_empty_input_returns_false() {
        let model = ChatPanelModel::default();
        assert!(!model.can_send());
    }

    #[test]
    fn chat_can_send_with_input_returns_true() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Hello".into();
        assert!(model.can_send());
    }

    #[test]
    fn chat_can_send_while_agent_running_returns_false() {
        let mut model = ChatPanelModel {
            is_agent_running: true,
            ..Default::default()
        };
        model.input.text = "Hello".into();
        assert!(!model.can_send());
    }

    #[test]
    fn chat_stop_generation_sets_idle() {
        let mut model = ChatPanelModel {
            is_agent_running: true,
            ..Default::default()
        };
        model.stop_generation();
        assert!(!model.is_agent_running);
    }

    #[test]
    fn chat_handle_send_enter_sends_message() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Hello".into();
        let result = model.handle_send_action(false);
        assert_eq!(result, Some("Hello".into()));
        assert_eq!(model.messages.len(), 1);
    }

    #[test]
    fn chat_handle_send_shift_enter_inserts_newline() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Hello".into();
        let result = model.handle_send_action(true);
        assert_eq!(result, None);
        assert!(model.input.text.contains('\n'));
        assert_eq!(model.input.cursor_position, model.input.text.len());
    }

    #[test]
    fn chat_input_clear() {
        let mut input = ChatInput {
            text: "test".into(),
            cursor_position: 4,
            ..Default::default()
        };
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
            tool_calls: Vec::new(),
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
            tool_calls: Vec::new(),
        };
        match &msg.status {
            MessageStatus::Failed(reason) => assert_eq!(reason, "timeout"),
            _ => panic!("expected Failed"),
        }
    }

    // ── CHAT-008: Drop/paste media mention ────────────────────────────────────

    #[test]
    fn chat_008_paste_media_mention_empty_input() {
        let mut model = ChatPanelModel::default();
        model.paste_media_mention("asset-123", "beach.mp4");
        assert_eq!(model.input.text, "@beach.mp4");
        assert_eq!(model.input.pending_mentions, vec!["asset-123".to_string()]);
    }

    #[test]
    fn chat_008_paste_media_mention_appends_space_if_needed() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Tell me about".into();
        model.paste_media_mention("a1", "clip.mp4");
        assert_eq!(model.input.text, "Tell me about @clip.mp4");
    }

    #[test]
    fn chat_008_paste_media_mention_no_extra_space_if_already_spaced() {
        let mut model = ChatPanelModel::default();
        model.input.text = "Tell me about ".into();
        model.paste_media_mention("a1", "clip.mp4");
        assert_eq!(model.input.text, "Tell me about @clip.mp4");
    }

    #[test]
    fn chat_008_paste_multiple_mentions() {
        let mut model = ChatPanelModel::default();
        model.paste_media_mention("a1", "clip1.mp4");
        model.paste_media_mention("a2", "clip2.mp4");
        assert!(model.input.text.contains("@clip1.mp4"));
        assert!(model.input.text.contains("@clip2.mp4"));
        assert_eq!(model.input.pending_mentions.len(), 2);
    }

    #[test]
    fn chat_008_clear_also_clears_pending_mentions() {
        let mut model = ChatPanelModel::default();
        model.paste_media_mention("a1", "clip.mp4");
        model.input.clear();
        assert!(model.input.pending_mentions.is_empty());
        assert!(model.input.text.is_empty());
    }
}
