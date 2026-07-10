## ADDED Requirements

### Requirement: Text tab edits the selected text clip

With a text clip selected, the Text tab SHALL show and edit its TextStyle: content (multiline), font family, size, opacity, color, alignment, background (color + toggle), shadow, stroke, and position, writing changes back through the standard text-update path so undo and persistence behave like any other edit.

#### Scenario: Editing content updates the clip

- **WHEN** the user edits the content field and commits
- **THEN** the selected clip's text updates on the timeline/preview and undo reverts it

#### Scenario: Style round-trip

- **WHEN** the user picks a font, size, and color
- **THEN** the clip's TextStyle carries those values after save/reload
