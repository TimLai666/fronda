# FAQ

**What is Fronda?**

`Fronda` is the cross-platform Rust editor in this fork of Palmier Pro.

**What code runs today?**

The active development surface is the Rust workspace. The repo also retains the inherited Swift 6.2 / SwiftUI / AppKit app as a legacy compatibility reference.

**Why keep the Swift app around?**

It remains the behavioral reference for parity checks. Fronda is expected to match current user-visible behavior unless a spec records an intentional product change.

**What platforms will Fronda target?**

Fronda is a cross-platform Rust app, with `gpui-ce` as the default UI stack.

**What platforms does the current app support?**

The legacy Swift app is macOS 26+ on Apple Silicon only. The Fronda workspace itself is cross-platform and is the primary target for repo work.

**Why do some identifiers still say Palmier?**

Because the current compatibility surface still includes upstream identifiers such as `PalmierPro`, `.palmier`, and `palmier://`. Those will only be renamed as an explicit migration, not incidentally. The MCP server name has already migrated to `fronda` as the first such spec-backed rename.

**Is this repo fully open source?**

This repository remains available under GPLv3, consistent with upstream. Fork-specific attribution and modification notices live in `NOTICE.md`.

**What is not in this repo?**

The upstream server-side generative processing is not included here. The rewrite specs describe the client-side observable contract only.

**Where are the rewrite specs?**

See `specs/rust-rewrite/`.
