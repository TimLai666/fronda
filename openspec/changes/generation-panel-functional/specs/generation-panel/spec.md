## ADDED Requirements

### Requirement: Real model catalog in the picker

The generation panel's model picker SHALL list the generation_core catalog entries for the selected type (video/image/audio) by display name, marking paid-only models as gated for free-tier accounts, replacing all hardcoded rows.

#### Scenario: Video models listed from the catalog

- **WHEN** the user opens the model picker with Video selected
- **THEN** the rows are exactly the catalog's video entries in catalog order and selecting one updates the generation state

### Requirement: Caps-driven settings popover

The gear button SHALL open a settings popover whose controls derive from the selected model's caps (durations, aspect ratios, resolutions, quality, count for video; count for image; instrumental/generate-audio toggles and voices for audio), persisting choices in the panel state.

#### Scenario: Switching models re-derives settings

- **WHEN** the user switches to a model whose caps lack the previously chosen duration
- **THEN** the setting falls back to that model's default and the popover shows only valid options

### Requirement: Cost estimate and credit gating

The panel SHALL show an estimated cost for the current model+parameters and disable Generate with an insufficient-credits message when the estimate exceeds the remaining credits.

#### Scenario: Insufficient credits

- **WHEN** the estimated cost exceeds credits_remaining
- **THEN** Generate is disabled and the panel explains the shortfall instead of silently doing nothing

### Requirement: Real submission path

Generate SHALL build a GenerationInput (prompt, model id, parameters, reference asset ids) and submit through the GenerationBackend seam; without a backend the panel SHALL show an explicit unavailable state, and is_generating SHALL reflect only real in-flight work.

#### Scenario: No backend installed

- **WHEN** Generate is pressed with no GenerationBackend
- **THEN** the panel shows generation is unavailable (no fake spinner) and no state is corrupted

### Requirement: Reference tiles hold real assets

Reference tiles SHALL accept a media-library asset (click-to-pick), render its thumbnail, support clearing, and enforce the per-model reference cap.

#### Scenario: Assign and clear a reference

- **WHEN** the user picks an asset for a tile and later clicks its clear button
- **THEN** the tile shows the asset's thumbnail while assigned and returns to the empty state after clearing
