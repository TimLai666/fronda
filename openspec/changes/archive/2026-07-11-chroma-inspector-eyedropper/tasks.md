## 1. Pure logic

- [x] 1.1 `chroma_controls`: rgb↔hue, `ChromaControls` (read/apply-args), `frame_uv_from_click`. Verified by 6 `chroma_controls` tests.

## 2. Inspector section

- [x] 2.1 Chroma params threaded through the scrub machinery (default/derive/commit→apply_effect/fmt). Verified by compile + the desktop-app suite (426 passed).
- [x] 2.2 Chroma Key section (toggle/swatch/presets/eyedropper/sliders) shown for visual clips, driving `apply_effect`. Verified by `fronda` bin compile.

## 3. Preview eyedropper

- [x] 3.1 `chroma_sampling` cross-view arm/consume; preview bounds-capture `canvas()` + click handler sample the frame PNG and apply `key.chroma`. Verified by `fronda` bin compile.

## 4. Gates

- [x] 4.1 `cargo test --workspace` exit 0.
- [x] 4.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (426 passed).
- [x] 4.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.

## 5. Human verification (out of this repo's reach)

- [x] 5.1 Interaction correctness (eyedropper click accuracy, slider feel, layout) is verified by a human running the app — this repo cannot run gpui. Recorded as the explicit boundary; compile + pure-logic tests cover everything machine-checkable here.
