## ADDED Requirements

### Requirement: Caption styling controls

The Captions tab SHALL provide working controls for source, language, font family (from the bundled font list), size, color, background (with toggle), case, and profanity censoring, persisting into the caption configuration used at generation time.

#### Scenario: Style change reflects in the preview

- **WHEN** the user changes the font size and background color
- **THEN** the live preview box re-renders the sample caption with those values

### Requirement: Live preview with placement

The tab SHALL render a caption preview box with center guides and scrubbable X/Y placement fields that update the configured caption position.

#### Scenario: Scrubbing placement moves the sample

- **WHEN** the user scrubs the Y field downward
- **THEN** the sample caption in the preview moves accordingly and the position persists

### Requirement: Generation gating

Generate SHALL require transcribed words: with none available it shows why (no transcription provider / no speech) instead of a fake progress state, and during a real run it shows the transcribing overlay.

#### Scenario: No words available

- **WHEN** the user hits Generate with no transcription available
- **THEN** an explanatory note appears and no overlay spins
