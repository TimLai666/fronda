## ADDED Requirements

### Requirement: Multiline text editing

The TextArea component SHALL provide multiline text editing through the platform text-input path: printable characters and IME composition (including CJK marked text with underline) insert at the cursor, Enter inserts a newline, and the field SHALL NOT emit any submit event.

#### Scenario: Typing with IME composition

- **WHEN** the user composes CJK text via an IME while the TextArea is focused
- **THEN** the marked (uncommitted) text renders underlined at the cursor, and committing replaces it with the final text and emits an Edited event

#### Scenario: Enter inserts a newline

- **WHEN** the user presses Enter (no modifiers) in a focused TextArea
- **THEN** a newline is inserted at the cursor and no submit occurs

### Requirement: Wrapping and layout

The TextArea SHALL soft-wrap text to its element width, honor hard line breaks, and grow its height with content between a configurable minimum and maximum number of lines.

#### Scenario: Long line wraps

- **WHEN** a line's shaped width exceeds the element width
- **THEN** the line wraps and the element height reflects the wrapped line count, up to the configured maximum

### Requirement: Cursor, selection, and clipboard

The TextArea SHALL support cursor movement by character (left/right) and by visual line (up/down), shift-extended and mouse-driven selection, and clipboard cut/copy/paste where paste preserves newlines.

#### Scenario: Paste keeps newlines

- **WHEN** the user pastes text containing newline characters
- **THEN** the pasted content retains its line breaks in the TextArea content

#### Scenario: Vertical cursor movement

- **WHEN** the user presses down with the cursor on a visual line that has a line below it
- **THEN** the cursor moves to the index on the next visual line closest to its current x position

### Requirement: Global shortcut isolation

The TextArea's key context SHALL include the `input` identifier so that modifier-free global shortcut bindings predicated on `!input` remain inert while the field is focused; escape and tab SHALL bubble to the host view.

#### Scenario: Space types instead of toggling playback

- **WHEN** the user presses space while a TextArea is focused
- **THEN** a space character is inserted and the global play/pause action does not fire

### Requirement: Host mirroring via events

The TextArea SHALL emit an Edited event on every content change from typing, IME commit, or paste, and SHALL NOT emit Edited from a programmatic set_text call; set_text SHALL reset selection and any marked composition range.

#### Scenario: Host model stays in sync

- **WHEN** a host view subscribes to Edited and mirrors text() into its model
- **THEN** the model reflects the field content after every user edit without double-applying programmatic updates
