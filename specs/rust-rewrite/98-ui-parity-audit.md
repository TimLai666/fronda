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
| 2 | Media tab library | MediaPanel/MediaTab/*.swift | media_panel_view.rs | DONE (2026-07-10): live search TextField + clear X; folder tiles/double-click drill-in/breadcrumb/New Folder/inline rename (rename_folder); View (Folders/Flat/Grouped) + Sort (name/date/duration/type) + Filter (type/AI/clear) menus; ctrl+shift multi-select + batch delete; item-count + index-status + wired empty state; 26 pure tests. Remaining (other changes/polish): drag-drop (row 6), context menus (row 7), marquee, thumb-size presets, swap/toast banners, moment search (needs index host) | XL | media-library-complete |
| 3 | Captions tab | CaptionsTab/CaptionTab.swift (410L) | media_panel_view.rs captions_tab_content | STUB: all rows static; missing Source/Language/Font/Size/Color/Background/Case/Censor controls, live preview box w/ guides, X/Y scrub, Agent Mode menu, Generate gating, transcribing overlay | L | (to open) |
| 4 | Music tab | MediaPanel/MusicTab.swift (330L) | media_panel_view.rs music_tab_content | STUB: static labels; missing input-mode menu, duration scrub, model menu (real AudioModelConfig), prompt field, cost note, credit gating, generating overlay, Agent Mode menu | L | (to open) |
| 5 | Inspector Text tab | Inspector/TextTab.swift + ColorField/FontPickerField | inspector_view.rs text_tab_content + field_components.rs | DONE (structural, 2026-07-10 inspector-completion): Content (TextArea), FontPickerField (bundled families), size/opacity scrub, ColorField (swatch+hex), alignment segmented, background/border toggle+color, position — all write back via update_text/set_clip_properties. Remaining: shadow editing (no tool path — needs a set_clip_properties `shadow` fill, agent_contract), system-font list, live-preview-during-scrub | L | inspector-completion |
| 6 | External drag-drop (systemic) | MediaTab+Drag, MediaPanelDropArea, DropZoneView, ProjectCard, AgentInputBox | only internal clip drag | MISSING: Finder→panel, asset→timeline/gen-slots, drop highlight states | L/XL | (to open) |
| 7 | Right-click context menus (systemic) | ProjectCard, MediaTab+Grids, AssetThumbnailView, FolderTileView | none anywhere | MISSING (Swift timeline has none either — not a gap there) | M/L | (to open) |
| 8 | Inspector Crop & Flip | InspectorView cropRow/flipRow | inspector_view.rs crop_row/flip_row | MOSTLY DONE (2026-07-10 inspector-completion): flip H/V toggles write transform.flipHorizontal/Vertical; crop toggle drives crop_editing_active (preview relay = app-shell wiring) + aspect menu (8 Swift presets, view state). Remaining: aspect-preset crop COMMIT (no static-crop tool — needs agent_contract), crop keyframe strip | M | inspector-completion |
| 9 | Inspector Source metadata | InspectorView assetDetailsContent | inspector_view.rs source_asset_content | DONE (2026-07-10 inspector-completion): real File section (type/dims/duration/size/path from manifest+fs), AI badge, Generated section (catalog model name, aspect, resolution, duration), Prompt + copy (clipboard + 1.4s feedback). Remaining: references strip (deferred with generation-panel per proposal non-goal) | M | inspector-completion |
| 10 | Home cards/samples/sidebar | HomeView, ProjectCard, SampleProjectsStrip, WelcomeOverlay | app_root render_home | PARTIAL: no hover states/trash+confirm/context menu/file-missing overlay; samples hardcoded (no service/posters/progress); Open Project no file panel; no IdentityStrip | M | (to open) |
| 11 | AI Edit tab | Inspector/AIEditTab.swift (433L) | ai_edit_tab_view.rs (414L) | PARTIAL (uncertain): structure present, actions likely not dispatching real edits — verify | M | (to open) |
| 12 | Preview settings menus + capture frame | PreviewContainerView.swift (926L) | preview_view.rs (915L) | PARTIAL: aspect/fps/resolution/zoom menus + Capture Frame button missing/unverified | M | (to open) |
| 13 | Inspector numeric binding | InspectorView scrub rows | inspector_view.rs derive_scrub_values | DONE (2026-07-10 inspector-completion): rows derive from the selected clip (keyframe-resolved at playhead via timeline_core::resolved_*_at; volume in dB), scrub commit → set_clip_properties/update_text, per-section reset (Levels/Transform/Playback). Selection inputs (selected_clip_ids/playhead_frame) are public fields pending app-shell wiring from the timeline. Remaining: fade write-back (no fade tool), live apply during drag (commit-on-release only) | M | inspector-completion |
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
