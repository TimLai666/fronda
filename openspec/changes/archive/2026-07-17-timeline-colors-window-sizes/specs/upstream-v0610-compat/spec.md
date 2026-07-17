## ADDED Requirements

### Requirement: Timeline clip visuals match the post-281 Swift palette

The timeline SHALL use the upstream #281 clip styling: the darker TrackColor palette (hex source of truth, including the sequence color), fully opaque clip fills, a thin black border only on clips at least the minimum border width (8), a white medium selection ring, and the XS_SM corner radius.

#### Scenario: Narrow clip has no border

- **WHEN** a clip narrower than the minimum border width renders
- **THEN** it draws without the black outline while wider clips draw it

### Requirement: Window defaults and skill frontmatter follow post-319 Swift

Home and Settings default window sizes SHALL be 1200x800, and skill loading SHALL require both a non-blank name and a non-blank description in the frontmatter, skipping (with a log line) files that fail (upstream #319 behavioral slices).

#### Scenario: Skill without description is skipped

- **WHEN** a skill file has a name but a blank description
- **THEN** it is not loaded and a skip line is logged
