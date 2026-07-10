# 98 — UI parity audit (Swift ↔ Rust), 2026-07-05

Full-surface structural/interaction audit of every Swift view vs its gpui
counterpart (agent sweep, verified file-by-file). Drives the /goal "UI 要跟
Swift 版完全一模一樣、真的可以使用". Pixel-level (spacing/color/type) diffs
are NOT covered here — structural/interaction parity first.

Systemic headline: the AI generation/edit surface (rows 1,3,4,5,11) renders a
plausible shell but is functionally hollow; drag-and-drop (row 6) and
right-click context menus (row 7) are absent as whole interaction layers.

## Ranked gaps

| # | Surface | Swift | Rust | Status | Size | Spectra change |
|---|---------|-------|------|--------|------|----------------|
| 1 | AI Generation panel | Generation/UI/GenerationView.swift (1884L) | generation_view.rs (2979L) | FUNCTIONAL (2026-07-10): model picker from model_catalog (catalog order, paid-gating badge), caps-driven settings popover (duration/aspect/resolution/quality/count + instrumental/generate-audio toggles, Esc/outside-click close), USD cost estimate (Fal-era CostEstimator @ 9dfde8d^) + insufficient-credits gating, reference tiles click-to-pick from manifest w/ thumbnails+clear+per-model caps (audio: no tiles), voice picker + lyrics/style TextAreas, Generate → shared-executor generate_* (no backend → explicit "unavailable" status, is_generating derived from manifest generationStatus). Residual gaps vs Swift: @mention autocomplete (own change), drag-drop into tiles (drag-drop-system), edit-video source strip (shows explicit "not available yet"), credits are a placeholder value (no account backend), settings pickers render as wrapped pills (not segmented/grid split), prompt placeholder not caps-driven, ref cards lack @Image1 tag chips, panel resize handle inert | XL | generation-panel-functional |
| 2 | Media tab library | MediaPanel/MediaTab/*.swift | media_panel_view.rs | PARTIAL→STUB: search is a static label; no View/Sort/Filter menus; NO folders (tiles/breadcrumb/new/rename); no drag-drop; no context menus; no marquee select; no thumb-size presets; no overflow menu; no banners/count/index-status; empty-state dead code | XL | (to open) |
| 3 | Captions tab | CaptionsTab/CaptionTab.swift (410L) | media_panel_view.rs captions_tab_content | STUB: all rows static; missing Source/Language/Font/Size/Color/Background/Case/Censor controls, live preview box w/ guides, X/Y scrub, Agent Mode menu, Generate gating, transcribing overlay | L | (to open) |
| 4 | Music tab | MediaPanel/MusicTab.swift (330L) | media_panel_view.rs music_tab_content | STUB: static labels; missing input-mode menu, duration scrub, model menu (real AudioModelConfig), prompt field, cost note, credit gating, generating overlay, Agent Mode menu | L | (to open) |
| 5 | Inspector Text tab | Inspector/TextTab.swift + ColorField/FontPickerField | none | MISSING: Content field, font picker, size, opacity, color, alignment segmented, background, shadow, stroke, position; no ColorField/FontPickerField components exist | L | (to open) |
| 6 | External drag-drop (systemic) | MediaTab+Drag, MediaPanelDropArea, DropZoneView, ProjectCard, AgentInputBox | only internal clip drag | MISSING: Finder→panel, asset→timeline/gen-slots, drop highlight states | L/XL | (to open) |
| 7 | Right-click context menus (systemic) | ProjectCard, MediaTab+Grids, AssetThumbnailView, FolderTileView | none anywhere | MISSING (Swift timeline has none either — not a gap there) | M/L | (to open) |
| 8 | Inspector Crop & Flip | InspectorView cropRow/flipRow | prop_row statics | PARTIAL: no crop toggle/aspect menu/keyframes, no flip H/V toggles | M | (to open) |
| 9 | Inspector Source metadata | InspectorView assetDetailsContent | source_media_content | PARTIAL: hardcoded values; no AI badge, references strip, Generated section, Prompt+copy, real File section | M | (to open) |
| 10 | Home cards/samples/sidebar | HomeView, ProjectCard, SampleProjectsStrip, WelcomeOverlay | app_root render_home | PARTIAL: no hover states/trash+confirm/context menu/file-missing overlay; samples hardcoded (no service/posters/progress); Open Project no file panel; no IdentityStrip | M | (to open) |
| 11 | AI Edit tab | Inspector/AIEditTab.swift (433L) | ai_edit_tab_view.rs (414L) | PARTIAL (uncertain): structure present, actions likely not dispatching real edits — verify | M | (to open) |
| 12 | Preview settings menus + capture frame | PreviewContainerView.swift (926L) | preview_view.rs (915L) | PARTIAL: aspect/fps/resolution/zoom menus + Capture Frame button missing/unverified | M | (to open) |
| 13 | Inspector numeric binding | InspectorView scrub rows | inspector_view.rs scrub_values | PARTIAL (uncertain): values from default HashMap not selected clip — verify hub binding; no per-section reset | M | (to open) |
| 14 | Tour spotlight | Editor/Tour/TourOverlay.swift | tour_overlay_view.rs | PARTIAL: spotlight cutout/anchor highlight not implemented (file self-documents) | M | (to open) |
| 15 | Welcome overlay | Project/WelcomeOverlay.swift | inline app_root | PARTIAL: simplified; diff title/imagery/animation | S | (to open) |
| 16 | Toolbar Add-Text button | Toolbar/ToolbarView.swift | toolbar_view.rs | PARTIAL: serif "T" button absent | S | (to open) |

## DONE (structural, pending pixel diff)

Agent chat panel (verify error-banner CTAs, scroll-to-bottom, BYOK label);
Settings window (verify Models pane fidelity); Export dialog; Help window;
Feedback; Account popover; Title bar; App menu; Preview transport core;
Timeline; Keyframes; Crop/Transform overlays; Update overlay;
Settings-mismatch; Project activity; Mention popover; Chat history.

## Flagged uncertainties (second pass needed)

Rows 11/12/13 inferred from patterns — confirm by reading render bodies.
Toolbar undo/redo wiring; Settings Models/Storage pane fidelity. Pixel-level
parity (spacing/colors/typography) not yet audited for DONE views.
