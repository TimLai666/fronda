## 1. Browser launcher

- [x] 1.1 `open_url_argv(url, os)` yields `explorer`/`open`/`xdg-open` per platform; `open_url` spawns best-effort. Verified by `open_url_argv_per_platform`.

## 2. Menu wiring

- [x] 2.1 Send Feedback opens `FEEDBACK_ISSUES_URL` in the browser (was an empty handler). Verified by compile of the desktop-app `fronda` bin (the arm calls `platform_adapter::open_url(agent_contract::FEEDBACK_ISSUES_URL)`).

## 3. Agent tool

- [x] 3.1 `FEEDBACK_ISSUES_URL` is a single shared public const in `agent_contract`. Verified by re-export compile + use from both the tool and the menu.
- [x] 3.2 `send_feedback` with no sender returns GitHub-issues guidance (not an error), idempotently, without consuming dedup/cap. Verified by `send_feedback_without_sender_points_to_github_issues`.
- [x] 3.3 Installed `FeedbackSender` still delivers payload with dedup + cap. Verified by the existing `send_feedback_delivers_payload_with_diagnostics` / `_rejects_duplicate_message` / `_caps_at_eight_successful_sends` / `_failed_send_not_recorded` (all still green).

## 4. Gates

- [x] 4.1 `cargo test --workspace` exit 0.
- [x] 4.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (420 passed).
- [x] 4.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.
