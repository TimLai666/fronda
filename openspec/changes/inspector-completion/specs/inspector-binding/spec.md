## ADDED Requirements

### Requirement: Rows bind to the selected clip

Inspector numeric rows (transform, volume, speed, opacity) SHALL display the selected clip's current values (keyframe-resolved at the playhead where applicable) and scrubbing SHALL write back through the standard clip-property tools; each section SHALL offer a reset to defaults.

#### Scenario: Selection drives values

- **WHEN** the user selects a clip whose scale is 0.5
- **THEN** the Scale row shows 0.5 (not a default), and scrubbing it to 0.7 updates the clip

### Requirement: Crop and Flip controls

The Crop row SHALL provide an enable toggle and aspect menu bound to the clip's crop, and the Flip row SHALL provide H/V toggles bound to the clip's flip flags.

#### Scenario: Flip toggles

- **WHEN** the user toggles Flip H on a selected clip
- **THEN** the clip's flip_horizontal flag flips and the preview mirrors accordingly

### Requirement: Real source metadata

The Source section SHALL show the asset's real file data (dimensions, size, path), an AI badge for generated assets, the Generated parameters from generation_input, and the prompt with a copy button.

#### Scenario: Generated asset

- **WHEN** an AI-generated asset is selected
- **THEN** the AI badge, its generation model/parameters, and its prompt (copyable) are shown
