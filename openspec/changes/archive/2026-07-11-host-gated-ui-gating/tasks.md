## 1. Availability API

- [x] 1.1 `is_generation_available()` / `is_transcription_available()` reflect installed seams. Verified by `availability_flags_reflect_installed_seams`.

## 2. UI gating

- [x] 2.1 Generation panel disables submit and shows the coming-soon status when no backend (`submit_enabled = can_submit && gen_available`; status override). Verified by compile of the desktop-app `fronda` bin (render reads the shared executor).
- [x] 2.2 AI-edit tab gates generate/re-run/upscale and shows the coming-soon status when no backend. Verified by compile of the `fronda` bin.
- [x] 2.3 Captions tab disables Generate Captions and shows the coming-soon note when no transcription provider. Verified by compile of the `fronda` bin.

## 3. Gates

- [x] 3.1 `cargo test --workspace` exit 0.
- [x] 3.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (420 passed).
- [x] 3.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.
