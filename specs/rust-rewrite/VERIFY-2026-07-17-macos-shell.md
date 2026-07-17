# macOS interactive verification — editor-shell-parity task 5.2

Date: 2026-07-17. Machine: macOS (Darwin 25.5.0, arm64, 1470×956 pt display).
Binary: `cargo build -p fronda-app-shell-gpui --features desktop-app --bin fronda`
(debug). Driven via System Events (AX) + CGEvent synthesis; pixel assertions
read from `screencapture` output (values below are the red channel of sRGB
pixels; BASE ≈ 10, SURFACE ≈ 22, RAISED ≈ 30, SUBTLE border ≈ 40).

## Gate commands

- `cargo test --workspace` — **2916 passed / 0 failed** (run twice: before and
  after the session's fixes).
- `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` — pass.
- Desktop-gated suites: `menu` 28 tests, `native_menu` 5, `assets` 2 — green.

## Checklist results

- **(a) Text visible** — PASS. Full UI text renders (menus, panels, inspector
  rows, chat, generation panel). The `desktop-app = [..., "gpui_platform/font-kit"]`
  feature fix is pinned by `desktop_app_enables_macos_font_backend`.
- **(b) Native menu bar** — PASS. System Events reports menus
  `Apple, fronda, File, Edit, View, Help`. File sections mirror Swift
  `MainMenu.swift` (New/Open ｜ Save/Save As ｜ Import Media/Import Timeline ｜
  Export); Edit/View/Help section breaks match; View contains a `Layout`
  submenu with `Default / Media / Vertical`. Key equivalents read back from
  the keymap: ⌘N (New Project), ⇧⌘I (Import Timeline), ⌘1 (Layout Default),
  ⌘C (Copy, `!input` display binding). Notes: the app-menu title shows the
  process name (`fronda`, lowercase — a bundle/CFBundleName concern, not a
  menu-model one); macOS auto-appends Writing Tools/AutoFill/Dictation/Emoji
  to Edit and a second "Enter Full Screen" to View (ours dispatches
  RunMenuAction, so AppKit does not dedupe; the system item is the working
  one — EnterFullScreen remains a no-op arm on all platforms).
- **(c) Layout switching via View menu** — PASS. Clicking Layout → Vertical
  rearranged to left (Media｜Inspector / Toolbar+Timeline) + right full-height
  Preview; Media/Default equally verified. Each selection logged exactly one
  `dispatch_menu_action` (temporary debug build).
- **(d) Command shortcuts, no double dispatch** — PASS. ⌘2 → Media, ⌘1 →
  Default, structurally distinct screenshots; one dispatch per chord (log
  showed one line per press; `performKeyEquivalent` consumes the chord before
  the menu sees it).
- **(e) Shell visuals** — PASS.
  - Panel seams: BASE gaps between surface cards measurable in pixel scans.
  - Divider drag (MediaWidth divider): hitbox at the seam (press at seam
    center +2pt), drag left −100 pt moved the seam exactly −100 pt and
    followed the pointer; drag right +80 pt clamped media back to the
    preview-min-400 bound (initial sizes at a 1360 pt window leave preview
    < 400, so the first drag snaps to the legal maximum — clamp working as
    specified). Remaining dividers share the same target enum + pure-tested
    clamp (`pane_resize`).
  - Welcome overlay at 1280×720: card centered, Skip / Watch Tutorial /
    Get started fully inside the window.
  - Media rail: three SVG icons visible (folder active-white, captions,
    music note) — after the assets fix below.
  - Preview empty state: centered 16:9 canvas rect measurable as
    border(40)/BASE(10) bands inside the SURFACE(22) pane — after the canvas
    fix below.
- **(f) Swift baseline comparison** — see the addendum at the bottom.

## Defects found and fixed during this pass

1. **Nothing focused at boot** (`app_root.rs::open_main_window`): gpui
   resolves keystroke dispatch and `is_action_available` along the focus
   path; with no focus, every native menu item validated to disabled
   (System Events: `enabled=false`) and no shortcut or key_down ever reached
   the root. Fix: `window.focus(&root.focus_handle, cx)` in the open_window
   closure. After: `enabled=true`, menu clicks and ⌘ chords dispatch once.
2. **Empty-state canvas hidden by transparent composite**
   (`preview_view.rs`): an empty timeline composites to a fully transparent
   PNG, so `frame_png` is `Some` and the None-branch canvas never drew.
   Fix: always paint the timeline-aspect canvas bounds under the frame,
   scaled by canvas_zoom.
3. **Four SVGs missing from the embedded assets** (`assets.rs`):
   captions/music_note (media rail) and eye_slash/speaker_slash (timeline
   badges) existed on disk but not in the explicit `include_bytes!` list —
   `svg().path()` silently rendered nothing on every platform. Fix: embed
   them + regression test `every_icon_on_disk_is_embedded`.

## Swift baseline comparison (f)

`swift build` completed (cached download resumed), `swift run` launched
Palmier Pro (debug, macOS 26.5 SDK). Side-by-side structural comparison,
same day, same machine:

- **Menu bar**: Swift shows `Apple, PalmierPro, File, Edit, View, Help`;
  Fronda shows `Apple, fronda, File, Edit, View, Help`. Same five groups,
  same Layout submenu (`Default, Media, Vertical`) under View. No Window
  menu on either side (confirms the spec fix — EDT-009 previously claimed
  a Window menu that never existed).
- **Welcome overlay**: both center a card with hero area and
  Skip / Watch Tutorial / Get started. (Swift hero shows sample artwork;
  Fronda shows the placeholder gradient — content, not layout.)
- **Default**: both — upper Media | Preview | Inspector, lower
  Toolbar + Timeline spanning the full preset width.
- **Media**: both — Media column full-height left (~30%), right region
  split into (Preview | Inspector) above and Toolbar + Timeline below.
- **Vertical**: both — left region (Media | Inspector above,
  Toolbar + Timeline below), right column full-height Preview.

Cosmetic deviations noted (not structural):
- App-menu title: `fronda` (process name; needs an app bundle with
  CFBundleName "Fronda" to display properly — release-packaging concern).
- Media rail third icon: Fronda uses a music-note glyph, the Swift build
  renders a waveform-style glyph.
- Fronda draws a SUBTLE border around the empty preview canvas so bounds
  are visible on dark surface; Swift's empty canvas is borderless at zoom 1
  (Swift borders it only at zoom < 1) — documented design decision.
- Agent panel visibility at startup differed in this run (Swift launched
  with agent hidden — visibility persists across launches in Swift
  (EDT-003), which Fronda defers; preset structure itself is unaffected).

Artifacts from this session: `~/Movies/Untitled.palmier` (Fronda dev-seam
project) and `~/Documents/Palmier Pro/Untitled Project.palmier` (Swift
baseline project) were created during verification and can be deleted.

## D9 addendum — evening interactive pass (same day)

After the decision-execution waves (D1–D5, D7, D8, D11, #294–#342 ports),
a second interactive pass on macOS verified the day's UI landings live:

- Native menu bar regression: `Apple, fronda, File, Edit, View, Help` intact.
- #327 panel groups: inspector renders Project/Settings as collapsible
  EditorPanelGroups (88pt label column); clicking the Settings header
  collapses it (chevron + content) while Project stays open.
- D8 focus ring: clicking inside a pane moves the accent ring to that
  pane's card (observed on the inspector after an in-pane click).
- Media rail three icons, centered preview empty-state canvas, timeline
  tab bar + toolbar all render correctly.
- Text rendering (font-kit) regression: clean.

Not covered tonight (needs human/hardware): physical audio-out for the
new cpal playback engine (seek pops, meter timing), whisper real-model
inference (no model file on this machine), multicam chip operations and
export-queue interactions beyond compile/model tests.
