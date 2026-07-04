use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_chat_title() -> String {
    "New chat".to_string()
}

fn default_chat_is_open() -> bool {
    true
}

fn new_id() -> Uuid {
    Uuid::new_v4()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMessageRole {
    User,
    Assistant,
    /// System-authored context (e.g. MCP project-navigation notices, upstream #238).
    /// Present so a chat session carrying a `system` message decodes instead of
    /// failing the whole session on an unknown role.
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ToolResultBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        base64: String,
        #[serde(rename = "mediaType")]
        media_type: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AgentContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "toolUse")]
    ToolUse {
        id: String,
        name: String,
        input: String,
    },
    #[serde(rename = "toolResult")]
    ToolResult {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        content: Vec<ToolResultBlock>,
        #[serde(rename = "isError")]
        is_error: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTimelineRangeMention {
    pub start_frame: i64,
    pub end_frame: i64,
    pub duration_frames: i64,
    pub fps: i64,
    pub start_timecode: String,
    pub end_timecode: String,
    pub duration_timecode: String,
    pub range_semantics: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMention {
    #[serde(default = "new_id")]
    pub id: Uuid,
    pub display_name: String,
    pub media_ref: Option<String>,
    #[serde(rename = "type")]
    pub r#type: Option<crate::timeline::ClipType>,
    pub clip_id: Option<String>,
    pub timeline_range: Option<AgentTimelineRangeMention>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    #[serde(default = "new_id")]
    pub id: Uuid,
    pub role: AgentMessageRole,
    pub blocks: Vec<AgentContentBlock>,
    pub mentions: Vec<AgentMention>,
    pub context_hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    #[serde(default = "new_id")]
    pub id: Uuid,
    #[serde(default = "default_chat_title")]
    pub title: String,
    #[serde(with = "crate::date_serde::iso8601_date")]
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<AgentMessage>,
    #[serde(default = "default_chat_is_open")]
    pub is_open: bool,
}
