<div align="center">

# Fronda

**Planned name of the cross-platform Rust successor to Palmier Pro.**

</div>

<img src="./assets/palmier-ui.png" alt="Palmier Pro Swift compatibility baseline UI" width="900" />

This repository is a modified fork of [Palmier Pro](https://github.com/palmier-io/palmier-pro). The current runnable application is still the inherited Swift/AppKit codebase; `Fronda` is the intended name of the Rust rewrite being developed in this fork.

## Current status

- Current runnable baseline: Palmier Pro on Swift 6.2, SwiftUI + AppKit, AVFoundation
- Current supported runtime: macOS 26+ on Apple Silicon
- Rust rewrite name: `Fronda`
- Rewrite target stack: Rust + `gpui-ce`
- Compatibility baseline: `specs/rust-rewrite/`

## Repository goals

- preserve the current editor's observable behavior in a testable spec
- port non-UI logic into pure Rust modules first
- use `gpui-ce` for windows, panes, input, shortcuts, and drag/drop
- make platform-specific behavior explicit behind adapters

## What is in the repo today

1. the original macOS-native Swift implementation that still defines current behavior
2. rewrite specs that list the behaviors the Rust port must satisfy with automated tests
3. fork-specific attribution, legal notices, and rewrite rules

## Build the current Swift baseline

```bash
swift build
swift run
```

For a bundled debug build that launches the `.app` and streams OSLog:

```bash
./scripts/dev.sh
```

## Test the current Swift baseline

```bash
swift test
```

GitHub Actions runs `swift build` and `swift test` on pushes to `main`, pull requests, and manual dispatch through `.github/workflows/ci.yml`.

## Rust rewrite spec baseline

Start here:

- `specs/rust-rewrite/README.md`
- `specs/rust-rewrite/99-test-matrix.md`
- `AGENTS.md`

The rule of thumb for rewrite work is simple: preserve observable behavior first, improve architecture second. If behavior changes intentionally, update the relevant spec in the same change.

## Current Palmier compatibility identifiers

`Fronda` is the Rust version name. Until identifier migration is made explicit, the current Swift baseline still exposes inherited identifiers that are part of the compatibility surface:

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

For rewrite work, prefer:

- bug fixes to the Swift baseline
- spec capture
- migration scaffolding
- compatibility-preserving refactors

Avoid expanding the legacy Swift app with large new features unless that is the explicit goal.

## Upstream source and license

- Rust rewrite name: Fronda
- Original project: Palmier Pro
- Upstream repository: <https://github.com/palmier-io/palmier-pro>
- Fork repository: <https://github.com/TimLai666/fronda>
- Fork copyright: Copyright (C) 2026 TimLai666
- Upstream copyright: Copyright (C) 2026 Palmier, Inc.

This repository remains available under [GPLv3](LICENSE). See [NOTICE.md](NOTICE.md) for attribution and modification notices.
