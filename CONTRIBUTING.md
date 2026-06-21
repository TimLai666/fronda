# Contributing

## Scope

`Fronda` is the planned name of the Rust rewrite of Palmier Pro. The current Swift app still serves as the compatibility baseline while that rewrite is in progress.

The best way to contribute is to open a GitHub issue first. Bug reports, parity gaps, spec corrections, and migration proposals are all useful.

Large unsolicited feature PRs against the legacy Swift app are unlikely to be accepted while the rewrite is in progress.

## Getting started

### Prerequisites

- macOS 26+
- Xcode 16+
- Swift 6.2 toolchain

### Clone

```bash
git clone https://github.com/TimLai666/fronda
cd fronda
```

### Build the current Swift baseline

```bash
swift build
swift run
```

For a bundled debug build that launches the `.app` and streams OSLog:

```bash
./scripts/dev.sh
```

## Test

```bash
swift test
```

CI runs `swift build` and `swift test` on every pull request, on pushes to `main`, and on manual workflow dispatch. The workflow lives at `.github/workflows/ci.yml` and targets `macos-26`, matching the app's current deployment target.

## Rewrite workflow

For Rust rewrite work:

1. treat `specs/rust-rewrite/` as the compatibility baseline
2. preserve observable behavior unless you are intentionally changing product behavior
3. update the relevant spec in the same change when behavior changes intentionally
4. bring tests or fixtures with the subsystem you port
5. avoid piecemeal renames of inherited `PalmierPro` / `palmier-pro` identifiers unless the migration is explicit and spec-backed

## Licensing

By contributing, you agree your contributions are licensed under [GPLv3](LICENSE).
