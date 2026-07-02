# Fronda Identifier Migration Plan

Last reviewed: 2026-06-29

## Goal

Make the repo read and behave as a Fronda-first codebase without breaking compatibility surfaces that still depend on inherited Palmier identifiers.

## Current identifier classes

### Repo and product-facing

These should already be treated as Fronda-first:

- repo name: `fronda`
- Rust binary name: `fronda`
- Rust workspace crate naming
- user-facing repo documentation

### Compatibility-sensitive runtime identifiers

These still intentionally preserve Palmier heritage:

- Swift package / target name: `PalmierPro`
- source path namespace: `Sources/PalmierPro`
- legacy executable name: `PalmierPro`
- bundle identifier: `io.palmier.pro`
- project extension: `.palmier`
- UTI: `io.palmier.project`
- URL scheme: `palmier://`
- MCP server name: ~~`palmier-pro`~~ → **migrated to `fronda`** (change `wire-mcp-server-into-rust-shell`, 2026-07; Rust MCP server had no live consumers, so the rename shipped with the shell wiring)
- Claude Desktop bundle: `palmier-pro.mcpb`
- Sparkle feed metadata in `appcast.xml`

## Decision rule

Do **not** rename compatibility-sensitive identifiers piecemeal.

A rename is only safe when all of the following are true:

1. the affected surface is listed explicitly
2. migration behavior is specified for existing users/projects
3. docs, tests, fixtures, and runtime adapters move together
4. the change is called out as an intentional compatibility break or compatibility-preserving migration

## Hold list

These values should stay unchanged for now:

### `appcast.xml`

Keep these Palmier values for now:

- channel title
- feed URL
- release enclosure URLs
- updater-facing package naming

Reason:

- they are part of the legacy Swift updater contract
- changing them without a release migration path would be speculative and potentially breaking

### `.palmier` package naming

Keep `.palmier` until Fronda has:

1. stable import/export compatibility tests
2. a migration story for existing projects
3. a clear decision on whether the extension change is worth the churn

### MCP identifiers

The MCP server *name* migrated to `fronda` (see above). Keep `palmier://` resource URIs until:

1. clients can discover the new name safely
2. existing integrations have a migration path
3. docs, server output, and test snapshots are updated together

## Recommended migration order

If identifier migration is approved later, do it in this order:

1. repo/docs/product copy
2. optional aliases in tooling and MCP docs
3. runtime dual-name support where possible
4. fixture/test updates
5. default-name flip
6. legacy-name retirement only after a compatibility window

## Immediate repo policy

Effective now:

1. documentation should refer to the product as `Fronda`
2. compatibility identifiers should be described as inherited surfaces, not as the primary product name
3. inherited identifiers should remain unchanged in code and metadata unless a migration change explicitly targets them
