## ADDED Requirements

### Requirement: Chroma key uses a soft hue-based algorithm

`apply_chroma_key` SHALL key a pixel by hue proximity to the key colour,
producing a soft factor `k ∈ [0,1]` from the product of a hue-closeness
smoothstep (widened by `softness`), a saturation smoothstep, and a chroma
(`dd = max−min`) smoothstep. Output alpha SHALL scale by `(1 − k)` so edges
feather, mirroring Swift's `Metal/ChromaKey.metal`. A disabled key SHALL be a
no-op.

#### Scenario: pure key hue fully keyed, off-hue kept

- **WHEN** a pure green pixel and a pure red pixel are keyed with a green key
- **THEN** the green pixel's alpha SHALL be 0 and the red pixel SHALL be unchanged

#### Scenario: hue-feather edge yields partial alpha

- **WHEN** a pixel whose hue sits in the feather band of the key (e.g. cyan against a green key with tolerance 0.4, softness 0.5) is keyed
- **THEN** its alpha SHALL be strictly between 0 and 255

### Requirement: Low-chroma pixels are not over-keyed

A pixel that shares the key hue but has little chroma (dark or near-grey) SHALL
be kept opaque — the chroma (`dd`) gate SHALL suppress keying below the chroma
threshold (upstream #291).

#### Scenario: dark near-grey key-hue pixel kept

- **WHEN** a dark, low-chroma green-hue pixel is keyed with a green key
- **THEN** its alpha SHALL remain 255

### Requirement: Spill suppression desaturates the leftover key tint

For a partially-keyed pixel with spill > 0, the RGB SHALL be mixed toward its
luma proportional to `spill · k`, reducing the leftover key-colour bleed.

#### Scenario: spill reduces green bleed on a soft edge

- **WHEN** a partially-keyed cyan pixel is keyed with a green key and spill 1.0
- **THEN** its alpha SHALL be partial and its green channel SHALL be reduced from full

### Requirement: ChromaKey carries softness compatibly

`core_model::ChromaKey` SHALL include a `softness` field defaulting to 0.1 when
absent, so existing `.palmier` / media data without it still decodes. The
`key.chroma` tool SHALL mirror the effect's `softness` param into
`Clip.chroma_key`.

#### Scenario: legacy data decodes with default softness

- **WHEN** a `ChromaKey` is decoded from data lacking `softness`
- **THEN** decoding SHALL succeed with `softness` = 0.1
