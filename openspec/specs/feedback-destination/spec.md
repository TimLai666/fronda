# feedback-destination Specification

## Purpose

TBD - created by archiving change 'feedback-github-link'. Update Purpose after archive.

## Requirements

### Requirement: Send Feedback opens the GitHub issues page

The app's Send Feedback command SHALL open Fronda's GitHub issues page in the
OS default browser. The destination URL SHALL be the single shared constant
`FEEDBACK_ISSUES_URL`. Browser launch SHALL be best-effort (a spawn failure is
ignored, not surfaced as a crash).

#### Scenario: menu action opens the issues URL

- **WHEN** the user triggers Send Feedback
- **THEN** the app SHALL launch the OS default browser at `FEEDBACK_ISSUES_URL`

#### Scenario: per-platform launcher

- **WHEN** resolving the browser launch argv
- **THEN** Windows SHALL use `explorer <url>`, macOS `open <url>`, and other systems `xdg-open <url>`


<!-- @trace
source: feedback-github-link
updated: 2026-07-11
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/platform_adapter.rs
-->

---
### Requirement: send_feedback tool directs to GitHub when no backend is connected

With no `FeedbackSender` installed (Fronda runs no feedback backend),
`send_feedback` SHALL succeed and return guidance pointing to
`FEEDBACK_ISSUES_URL` rather than returning an unavailable error. Returning
guidance SHALL NOT consume the session dedup or cap budget (nothing is sent).
When a host installs a `FeedbackSender`, the tool SHALL still deliver through it
(dedup, cap, and diagnostics unchanged).

#### Scenario: no backend returns the issues URL

- **WHEN** `send_feedback` is called with a message and no sender is installed
- **THEN** it SHALL return a success result whose text contains `FEEDBACK_ISSUES_URL`

#### Scenario: guidance is idempotent

- **WHEN** `send_feedback` is called twice with the same message and no sender
- **THEN** both calls SHALL return the guidance (no duplicate rejection, no cap consumed)

#### Scenario: installed sender still delivers

- **WHEN** a `FeedbackSender` is installed and `send_feedback` is called
- **THEN** the payload (message + app version + timeline summary) SHALL be delivered through the sender, with dedup and the per-session cap enforced

<!-- @trace
source: feedback-github-link
updated: 2026-07-11
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/platform_adapter.rs
-->