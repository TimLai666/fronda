## ADDED Requirements

### Requirement: Project card interactions

Recent-project cards SHALL show hover feedback, a hover-revealed delete button with a confirmation step, and a file-missing overlay when the project path no longer exists.

#### Scenario: Missing project file

- **WHEN** a recent project's directory has been deleted on disk
- **THEN** its card dims with a file-missing indicator and opening it is prevented with an explanation

### Requirement: Open Project file panel

The sidebar Open Project action SHALL present the platform folder picker and open the chosen .palmier project.

#### Scenario: Pick a project

- **WHEN** the user picks a valid project directory in the panel
- **THEN** the editor opens that project exactly like a recents click

### Requirement: Preview settings menus

The preview header SHALL offer Aspect-Ratio, Frame-Rate, Resolution/Quality, and Zoom menus fed by the project presets, applying selections through the standard settings path.

#### Scenario: Change fps

- **WHEN** the user picks a different frame rate
- **THEN** the timeline settings update via set_project_settings semantics (rescale prompts included)

### Requirement: Capture frame

A Capture Frame button SHALL composite the current paused frame and add it to the media library as an image asset.

#### Scenario: Capture

- **WHEN** the user hits Capture Frame while paused
- **THEN** a PNG of the composited frame lands in the project media and appears in the library

### Requirement: Tour spotlight and Add-Text

The tour overlay SHALL visually spotlight the current step's anchor region (dimming everything else), and the toolbar SHALL include the Add-Text button inserting a default text clip at the playhead.

#### Scenario: Add text

- **WHEN** the user clicks the toolbar "T" button
- **THEN** a text clip appears at the playhead on the appropriate track
