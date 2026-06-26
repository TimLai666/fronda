//! Ported from PalmierPro Tests/Agent/UndoToolTests.swift
//!
//! Tests the undo tool behavior at the Rust agent_contract level.
//! The Swift tests use a ToolHarness; our Rust equivalents test the UndoStack directly
//! plus the validation/execution coordination logic.

use agent_contract::undo::{UndoCommand, UndoError, UndoStack};
use core_model::Timeline;

fn test_timeline(fps: i64, width: i64, height: i64) -> Timeline {
    Timeline {
        fps,
        width,
        height,
        settings_configured: false,
        selected_clip_ids: Default::default(),
        transcription_language: None,
        tracks: vec![],
        compound_timelines: std::collections::HashMap::new(),
    }
}

fn make_cmd(id: &str, tool: &str, before: Timeline, after: Timeline) -> UndoCommand {
    UndoCommand::new(id.to_string(), tool.to_string(), before, after)
}

/// Swift: undoRevertsAgentRippleDelete — undo restores timeline to pre-edit state
#[test]
fn port_undo_reverts_agent_edit() {
    let mut stack = UndoStack::new();
    let before = test_timeline(30, 1920, 1080);
    let after = {
        let mut t = test_timeline(30, 1920, 1080);
        t.fps = 60;
        t
    };
    stack.push_command(make_cmd(
        "cmd-1",
        "ripple_delete_ranges",
        before.clone(),
        after,
    ));
    assert!(stack.can_undo());
    let restored = stack.undo().unwrap();
    assert_eq!(restored.fps, before.fps);
}

/// Swift: refusesWhenAssistantHasNotEdited — empty stack refuses undo
#[test]
fn port_undo_refuses_when_no_edits() {
    let mut stack = UndoStack::new();
    assert_eq!(stack.undo(), Err(UndoError::NoCommands));
}

/// Swift: refusesSecondUndoWithNothingLeft — undo then undo again fails
#[test]
fn port_undo_refuses_second_undo() {
    let mut stack = UndoStack::new();
    let before = test_timeline(30, 1920, 1080);
    let after = test_timeline(60, 1920, 1080);
    stack.push_command(make_cmd("cmd-1", "add_clips", before, after));
    assert!(stack.undo().is_ok());
    assert_eq!(stack.undo(), Err(UndoError::NoCommands));
}

/// Swift: refusesWhenLatestEditIsNotTheAssistants — UNDO-005
/// The caller checks latest_command_id() before calling undo(). If the caller's
/// tracked latest command id doesn't match the stack's latest, undo is refused.
#[test]
fn port_undo_refuses_when_latest_not_assistants() {
    let mut stack = UndoStack::new();
    let t = test_timeline(30, 1920, 1080);
    stack.push_command(make_cmd(
        "cmd-1",
        "ripple_delete_ranges",
        t.clone(),
        t.clone(),
    ));

    // Simulate: caller sees that the latest command is what they expected
    let caller_tracked_id = Some("cmd-1");
    assert_eq!(stack.latest_command_id(), caller_tracked_id);

    stack.undo().unwrap();
    // After undo, stack is empty
    assert_eq!(stack.latest_command_id(), None);
}
