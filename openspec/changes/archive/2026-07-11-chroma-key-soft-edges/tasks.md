## 1. Data model

- [x] 1.1 `ChromaKey` gains a serde-default `softness` field; presets set it; legacy data still decodes. Verified by `cargo test --workspace` (core_model compatibility) exit 0.

## 2. Algorithm

- [x] 2.1 `apply_chroma_key` mirrors Swift's soft hue-based kernel (hue/sat/chroma smoothsteps, `alpha *= 1-k`, luma-mix spill, `dd` gate). Verified by `chroma_key_makes_key_colour_transparent`, `chroma_key_soft_edge_gives_partial_alpha`, `chroma_key_keeps_low_chroma_pixels`, `chroma_key_spill_suppression_reduces_green_bleed`, `chroma_key_disabled_is_noop`.
- [x] 2.2 `key.chroma` tool mirrors `softness` into `Clip.chroma_key` (was dropped). Verified by compile + the timeline_v2 chroma tests.

## 3. Gates

- [x] 3.1 `cargo test --workspace` exit 0.
- [x] 3.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (420 passed).
- [x] 3.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.
