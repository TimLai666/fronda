//! Undo system for agent timeline edits (UNDO-001 to UNDO-006).
//!
//! Snapshot-based undo stack scoped to the current runtime session.
//! Tracks timeline snapshots before/after each assistant-made mutation.

use core_model::Timeline;

/// A snapshot-based undo command storing a timeline before/after pair.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoCommand {
    /// UUID command id.
    pub id: String,
    /// Which tool produced this edit (e.g. "add_clips", "move_clips").
    pub tool_name: String,
    /// Snapshot before the edit.
    pub before: Timeline,
    /// Snapshot after the edit (for potential redo).
    pub after: Timeline,
}

impl UndoCommand {
    pub fn new(id: String, tool_name: String, before: Timeline, after: Timeline) -> Self {
        Self {
            id,
            tool_name,
            before,
            after,
        }
    }
}

/// Errors that can occur during undo.
#[derive(Debug, Clone, PartialEq)]
pub enum UndoError {
    /// UNDO-004: No tracked undoable edit.
    NoCommands,
}

/// Session-scoped undo stack for assistant-made timeline edits.
///
/// Commands are ordered most-recent-first: push to the end, pop from the end.
/// This satisfies UNDO-003 (most-recent-first undo).
#[derive(Debug, Clone)]
pub struct UndoStack {
    commands: Vec<UndoCommand>,
    max_depth: usize,
}

impl UndoStack {
    /// Creates an empty stack with the default max depth of 50.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            max_depth: 50,
        }
    }

    /// Creates an empty stack with a custom max depth.
    pub fn with_max_depth(depth: usize) -> Self {
        Self {
            commands: Vec::new(),
            max_depth: depth,
        }
    }

    /// Pushes a command onto the stack.
    ///
    /// If the stack exceeds `max_depth`, the oldest command (front) is trimmed.
    /// Most-recent-first ordering: push to end, pop from end (UNDO-003).
    pub fn push_command(&mut self, cmd: UndoCommand) {
        if self.max_depth == 0 {
            return;
        }
        if self.commands.len() >= self.max_depth {
            self.commands.remove(0);
        }
        self.commands.push(cmd);
    }

    /// Pops and restores the most recent command's `before` snapshot.
    ///
    /// Returns `UndoError::NoCommands` if the stack is empty (UNDO-004).
    pub fn undo(&mut self) -> Result<Timeline, UndoError> {
        let cmd = self.commands.pop().ok_or(UndoError::NoCommands)?;
        Ok(cmd.before)
    }

    /// Returns true if the stack is non-empty.
    pub fn can_undo(&self) -> bool {
        !self.commands.is_empty()
    }

    /// Returns a reference to the most recent command, if any.
    pub fn peek(&self) -> Option<&UndoCommand> {
        self.commands.last()
    }

    /// Returns the id of the most recent command, if any.
    ///
    /// Callers can use this to verify UNDO-005: that the latest undoable
    /// change was still the assistant's latest change.
    pub fn latest_command_id(&self) -> Option<&str> {
        self.commands.last().map(|cmd| cmd.id.as_str())
    }

    /// Removes all commands from the stack.
    pub fn clear(&mut self) {
        self.commands.clear();
    }

    /// Returns the number of commands on the stack.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if the stack contains no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::Timeline;

    fn test_timeline(fps: i64, width: i64, height: i64) -> Timeline {
        Timeline {
            fps,
            width,
            height,
            settings_configured: false,
            selected_clip_ids: Default::default(),
            tracks: vec![],
        }
    }

    fn make_cmd(id: &str, tool: &str, before: Timeline, after: Timeline) -> UndoCommand {
        UndoCommand::new(id.to_string(), tool.to_string(), before, after)
    }

    /// UNDO-004: undo on empty stack returns error.
    #[test]
    fn undo_001_empty_stack_refuses() {
        let mut stack = UndoStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.undo(), Err(UndoError::NoCommands));
    }

    /// UNDO-003: push a command, undo restores the before snapshot.
    #[test]
    fn undo_002_push_and_undo() {
        let mut stack = UndoStack::new();
        let before = test_timeline(30, 1920, 1080);
        let after = test_timeline(30, 1920, 1080);

        stack.push_command(make_cmd("cmd-1", "add_clips", before.clone(), after));
        assert!(stack.can_undo());

        let restored = stack.undo().unwrap();
        assert_eq!(restored, before);
        assert!(!stack.can_undo());
    }

    /// UNDO-003: push two commands, undo restores the last pushed.
    #[test]
    fn undo_003_most_recent_first() {
        let mut stack = UndoStack::new();

        let before_1 = test_timeline(30, 1920, 1080);
        let after_1 = test_timeline(30, 1920, 1080);
        let before_2 = test_timeline(60, 1920, 1080);
        let after_2 = test_timeline(60, 1920, 1080);

        stack.push_command(make_cmd("cmd-1", "add_clips", before_1, after_1));
        stack.push_command(make_cmd("cmd-2", "move_clips", before_2.clone(), after_2));

        // UNDO-003: most-recent-first — should restore before_2 (the second push).
        let restored = stack.undo().unwrap();
        assert_eq!(restored.fps, 60);
        assert_eq!(stack.len(), 1);

        // Second undo restores the first.
        let restored_2 = stack.undo().unwrap();
        assert_eq!(restored_2.fps, 30);
        assert!(stack.is_empty());
    }

    /// UNDO-003 / max_depth: pushing beyond max_depth trims the oldest.
    #[test]
    fn undo_004_max_depth_trims_oldest() {
        let mut stack = UndoStack::with_max_depth(3);

        for i in 0..5 {
            let before = test_timeline(i * 10, 1920, 1080);
            let after = test_timeline(i * 10, 1920, 1080);
            stack.push_command(make_cmd(&format!("cmd-{i}"), "add_clips", before, after));
        }

        // max_depth = 3, only the last 3 should remain (cmd-2, cmd-3, cmd-4).
        assert_eq!(stack.len(), 3);

        // Most recent first: cmd-4, cmd-3, cmd-2.
        assert_eq!(stack.undo().unwrap().fps, 40);
        assert_eq!(stack.undo().unwrap().fps, 30);
        assert_eq!(stack.undo().unwrap().fps, 20);
        assert!(stack.is_empty());
    }

    /// can_undo returns true after push, false after undo.
    #[test]
    fn undo_005_can_undo_after_push() {
        let mut stack = UndoStack::new();
        assert!(!stack.can_undo());

        let before = test_timeline(30, 1920, 1080);
        let after = test_timeline(30, 1920, 1080);
        stack.push_command(make_cmd("cmd-1", "add_clips", before, after));

        assert!(stack.can_undo());

        stack.undo().unwrap();
        assert!(!stack.can_undo());
    }

    /// clear empties the stack.
    #[test]
    fn undo_006_clear_resets() {
        let mut stack = UndoStack::new();

        let before = test_timeline(30, 1920, 1080);
        let after = test_timeline(30, 1920, 1080);
        stack.push_command(make_cmd("cmd-1", "add_clips", before, after));
        assert_eq!(stack.len(), 1);

        stack.clear();
        assert!(stack.is_empty());
        assert!(!stack.can_undo());
        assert_eq!(stack.undo(), Err(UndoError::NoCommands));
    }

    /// latest_command_id returns the id of the most recent command.
    #[test]
    fn undo_007_latest_command_id() {
        let mut stack = UndoStack::new();
        assert!(stack.latest_command_id().is_none());

        let before = test_timeline(30, 1920, 1080);
        let after = test_timeline(30, 1920, 1080);
        stack.push_command(make_cmd("cmd-1", "add_clips", before, after));

        assert_eq!(stack.latest_command_id(), Some("cmd-1"));

        let before_2 = test_timeline(60, 1920, 1080);
        let after_2 = test_timeline(60, 1920, 1080);
        stack.push_command(make_cmd("cmd-2", "move_clips", before_2, after_2));

        assert_eq!(stack.latest_command_id(), Some("cmd-2"));

        stack.undo().unwrap();
        assert_eq!(stack.latest_command_id(), Some("cmd-1"));

        stack.undo().unwrap();
        assert!(stack.latest_command_id().is_none());
    }
}
