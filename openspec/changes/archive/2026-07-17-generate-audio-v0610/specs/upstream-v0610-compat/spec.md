## ADDED Requirements

### Requirement: generate_audio supports source-based categories per Swift v0.6.10

generate_audio SHALL accept sourceMediaRef and targetLanguage, gate cleanup/dubbing categories on a source asset (rejecting silent-video sources with the upstream message and deriving duration from the source), emit list_models audio entries with inputs/minSeconds/maxSeconds/targetLanguages when present, and expand sourceMediaRef through the short-id system, while keeping the honest backend-absent error (upstream #294 contract layer; catalog entries stay dormant until a generation backend exists).

#### Scenario: Dubbing requires a source

- **WHEN** generate_audio targets a dubbing-category model without sourceMediaRef
- **THEN** the call fails with the upstream source-required error

#### Scenario: Silent video source rejected

- **WHEN** sourceMediaRef points at a video whose has_audio is false
- **THEN** the call fails with the upstream no-audio message before any backend interaction
