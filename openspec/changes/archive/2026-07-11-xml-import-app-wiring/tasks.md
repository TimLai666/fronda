## 1. Executor support

- [x] 1.1 `ToolExecutor::adopt_timeline` swaps in an external timeline as active, keeps the prior one as a sibling, assigns a fresh id when missing, clears undo, and bumps the revision. Verified by `adopt_timeline_switches_active_and_keeps_prev_as_sibling`.

## 2. Import pipeline (pure, executor-testable)

- [x] 2.1 `import_timeline_from_xml` parses via `import_xml`, relinks media, remaps `media_ref`, and adopts a new active timeline while preserving the current one. Verified by `import_adds_new_active_timeline_without_dropping_current`.
- [x] 2.2 Media relink matches existing library entries by filename, registers unmatched paths, and remaps clips keyed by both file id (XMEML) and filename (FCPXML). Verified by `import_relinks_existing_media_and_registers_missing` (relinked=1, registered=1, both clips resolve).
- [x] 2.3 `strip_file_url` handles `file://` / `file://localhost` / Windows `file:///C:/…`; `detect_format` prefers content over extension. Verified by `strip_file_url_forms` and `detect_format_prefers_content_then_extension`.

## 3. App wiring

- [x] 3.1 File menu + ⌘⇧I shortcut expose Import Timeline distinct from Import Media. Verified by `menu_003_file_menu_items` and `menu_008_shortcuts_count` (48).
- [x] 3.2 `perform_menu_action(ImportTimeline)` opens a file picker and `import_timeline_at` imports + refreshes the timeline view. Verified by compile of the desktop-app `fronda` bin (gpui glue).
- [x] 3.3 Help view lists the Import Timeline shortcut. Verified by content review of `help_view`.

## 4. Gates

- [x] 4.1 `cargo test --workspace` exit 0.
- [x] 4.2 `cargo test -p fronda-app-shell-gpui --features desktop-app` exit 0 (419 passed).
- [x] 4.3 `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` exit 0, zero warnings.
