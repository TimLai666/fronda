## ADDED Requirements

### Requirement: Live search

The media tab's search box SHALL be a real text field that filters the visible grid by name substring as the user types (IME included), with a clear button restoring the full view.

#### Scenario: Typing filters the grid

- **WHEN** the user types "roll" with assets "A-roll", "B-roll", "music"
- **THEN** only A-roll and B-roll tiles remain visible, and clearing restores all three

### Requirement: Folder navigation

The grid SHALL render folder tiles for the manifest's folders, enter a folder on double-click showing only its assets, provide a breadcrumb to navigate back, and support creating and renaming folders.

#### Scenario: Enter and leave a folder

- **WHEN** the user double-clicks a folder tile and then clicks the root breadcrumb
- **THEN** the grid shows the folder's assets inside and the full library after returning

### Requirement: View, sort, and filter menus

The toolbar SHALL provide View (Folders/Flat/Grouped), Sort (name/date/type), and Filter (media type, AI-generated, clear) menus whose selections immediately reorganize the grid.

#### Scenario: Filter by type

- **WHEN** the user filters to Audio
- **THEN** only audio assets (and no folders in Flat view) remain visible until the filter clears

### Requirement: Multi-select and batch delete

The grid SHALL support toggling selection with ctrl/cmd-click and range-extending with shift-click, and a batch delete acting on the whole selection.

#### Scenario: Range select

- **WHEN** the user clicks asset 2 then shift-clicks asset 5 in the current ordering
- **THEN** assets 2..5 are selected and delete removes all of them

### Requirement: Status row

The tab SHALL show the visible item count and the search index status, and render the empty state when the library has no assets.

#### Scenario: Empty library

- **WHEN** a project has no media
- **THEN** the empty-state view renders instead of a blank grid
