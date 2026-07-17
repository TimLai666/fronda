## ADDED Requirements

### Requirement: Audio sync uses seeded search with global fallback

find_sync_offset SHALL support a seeded center-lag window (from capture-date deltas when available) searched before the global range, falling back to the full search when the seeded window's confidence is insufficient, matching upstream #269 seeding semantics; update_text SHALL reject the former flat style keys now that the inspector sends nested style patches.

#### Scenario: Bad seed still finds the true offset

- **WHEN** the capture-date seed points far from the true alignment
- **THEN** the global fallback search returns the same offset as an unseeded run

#### Scenario: Flat style keys rejected

- **WHEN** update_text is called with a top-level fontSize
- **THEN** validation fails directing the caller to the nested style object
