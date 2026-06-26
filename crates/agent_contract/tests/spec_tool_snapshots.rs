//! Snapshot-style contract tests for agent tool definitions.
//!
//! Validates that the tool registry, system instruction, and error handling
//! meet the documented contract. These tests access the public API only.

use agent_contract::{all_tools, ToolExecutor, SYSTEM_INSTRUCTION};
use core_model::{MediaManifest, Timeline, ToolResultBlock};

// ── TDEF-001: Exactly the right number of tools ──────────────────────────────

#[test]
fn tdef_001_exactly_53_tools() {
    let tools = all_tools();
    assert_eq!(
        tools.len(),
        53,
        "TDEF-001: exactly 53 tools (42 + Issues #172/174/157/165/158/155)"
    );
}

// ── TDEF-002: All tool names are snake_case ──────────────────────────────────

#[test]
fn tdef_002_names_are_snake_case() {
    let tools = all_tools();
    for tool in &tools {
        assert!(
            !tool.name.contains('-'),
            "tool '{}' should not contain hyphens",
            tool.name
        );
        assert!(
            tool.name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "tool '{}' has invalid characters",
            tool.name
        );
    }
}

#[test]
fn tdef_002_all_names_are_unique() {
    let tools = all_tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 53, "all 53 tool names must be unique");
}

// ── TDEF-003: Each tool has a valid JSON schema ──────────────────────────────

#[test]
fn tdef_003_each_tool_has_json_schema() {
    let tools = all_tools();
    for tool in &tools {
        let schema = &tool.input_schema;
        assert_eq!(
            schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "tool '{}' schema must be type object",
            tool.name
        );
        assert!(
            schema.get("properties").is_some(),
            "tool '{}' schema must have properties",
            tool.name
        );
    }
}

#[test]
fn tdef_003_schema_snapshot_get_timeline() {
    let tools = all_tools();
    let tool = tools.iter().find(|t| t.name == "get_timeline").unwrap();
    let json = serde_json::to_string_pretty(&tool.input_schema).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&json).unwrap();
    // get_timeline has no required params
    assert_eq!(
        schema
            .pointer("/required")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(0)
    );
}

#[test]
fn tdef_003_schema_snapshot_split_clip() {
    let tools = all_tools();
    let tool = tools.iter().find(|t| t.name == "split_clip").unwrap();
    let json = serde_json::to_string_pretty(&tool.input_schema).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&json).unwrap();
    let required: Vec<&str> = schema
        .pointer("/required")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(required.contains(&"clipId"), "split_clip requires clipId");
    assert!(required.contains(&"frame"), "split_clip requires frame");
}

// ── TDEF-004: System instruction is present and non-empty ────────────────────

#[test]
fn tdef_004_system_instruction_exists() {
    assert!(
        !SYSTEM_INSTRUCTION.is_empty(),
        "TDEF-004: must be non-empty"
    );
    assert!(
        SYSTEM_INSTRUCTION.contains("Fronda"),
        "TDEF-004: must reference Fronda"
    );
}

// ── TDEF-005: Instruction contract contains key guidance phrases ──────────────

#[test]
fn tdef_005_instruction_contract_key_guidance() {
    let required_phrases = [
        "get_timeline once per session",
        "get_media before referencing",
        "list_models before any generation",
        "inspect_media before describing",
        "user confirmation before execution",
        "terse and outcome-first",
    ];
    for phrase in &required_phrases {
        assert!(
            SYSTEM_INSTRUCTION.contains(phrase),
            "TDEF-005: system instruction must contain '{phrase}'"
        );
    }
}

// ── AID-005: Tool results support text and image blocks ──────────────────────

#[test]
fn aid_005_tool_result_supports_text_block() {
    let text = serde_json::json!({"kind": "text", "text": "hello world"});
    let block: ToolResultBlock = serde_json::from_value(text).unwrap();
    match block {
        ToolResultBlock::Text { text } => assert_eq!(text, "hello world"),
        _ => panic!("expected Text variant"),
    }
}

#[test]
fn aid_005_tool_result_supports_image_block() {
    let img = serde_json::json!({
        "kind": "image",
        "base64": "abcd1234",
        "mediaType": "image/png"
    });
    let block: ToolResultBlock = serde_json::from_value(img).unwrap();
    match block {
        ToolResultBlock::Image { base64, media_type } => {
            assert_eq!(base64, "abcd1234");
            assert_eq!(media_type, "image/png");
        }
        _ => panic!("expected Image variant"),
    }
}

// ── AID-006: Unknown tool returns "Unknown tool: <name>" ─────────────────────

#[test]
fn aid_006_unknown_tool_returns_named_error() {
    let mut executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
    let err = executor
        .execute("nonexistent_tool", &serde_json::json!({}))
        .unwrap_err();
    assert!(
        err.contains("Unknown tool:"),
        "AID-006: error should contain 'Unknown tool:', got: {err}"
    );
    assert!(
        err.contains("nonexistent_tool"),
        "AID-006: error should contain tool name, got: {err}"
    );
}

#[test]
fn aid_006_unknown_tool_error_exact_format() {
    let mut executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
    let err = executor
        .execute("foo_bar", &serde_json::json!({}))
        .unwrap_err();
    assert_eq!(err, "Unknown tool: foo_bar");
}
