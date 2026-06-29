# Contributing

## Scope

`Fronda` is the primary Rust codebase in this fork of Palmier Pro. The legacy Swift app remains in the repo as a compatibility reference, not the main development target.

The best way to contribute is to open a GitHub issue first. Bug reports, parity gaps, spec corrections, and migration proposals are all useful.

Large unsolicited feature PRs against the legacy Swift app are unlikely to be accepted unless they directly protect parity or unblock the Rust codebase.

## Getting started

### Prerequisites for Fronda work

- Rust stable toolchain
- desktop toolchain for `gpui-ce` if you are touching the shell

### Optional prerequisites for legacy Swift compatibility work

- macOS 26+
- Xcode 16+
- Swift 6.2 toolchain

### Clone

```bash
git clone https://github.com/TimLai666/fronda
cd fronda
```

### Test the Rust workspace

```bash
cargo test --workspace
```

### Check the desktop shell

```bash
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

### Run the desktop shell

```bash
cargo run -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

### Legacy Swift compatibility baseline

```bash
swift build
swift run
swift test
```

For a bundled debug build that launches the `.app` and streams OSLog:

```bash
./scripts/dev.sh
```

CI runs Rust workspace tests and shell checks on every pull request, push to `main`, and manual workflow dispatch. It also keeps the legacy Swift compatibility baseline building and testing on macOS through `.github/workflows/ci.yml`.

## Rewrite workflow

For Fronda work:

1. treat `specs/rust-rewrite/` as the compatibility baseline
2. preserve observable behavior unless you are intentionally changing product behavior
3. update the relevant spec in the same change when behavior changes intentionally
4. bring tests or fixtures with the subsystem you port
5. avoid piecemeal renames of inherited `PalmierPro` / `palmier-pro` identifiers unless the migration is explicit and spec-backed

For legacy Swift changes, keep the scope narrow and tie the change back to parity, fixtures, or compatibility evidence.

## Licensing

By contributing, you agree your contributions are licensed under [GPLv3](LICENSE).
