# 98 — UI parity audit (Swift ↔ Rust), 2026-07-05

Full-surface structural/interaction audit of every Swift view vs its gpui
counterpart (agent sweep, verified file-by-file). Drives the /goal "UI 要跟
Swift 版完全一模一樣、真的可以使用". Pixel-level (spacing/color/type) diffs
are NOT covered here — structural/interaction parity first.

Systemic headline: the AI generation/edit surface (rows 1,3,4,5,11) renders a
plausible shell but is functionally hollow — rows 1/3/4 became functional in
2026-07-10 changes (backend/host seams still gated); drag-and-drop (row 6) and
right-click context menus (row 7) are absent as whole interaction layers.

## Ranked gaps

| # | Surface | Swift | Rust | Status | Size | Spectra change |
|---|---------|-------|------|--------|------|----------------|
| 1 | AI Generation panel | Generation/UI/GenerationView.swift (1884L) | generation_view.rs (2979L) | FUNCTIONAL (2026-07-10): model picker from model_catalog (catalog order, paid-gating badge), caps-driven settings popover (duration/aspect/resolution/quality/count + instrumental/generate-audio toggles, Esc/outside-click close), USD cost estimate (Fal-era CostEstimator @ 9dfde8d^) + insufficient-credits gating, reference tiles click-to-pick from manifest w/ thumbnails+clear+per-model caps (audio: no tiles), voice picker + lyrics/style TextAreas, Generate → shared-executor generate_* (no backend → explicit "unavailable" status, is_generating derived from manifest generationStatus). Residual gaps vs Swift: @mention autocomplete (own change), drag-drop into tiles (drag-drop-system), edit-video source strip (shows explicit "not available yet"), credits are a placeholder value (no account backend), settings pickers render as wrapped pills (not segmented/grid split), prompt placeholder not caps-driven, ref cards lack @Image1 tag chips, panel resize handle inert | XL | generation-panel-functional |
| 2 | Media tab library | MediaPanel/MediaTab/*.swift | media_panel_view.rs | DONE (2026-07-10): live search TextField + clear X; folder tiles/double-click drill-in/breadcrumb/New Folder/inline rename (rename_folder); View (Folders/Flat/Grouped) + Sort (name/date/duration/type) + Filter (type/AI/clear) menus; ctrl+shift multi-select + batch delete; item-count + index-status + wired empty state; 26 pure tests. Remaining (other changes/polish): drag-drop (row 6), context menus (row 7), marquee, thumb-size presets, swap/toast banners, moment search (needs index host) | XL | media-library-complete |
| 3 | Captions tab | CaptionsTab/CaptionTab.swift (410L) | media_panel_view.rs render_captions_tab | FUNCTIONAL (2026-07-10): Source menu (Auto/selection/per-track "V1 · n clips" from live timeline), Language menu (curated BCP-47 stand-in list, seeds from transcription_language, feeds add_captions `language`), Font picker (bundled render_core families only), Size/X/Y scrub rows (inspector drag pattern, centre snap 0.02), color+background swatch strips w/ toggle, Case (Auto/UPPERCASE/lowercase), Censor toggle, live preview (canvas aspect, 1080-ref scaling, centre guides), Generate → add_captions (no speech engine → explicit "Transcription unavailable" note, overlay only on a real queued run), Agent Mode menu (3 tasks + translate submenu, Swift prompt text) via set_agent_chat_handoff seam. Residual vs Swift: chat handoff seam not installed by app_root yet (items note it), color well is preset swatches (shared ColorField = row 5), locale list is static (no transcription host), aiGradient pill approximated with accent, in-menu/preview font faces need gpui font registration | L | captions-music-tabs |
| 4 | Music tab | MediaPanel/MusicTab.swift (330L) | media_panel_view.rs render_music_tab | FUNCTIONAL (2026-07-10): Input menu (Video↔Text to Music; Rust catalog has no `inputs` metadata so text mode always offered), Model menu (catalog AudioCategory::Music entries), Duration scrub 1..600s (text mode) / whole-timeline span summary "Whole timeline · 0:00 – m:ss · Xs" (video mode; no marked in/out ranges in Rust yet), prompt TextArea (IME), USD cost estimate (model_catalog::audio_cost) in the Generate label, validation notes in Swift order (model/prompt/span 1..900s/credit shortfall vs GenerationState credits), Generate → generate_music (no backend → explicit "Generation unavailable" note; overlay only while the manifest has an in-flight entry), Agent Mode menu (timeline task + 5-mood submenu). Residual vs Swift: chat handoff seam uninstalled (notes it), credits placeholder (no account backend), min/maxSeconds + promptLabel not in the Rust catalog (defaults 1/900, "Prompt"), text-mode placement frame (playhead/marked-range start) not sent — generate_music schema has no placement arg | L | captions-music-tabs |
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
