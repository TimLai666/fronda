## Why

The porting table marked chroma key "done", but Rust's `apply_chroma_key` was a simplified hard **RGB-distance cutoff** (binary alpha, no feathering) — nowhere near Swift's soft **hue-based** key. Green-screen output had hard, aliased edges. The `key.chroma` tool even carried a `softness` param (matching Swift), but the mirror into `Clip.chroma_key` dropped it, so the renderer couldn't feather at all. This closes the one concrete quality-parity gap found while auditing upstream before release. It also folds in upstream #291 (chroma-key correctness fix).

## What Changes

- `apply_chroma_key` (compositor) rewritten to mirror Swift's `Metal/ChromaKey.metal`:
  - Per-pixel HSV decomposition; `k = (1 − smoothstep hue-closeness) · smoothstep(saturation) · smoothstep(chroma dd)`.
  - Alpha `*= (1 − k)` for **feathered edges**; spill desaturates leftover key tint toward luma proportional to `spill·k`.
  - The chroma (`dd`) gate keeps dark / near-grey pixels that share the key hue (upstream #291 fix), instead of over-keying them.
  - Key hue derived from the stored key RGB; new `smoothstep` / `hue_sat_chroma` helpers.
- `core_model::ChromaKey` gains a `softness` field (`#[serde(default = 0.1]`, backward-compatible); presets and the tool mirror set it.
- The `key.chroma` tool now mirrors `softness` into `Clip.chroma_key` (was dropped).
- Compositor tests rewritten to the new (correct) hue-based semantics + new soft-edge and low-chroma-gate cases.

## Non-Goals

- No agent/MCP tool-surface change (the `key.chroma` params already included `softness`).
- Eyedropper / preview color sampler (UI part of #291) — separate, not in this change.
- No change to the effect on-disk params; only the render-model mirror gains `softness`.

## Capabilities

### New Capabilities

- `chroma-key-soft-edges`: the CPU compositor keys green/blue screens with soft, feathered edges matching Swift, gated on hue + saturation + chroma.

## Impact

- Affected code:
  - Modified: `crates/render_core/src/compositor.rs` (algorithm + helpers + 5 tests), `crates/core_model/src/timeline.rs` (`softness` field + presets), `crates/agent_contract/src/tool_exec.rs` (mirror), `crates/agent_contract/src/timeline_v2.rs` (test literals), `AGENTS.md`
- On-disk: `ChromaKey` serialization gains `softness` (additive, serde-default; old files still decode). No agent/MCP contract change.
