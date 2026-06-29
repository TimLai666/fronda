# `fronda-app-shell-gpui`

This crate is the active `gpui-ce` app shell for `Fronda`.

Relevant specs:

- `specs/rust-rewrite/03-timeline-editor-and-preview.md`
- `specs/rust-rewrite/06-search-transcription-generation-and-shell.md`
- `specs/rust-rewrite/07-ui-port-spec.md`
- `specs/rust-rewrite/98-verification-plan.md`

It is intentionally split in two layers:

- the library target keeps shell copy and shell state in plain Rust so workspace tests stay portable
- the `fronda` binary is feature-gated behind `desktop-app` so CI can check the real `gpui-ce` desktop shell on supported runners without forcing every workspace test environment to build it

## Commands

Run the pure Rust workspace tests:

```bash
cargo test --workspace
```

Check the `gpui-ce` desktop shell binary on a supported platform:

```bash
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

Launch the desktop shell on a supported platform:

```bash
cargo run -p fronda-app-shell-gpui --features desktop-app --bin fronda
```
