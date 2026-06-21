# FAQ

**What is Fronda?**

`Fronda` is the planned name of the cross-platform Rust rewrite being developed in this fork of Palmier Pro.

**What code runs today?**

Today the runnable app is still the inherited Swift 6.2 / SwiftUI / AppKit implementation from the Palmier Pro codebase.

**Why keep the Swift app around?**

It is the behavioral reference. The Rust rewrite is expected to match current user-visible behavior unless a spec records an intentional product change.

**What platforms will Fronda target?**

The rewrite target is a cross-platform Rust app, with `gpui-ce` as the default UI stack.

**What platforms does the current app support?**

The current executable is still macOS 26+ on Apple Silicon only.

**Why do some identifiers still say Palmier?**

Because the current compatibility surface still includes upstream identifiers such as `PalmierPro`, `.palmier`, `palmier://`, and the MCP server name `palmier-pro`. Those will only be renamed as an explicit migration, not incidentally.

**Is this repo fully open source?**

This repository remains available under GPLv3, consistent with upstream. Fork-specific attribution and modification notices live in `NOTICE.md`.

**What is not in this repo?**

The upstream server-side generative processing is not included here. The rewrite specs describe the client-side observable contract only.

**Where are the rewrite specs?**

See `specs/rust-rewrite/`.
