<div align="center">

# Fronda

**Cross-platform Rust video editor workspace derived from Palmier Pro.**

</div>

<img src="./assets/palmier-ui.png" alt="Palmier Pro Swift compatibility baseline UI" width="900" />

This repository is a modified fork of [Palmier Pro](https://github.com/palmier-io/palmier-pro). `Fronda` is the primary product and codebase in this fork: a cross-platform Rust editor built around `gpui-ce`, with the inherited Swift app retained as a legacy compatibility reference.

## Current status

- Primary implementation: Rust workspace
- Primary UI stack: `gpui-ce`
- Legacy compatibility reference: Palmier Pro on Swift 6.2, SwiftUI + AppKit, AVFoundation
- Legacy runtime: macOS 26+ on Apple Silicon
- Compatibility baseline: `specs/rust-rewrite/`

## Repository goals

- preserve the current editor's observable behavior in a testable spec
- port non-UI logic into pure Rust modules first
- use `gpui-ce` for windows, panes, input, shortcuts, and drag/drop
- make platform-specific behavior explicit behind adapters

## What is in the repo today

1. the Rust workspace that carries the active Fronda implementation
2. compatibility specs that define the user-visible contract Fronda must preserve or explicitly change
3. the legacy macOS-native Swift implementation kept as behavioral reference where parity is still being verified
4. fork-specific attribution, legal notices, and rewrite rules

## Rust workspace

```bash
cargo test --workspace
```

Check the `gpui-ce` desktop shell on a supported desktop toolchain:

```bash
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

Launch the desktop shell on a supported desktop toolchain:

```bash
cargo run -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

GitHub Actions runs spec validation, Rust workspace tests, a `gpui-ce` shell compile check, and legacy Swift baseline build/test on pushes to `main`, pull requests, and manual dispatch through `.github/workflows/ci.yml`.

## Legacy Swift baseline

Build:

```bash
swift build
swift run
```

For a bundled debug build that launches the `.app` and streams OSLog:

```bash
./scripts/dev.sh
```

Test:

```bash
swift test
```

## Rust compatibility spec baseline

Start here:

- `specs/rust-rewrite/README.md`
- `specs/rust-rewrite/10-current-status-and-plan.md`
- `specs/rust-rewrite/11-identifier-migration-plan.md`
- `specs/rust-rewrite/99-test-matrix.md`
- `AGENTS.md`

The rule of thumb for Fronda work is simple: preserve observable behavior first, improve architecture second. If behavior changes intentionally, update the relevant spec in the same change.

## Current Palmier compatibility identifiers

`Fronda` is the active Rust product name. Until identifier migration is made explicit, the repo still preserves inherited identifiers that remain part of the compatibility surface:

- Swift package / target / source paths still use `PalmierPro`
- project packages still use the `.palmier` extension
- MCP server name is still `palmier-pro`
- MCP resource URIs are still `palmier://...`
- auth callback scheme is still `palmier://callback`

So current MCP setup examples still use the legacy server name:

### Claude Code

```bash
claude mcp add --transport http palmier-pro http://127.0.0.1:19789/mcp
```

### Codex

```bash
codex mcp add palmier-pro --url http://127.0.0.1:19789/mcp
```

### Cursor

Add this to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "palmier-pro": {
      "type": "http",
      "url": "http://127.0.0.1:19789/mcp"
    }
  }
}
```

## Contributing

See `CONTRIBUTING.md`.

For Fronda work, prefer:

- Rust implementation work
- spec capture and spec updates
- compatibility-preserving refactors
- targeted legacy Swift fixes only when they protect or clarify the compatibility contract

Avoid expanding the legacy Swift app with large new features unless that is the explicit goal.

## Upstream source and license

- Primary Rust product: Fronda
- Original project: Palmier Pro
- Upstream repository: <https://github.com/palmier-io/palmier-pro>
- Fork repository: <https://github.com/TimLai666/fronda>
- Fork copyright: Copyright (C) 2026 TimLai666
- Upstream copyright: Copyright (C) 2026 Palmier, Inc.

This repository remains available under [GPLv3](LICENSE). See [NOTICE.md](NOTICE.md) for attribution and modification notices.
