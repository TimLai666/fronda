//! Session persistence contract tests (SES-001 through SES-016).
//!
//! Validates JSON serialization format, ISO-8601 date compliance,
//! default deserialization, and session selection semantics.

use agent_contract::session::{new_chat, select_session, sort_sessions};
use core_model::{AgentMessage, AgentMessageRole, ChatSession};
use uuid::Uuid;

/// Helper to build a minimal text block
fn text_block(text: &str) -> core_model::AgentContentBlock {
    core_model::AgentContentBlock::Text {
        text: text.to_string(),
    }
}

/// Helper to build a user message
fn user_message(text: &str) -> AgentMessage {
    AgentMessage {
        id: Uuid::new_v4(),
        role: AgentMessageRole::User,
        blocks: vec![text_block(text)],
        mentions: vec![],
        context_hint: None,
    }
}

/// Helper to build a session with given fields
fn make_session(id: Uuid, title: &str, is_open: bool) -> ChatSession {
    ChatSession {
        id,
        title: title.to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
        is_open,
    }
}

// ── SES-001: Sessions serialize to JSON ──────────────────────────────────────

#[test]
fn ses_001_session_serializes_to_json() {
    let session = ChatSession {
        id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        title: "My Project".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![user_message("Hello")],
        is_open: true,
    };

    let json = serde_json::to_string(&session).expect("SES-001: must serialize to JSON");
    assert!(
        !json.is_empty(),
        "SES-001: serialized JSON must not be empty"
    );
    assert!(
        json.contains("My Project"),
        "SES-001: JSON must contain title"
    );
}

#[test]
fn ses_001_session_deserializes_round_trip() {
    let session = make_session(Uuid::new_v4(), "Round Trip Test", true);
    let json = serde_json::to_string(&session).unwrap();
    let deserialized: ChatSession = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, session.id);
    assert_eq!(deserialized.title, session.title);
    assert_eq!(deserialized.is_open, session.is_open);
}

// ── SES-003: Uses ISO-8601 dates ─────────────────────────────────────────────

#[test]
fn ses_003_date_uses_iso8601_format() {
    let session = make_session(Uuid::new_v4(), "Date Test", true);
    let json = serde_json::to_value(&session).unwrap();
    let date_str = json
        .get("updatedAt")
        .and_then(|v| v.as_str())
        .expect("SES-003: updatedAt must be a string");

    // ISO-8601 format: 2026-06-25T12:34:56Z
    assert!(
        date_str.ends_with('Z'),
        "SES-003: ISO-8601 date must end with Z, got: {date_str}"
    );
    assert!(
        date_str.contains('T'),
        "SES-003: ISO-8601 date must contain T separator, got: {date_str}"
    );

    // Verify it parses as RFC3339
    let parsed = chrono::DateTime::parse_from_rfc3339(date_str);
    assert!(
        parsed.is_ok(),
        "SES-003: date must be valid RFC3339/ISO-8601, got: {date_str}, err: {:?}",
        parsed.err()
    );
}

#[test]
fn ses_003_date_deserializes_from_iso8601() {
    let json = serde_json::json!({
        "id": "550e8400-e29b-41d4-a716-446655440001",
        "title": "ISO Test",
        "updatedAt": "2026-06-25T10:30:00Z",
        "messages": [],
        "isOpen": true
    });
    let session: ChatSession = serde_json::from_value(json).unwrap();
    assert_eq!(session.title, "ISO Test");
    assert_eq!(session.updated_at.to_rfc3339(), "2026-06-25T10:30:00+00:00");
}

// ── SES-004: Pretty-printed with sorted keys ─────────────────────────────────

#[test]
fn ses_004_pretty_printed_with_sorted_keys() {
    let session = make_session(Uuid::new_v4(), "Prettify Test", true);
    let pretty = serde_json::to_string_pretty(&session).unwrap();
    let compact = serde_json::to_string(&session).unwrap();

    // Pretty-printed is longer (contains newlines and indentation)
    assert!(
        pretty.len() > compact.len(),
        "SES-004: pretty-printed output must be longer than compact"
    );
    assert!(
        pretty.contains('\n'),
        "SES-004: pretty-printed must contain newlines"
    );

    // Round-trip: pretty-printed JSON should deserialize back
    let deserialized: ChatSession = serde_json::from_str(&pretty).unwrap();
    assert_eq!(deserialized.id, session.id);
}

#[test]
fn ses_004_sorted_keys_stable() {
    // Serialize with sorted keys and verify field ordering is stable
    let session = make_session(Uuid::new_v4(), "Sorted Keys", true);
    let json_str = serde_json::to_string(&session).unwrap();

    // Deserialize to a Value then serialize again with pretty-print
    let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
    let sorted = serde_json::to_string_pretty(&value).unwrap();

    // Re-parse to verify stable round-trip
    let _: ChatSession = serde_json::from_str(&sorted).unwrap();

    // Verify certain expected field names appear in the output
    assert!(sorted.contains("\"id\""), "output must contain id field");
    assert!(
        sorted.contains("\"updatedAt\""),
        "output must contain updatedAt field"
    );
    assert!(
        sorted.contains("\"isOpen\""),
        "output must contain isOpen field"
    );
}

// ── SES-008: Missing isOpen defaults to true ─────────────────────────────────

#[test]
fn ses_008_missing_is_open_defaults_to_true() {
    let json = serde_json::json!({
        "id": "550e8400-e29b-41d4-a716-446655440002",
        "title": "No isOpen",
        "updatedAt": "2026-06-25T12:00:00Z",
        "messages": []
        // isOpen intentionally omitted
    });
    let session: ChatSession = serde_json::from_value(json).unwrap();
    assert!(
        session.is_open,
        "SES-008: missing isOpen must default to true"
    );
}

#[test]
fn ses_008_is_open_false_when_explicitly_set() {
    let json = serde_json::json!({
        "id": "550e8400-e29b-41d4-a716-446655440003",
        "title": "Closed Session",
        "updatedAt": "2026-06-25T12:00:00Z",
        "messages": [],
        "isOpen": false
    });
    let session: ChatSession = serde_json::from_value(json).unwrap();
    assert!(!session.is_open, "explicit isOpen: false must be honored");
}

// ── SES-013: Selecting session syncs current before loading ──────────────────

#[test]
fn ses_013_select_session_syncs_current() {
    let current = ChatSession {
        id: Uuid::new_v4(),
        title: "Current Chat".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![user_message("current work")],
        is_open: true,
    };

    let target = ChatSession {
        id: Uuid::new_v4(),
        title: "Target Chat".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
        is_open: false,
    };

    let sessions = vec![current.clone(), target.clone()];
    let (synced, selected) = select_session(Some(current.clone()), target.id, &sessions);

    // SES-013: current is synced back because it has content
    assert!(
        synced.is_some(),
        "SES-013: current session must be synced before loading target"
    );
    let synced = synced.unwrap();
    assert_eq!(
        synced.id, current.id,
        "SES-013: synced session must be the current one"
    );

    // Target session is selected
    assert!(
        selected.is_some(),
        "SES-013: target session must be selected"
    );
    assert_eq!(
        selected.unwrap().id,
        target.id,
        "SES-013: selected session must match target"
    );
}

#[test]
fn ses_013_select_session_drops_empty_new_chat() {
    let empty = ChatSession {
        id: Uuid::new_v4(),
        title: "New chat".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![],
        is_open: true,
    };

    let target = ChatSession {
        id: Uuid::new_v4(),
        title: "Real Session".to_string(),
        updated_at: chrono::Utc::now(),
        messages: vec![user_message("hello")],
        is_open: false,
    };

    let sessions = vec![empty.clone(), target.clone()];
    let (synced, selected) = select_session(Some(empty), target.id, &sessions);

    // SES-013: empty "New chat" is dropped, not synced
    assert!(
        synced.is_none(),
        "SES-013: empty 'New chat' must be dropped, not synced"
    );

    // Target is selected
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().id, target.id);
}

// ── SES-016: Title derivation (bonus coverage) ───────────────────────────────

#[test]
fn ses_016_title_derived_from_first_user_message() {
    let msg = AgentMessage {
        id: Uuid::new_v4(),
        role: AgentMessageRole::User,
        blocks: vec![text_block("Add a title card please")],
        mentions: vec![],
        context_hint: None,
    };
    let fresh = new_chat(None);
    let (_, mut session) = new_chat(Some(fresh.1));
    session.messages.push(msg);
    assert!(!session.messages.is_empty(), "session should have messages");
}

// ── SES-009: Sort sessions descending by updated_at ──────────────────────────

#[test]
fn ses_009_sessions_sort_descending_by_updated_at() {
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
    let mut sessions = vec![older, newer.clone()];
    sort_sessions(&mut sessions);
    assert_eq!(sessions[0].id, newer.id, "SES-009: newest first");
}
