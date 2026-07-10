## ADDED Requirements

### Requirement: Mode, model, and prompt

The Music tab SHALL offer an input-mode menu (Video to Music / Text to Music), a model menu listing the catalog's music-capable audio entries, a duration scrub in text mode, and a real prompt field.

#### Scenario: Text mode exposes duration

- **WHEN** the user switches to Text to Music
- **THEN** the duration scrub appears and the source-span summary hides

### Requirement: Cost and credit gating

The tab SHALL show the cost estimate for the selection and disable generation with an explanatory note when credits are insufficient or no backend is available.

#### Scenario: No backend

- **WHEN** Generate is pressed with no generation backend installed
- **THEN** the tab shows an unavailable note and no fake overlay runs
