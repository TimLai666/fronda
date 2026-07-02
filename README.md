<div align="center">

# Fronda

**Cross-platform video editor written in Rust, driven by AI agents over MCP.**

</div>

<img src="./assets/palmier-ui.png" alt="Fronda UI, matching the Palmier Pro baseline" width="900" />

Fronda is a cross-platform Rust rewrite of [Palmier Pro](https://github.com/palmier-io/palmier-pro), built on `gpui-ce`. It runs on macOS, Windows, and Linux. The inherited Swift app is kept in this repository only as a legacy behavioral reference while parity is verified — **the Rust workspace is the primary codebase.**

## Status

| Area | State |
| --- | --- |
| Core logic (timeline math, project I/O, media library, search, generation, export planning) | Ported to pure Rust crates under `crates/`, covered by the workspace test suite |
| Desktop shell | `gpui-ce` app shell (`crates/app_shell_gpui`), cross-platform |
| MCP server | Protocol implementation ported to Rust (`crates/mcp_server`); not yet started by the Rust desktop shell — see [Connecting via MCP](#connecting-via-mcp) |
| Legacy Swift app | Behavioral reference only, macOS 26+ / Apple Silicon |

The compatibility contract lives in `specs/rust-rewrite/`. The rule of thumb: preserve observable behavior first, improve architecture second. Intentional behavior changes must update the relevant spec in the same change.

## Build and run

Requires a recent stable Rust toolchain.

Run the test suite:

```bash
cargo test --workspace
```

Type-check the desktop shell:

```bash
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

Launch the desktop app:

```bash
cargo run -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

CI (`.github/workflows/ci.yml`) runs spec validation, workspace `fmt`/`clippy`/tests, a `gpui-ce` shell compile check, and the legacy Swift baseline build/test.

## Connecting via MCP

Fronda exposes its editing tools to AI agents (Claude Code, Codex, Cursor, and any other MCP client) through a local MCP server.

**Current state:**

- The MCP server implementation is ported to Rust in `crates/mcp_server` (`fronda-mcp-server`): HTTP JSON-RPC on `127.0.0.1:19789`, loopback-only by default, optional bearer-token auth for network exposure.
- The Rust desktop shell does **not** yet start this server at runtime — wiring it into the app shell is in progress. Until then, a live MCP connection requires running the legacy Swift app on macOS.
- The tool names, schemas, and resource URIs are part of the preserved compatibility contract, so client configuration is identical for both implementations.

For compatibility, the server still identifies as `palmier-pro` and resources still use `palmier://` URIs (see [Compatibility identifiers](#compatibility-identifiers)).

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

## Repository layout

- `crates/` — the Rust workspace: `core_model`, `timeline_core`, `project_io`, `media_library`, `render_core`, `search_core`, `generation_core`, `agent_contract`, `mcp_server`, `app_contract`, `app_shell_gpui`, and more
- `specs/rust-rewrite/` — the compatibility spec baseline; start with `specs/rust-rewrite/README.md` and `10-current-status-and-plan.md`
- `openspec/` — Spectra spec-driven-development specs and change proposals
- `Sources/`, `Tests/` — the legacy Swift implementation (behavioral reference)
- `AGENTS.md` — repo rules for AI-assisted work

## Compatibility identifiers

Fronda preserves inherited identifiers until an explicit, spec-backed migration:

- project packages use the `.palmier` extension (`project.json`, `media.json`, `generation-log.json`, `chat/*.json` inside)
- MCP server name is `palmier-pro`
- MCP resource URIs are `palmier://...`
- auth callback scheme is `palmier://callback`
- Swift package / target / source paths still use `PalmierPro`

Do not rename these piecemeal.

## Legacy Swift baseline

Kept as the behavioral reference on macOS 26+ / Apple Silicon:

```bash
swift build
swift run    # or ./scripts/dev.sh for a bundled debug build with OSLog
swift test
```

Avoid adding large new features to the Swift app; prefer Rust implementation work, spec capture, and compatibility fixes.

## Contributing

See `CONTRIBUTING.md`. For Fronda work, start from the Rust workspace and prefer:

- Rust implementation work
- spec capture and spec updates
- compatibility-preserving refactors
- targeted legacy Swift fixes only when they protect or clarify the compatibility contract

## Upstream source and license

- Primary Rust product: Fronda
- Original project: Palmier Pro
- Upstream repository: <https://github.com/palmier-io/palmier-pro>
- Fork repository: <https://github.com/TimLai666/fronda>
- Fork copyright: Copyright (C) 2026 TimLai666
- Upstream copyright: Copyright (C) 2026 Palmier, Inc.

This repository remains available under [GPLv3](LICENSE). See [NOTICE.md](NOTICE.md) for attribution and modification notices.
