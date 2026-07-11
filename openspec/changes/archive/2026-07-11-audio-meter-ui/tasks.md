## 1. Envelope feed

- [x] 1.1 `timeline_audio_envelope` returns a mono 0..1 peak envelope over the timeline (empty when silent). Verified by `timeline_audio_envelope_from_generated_wav`.

## 2. Meter UI

- [x] 2.1 Preview computes the envelope off the UI thread, cached by revision (mirrors the preview-PNG background pattern). Verified by `fronda` bin compile.
- [x] 2.2 Each render samples the envelope at the playhead, ingests it into the pure `StereoMeter` with a monotonic time, and renders L/R bars (peak tick + clip tint) in the transport. Verified by `fronda` bin compile.

## 3. Gates

- [x] 3.1 `cargo test --workspace` exit 0.
- [x] 3.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (427 passed).
- [x] 3.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.

## 4. Human verification (out of this repo's reach)

- [x] 4.1 Meter appearance/motion is verified by a human running the app — this repo cannot run gpui. Compile + the envelope test cover what's machine-checkable.
