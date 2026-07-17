## ADDED Requirements

### Requirement: Text styling renders and patches per Swift v0.6.10

The text renderer SHALL draw underline/strikethrough/overline bars, apply fontCase display transformation, tracking, additive lineSpacing, the rich background (per-axis padding, corner radius, offsets, outline) and border-width glyph stroke; the add_texts/update_text/add_captions tools SHALL take the nested partial-patch style object (flat style fields removed, add_captions textCase removed, 6-digit hex preserving current alpha vs 8-digit setting it); FCPXML SHALL emit strokeWidth from border width and title text through fontCase (upstream #330/#336 remainders).

#### Scenario: Nested style partial patch

- **WHEN** update_text patches style.outline.width only
- **THEN** outline width changes and every other style field keeps its prior value

#### Scenario: Uppercase fontCase renders transformed text

- **WHEN** a text clip has fontCase "uppercase"
- **THEN** rendering and FCPXML titles use the uppercased text while the stored content is unchanged
