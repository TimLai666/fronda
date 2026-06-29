# Runtime, Packaging, Design System, and Shell Contracts

Scope sources:

- `Package.swift`
- `Sources/PalmierPro/Resources/Info.plist`
- `Sources/PalmierPro/Resources/**`
- `Sources/PalmierPro/Utilities/Constants.swift`
- `Sources/PalmierPro/UI/AppTheme.swift`
- `Sources/PalmierPro/App/main.swift`
- `Sources/PalmierPro/App/AppDelegate.swift`
- `Sources/PalmierPro/App/AppState.swift`
- `Sources/PalmierPro/App/MainMenu.swift`
- `Sources/PalmierPro/Toolbar/ToolbarView.swift`
- `Sources/PalmierPro/Settings/**`
- `Sources/PalmierPro/Help/**`
- `CONTRIBUTING.md`
- `AGENTS.md`

## A. Current Swift baseline and runtime envelope

- [x] `RUN-001`: The current implementation baseline is Swift `6.2` with package name `PalmierPro` and executable product `PalmierPro`.
- [x] `RUN-002`: The current supported platform is macOS `26.0+` only.
- [x] `RUN-003`: Development prerequisites remain macOS `26+`, Xcode `16+`, and the Swift `6.2` toolchain.
- [x] `RUN-004`: Standard development commands remain `swift build` and `swift run`; bundled debug launch remains `./scripts/dev.sh`.
- [x] `RUN-005`: The current app is a native Mac app built with SwiftUI, AppKit, and AVFoundation, and Fronda must preserve observable native-editor behavior before changing architecture.
- [x] `RUN-006`: The current app is intended as a non-sandboxed Developer ID app; any Fronda packaging plan must explicitly preserve or replace this distribution/security model.
- [x] `RUN-007`: Fronda uses Rust + `gpui-ce`, and the compatibility specs in this folder describe the current Palmier Pro-derived baseline that the active Fronda codebase must satisfy, not only desired architecture.

## B. Package dependencies and bundled resources

- [x] `PKG-001`: The Swift package dependency surface is part of the source baseline and currently includes:
  - `DSWaveformImage` from `14.2.2`
  - `modelcontextprotocol/swift-sdk` from `0.11.0`
  - `Sparkle` from `2.7.0`
  - `sentry-cocoa` from `8.40.0`
  - `clerk-convex-swift` from `0.1.0`
  - `clerk-ios` from `1.0.0`
  - `convex-swift` from `0.8.0`
  - `swift-transformers` from `1.3.3`
  - `lottie-ios` from `4.6.1`
- [x] `PKG-002`: The app target path remains `Sources/PalmierPro` in the Swift baseline.
- [x] `PKG-003`: `Resources/Info.plist`, `Resources/AppIcon.icon`, `Resources/AppIcon.icns`, and `Resources/AppIcon.png` are excluded from SwiftPM resource copying because they are bundle/app packaging inputs.
- [x] `PKG-004`: Bundled copied resources remain:
  - `Resources/Fonts`
  - `Resources/MCPB/palmier-pro.mcpb`
  - `Resources/Images`
  - `Resources/Changelog`
- [x] `PKG-005`: The app must register bundled fonts at startup before UI use.
- [x] `PKG-006`: The bundled Claude Desktop extension remains `palmier-pro.mcpb` unless intentionally renamed with matching help/install updates.
- [x] `PKG-007`: Bundled font discovery checks both `Fonts` and `PalmierPro_PalmierPro.bundle/Fonts` under `Bundle.main.resourceURL` at runtime instead of relying on compile-time environment splits.

## C. Bundle, document, URL, and updater metadata

- [x] `BNDL-001`: Bundle display name and bundle name remain `Palmier Pro`.
- [x] `BNDL-002`: Bundle executable remains `PalmierPro`.
- [x] `BNDL-003`: Bundle identifier remains `io.palmier.pro` unless intentionally migrated.
- [x] `BNDL-004`: Bundle package type remains `APPL`.
- [x] `BNDL-005`: Current version metadata is `CFBundleShortVersionString = 0.3.5` and `CFBundleVersion = 53`; version changes must also preserve the changelog/update badge semantics specified elsewhere.
- [x] `BNDL-006`: Minimum system version in bundle metadata remains `26.0` for the current Mac app.
- [x] `BNDL-007`: The app remains high-resolution capable.
- [x] `BNDL-008`: The app registers the `palmier` URL scheme under URL name `io.palmier.pro`.
- [x] `BNDL-009`: The app exports UTI `io.palmier.project`, description `Palmier Project`, filename extension `palmier`, and conformance to `com.apple.package`.
- [x] `BNDL-010`: The document type `Palmier Project` has role `Editor`, uses `LSTypeIsPackage = true`, and maps to document class `PalmierPro.VideoProject` in the Swift baseline.
- [x] `BNDL-011`: Sparkle automatic checks remain enabled in bundle metadata.
- [x] `BNDL-012`: The Sparkle feed URL remains `https://raw.githubusercontent.com/palmier-io/palmier-pro/main/appcast.xml` unless intentionally migrated.
- [x] `BNDL-013`: The Sparkle public EdDSA key remains part of the signed-update contract and must not be silently changed.

## D. Startup and app lifecycle

- [x] `BOOT-001`: Pre-run startup order remains logging bootstrap, telemetry startup, bundled font registration, tooltip-delay override, AppKit app/delegate/menu setup, then `NSApplication.run()`.
- [x] `BOOT-002`: Startup sets `NSInitialToolTipDelay` to `10` milliseconds, shortening the default tooltip delay from 2 seconds to 0.01 seconds.
- [x] `BOOT-003`: On launch, the app sets activation policy to regular and activates itself, including when launched from CLI.
- [x] `BOOT-004`: On launch, the app initializes the updater, shows Home, configures notifications, starts MCP if the user preference allows it, and only then defers account/model client configuration onto the main actor so backend setup cannot block first paint.
- [x] `BOOT-005`: `applicationShouldOpenUntitledFile` returns `false`; opening the app must not auto-create an untitled project document.
- [x] `BOOT-006`: Reopening the app with no visible windows shows Home and returns `true` from the reopen handler.
- [x] `BOOT-007`: Closing active project state through `showHome()` autosaves edited projects before hiding project windows and showing Home.
- [x] `BOOT-008`: Showing an editor hides Home, stores the active project, and shows the project document windows.
- [x] `BOOT-009`: Opening a sample project uses cached materialization when available, never registers the sample in Recents, and can optionally start the tutorial.
- [x] `BOOT-010`: Project open panels allow only the Palmier project UTType, disallow choosing directories as directories, treat file packages as packages, and allow one selection.
- [x] `BOOT-011`: New project save panels default to name `Untitled Project`, directory `~/Documents/Palmier Pro`, and Palmier project content type.

## E. Window size and native-window contracts

- [x] `WIN-001`: Home default window size is `960×640` (intentionally changed from Swift `1200×1200` for cross-platform ergonomics); Home minimum remains `760×480`.
- [x] `WIN-002`: Project default window size remains `1600×1000`; Project minimum remains `960×600`.
- [x] `WIN-003`: Project titlebar trailing reserved width remains `280`.
- [x] `WIN-004`: Settings window default content size remains `980×640`; minimum remains `760×480`; autosave name remains `PalmierProSettings-v2`.
- [x] `WIN-005`: Settings window uses dark appearance, translucent base background, hidden title, transparent titlebar, full-size content view, and is movable by background.
- [x] `WIN-006`: Help window keeps tabs `Shortcuts` and `MCP` with sidebar width `220`.
- [x] `WIN-007`: Feedback window size remains `480×480`; minimum remains `480×420`; title remains `Send feedback`; it uses the same dark/translucent hidden-title native window style as Settings.

## F. Main menu and command shortcuts

- [x] `MENU-001`: Main menu top-level groups remain App, File, Edit, View, and Help.
- [x] `MENU-002`: App menu items remain About Palmier Pro, Check for Updates…, Settings… (`Cmd+,`), and Quit Palmier Pro (`Cmd+Q`).
- [x] `MENU-003`: File menu items remain New (`Cmd+N`), Open… (`Cmd+O`), Save (`Cmd+S`), Save As… (`Cmd+Shift+S`), Import Media… (`Cmd+I`), and Export… (`Cmd+E`).
- [x] `MENU-004`: Edit menu items remain Undo (`Cmd+Z`), Redo (`Cmd+Shift+Z`), Cut (`Cmd+X`), Copy (`Cmd+C`), Paste (`Cmd+V`), Select All (`Cmd+A`), Split at Playhead (`Cmd+K`), Trim Start to Playhead (`Q`), Trim End to Playhead (`W`), and Delete (`Backspace`).
- [x] `MENU-005`: View menu items remain Media Panel (`Cmd+0`), Inspector (`Cmd+Option+0`), Agent Panel (`Cmd+Option+A`), Maximize Focused Panel (`` ` ``), Layout submenu, and Enter Full Screen (`Cmd+F`).
- [x] `MENU-006`: Layout submenu items remain Default (`Cmd+1`), Media (`Cmd+2`), and Vertical (`Cmd+3`).
- [x] `MENU-007`: Help menu items remain Tutorial, Keyboard Shortcuts (`Cmd+?`), MCP Instructions, and Send Feedback….
- [x] `MENU-008`: Menu actions route through the responder chain to the active editor where appropriate, rather than directly owning editor state in the menu builder.

## G. Help shortcuts surface

- [x] `KEY-001`: The visible shortcut help keeps Playback rows: Space play/pause, Left step backward, Right step forward, Shift+Left skip backward, Shift+Right skip forward.
- [x] `KEY-002`: The visible shortcut help keeps Tools rows: `V` selection tool and `C` razor tool.
- [x] `KEY-003`: The visible shortcut help keeps Editing rows: Cmd+K split, `[` or `Q` trim start, `]` or `W` trim end, Backspace delete, Shift+Backspace ripple delete, Option+Drag duplicate clip.
- [x] `KEY-004`: The visible shortcut help keeps Timeline rows: Shift+Drag Ruler select range, Drag Range Edge adjust range, `I` mark range start, `O` mark range end, Option+Scroll zoom to cursor, Pinch zoom to cursor, Cmd+Scroll scroll horizontally.
- [x] `KEY-005`: The visible shortcut help keeps File/Edit/View rows matching the main menu plus Cmd+Scroll preview zoom, Escape deselect/reset tool.
- [x] `KEY-006`: Shortcut help uses a key-column width of `118` and two-column grouping with the first four shortcut groups on the left.

## H. Toolbar and interaction constants

- [x] `UIX-001`: Toolbar height remains `38` and panel header height remains `28`.
- [x] `UIX-002`: Toolbar controls remain undo, redo, pointer tool, razor tool, split at playhead, trim start, trim end, add text, and timeline zoom slider.
- [x] `UIX-003`: Pointer tool shortcut remains `V`; razor tool shortcut remains `C`.
- [x] `UIX-004`: Timeline default `pixelsPerFrame` remains `4.0`.
- [x] `UIX-005`: Default generated/created durations remain image `5s`, audio TTS `10s`, audio music `60s`, and text `3s`.
- [x] `UIX-006`: Aspect-ratio tolerance remains `0.02`.
- [x] `UIX-007`: Zoom constants remain min `0.05`, floor `0.0001`, max `40.0`, scroll sensitivity `0.04`, magnify sensitivity `1.5`, pan speed `5.0`, and fit-all buffer `3.0`.
- [x] `UIX-008`: Timeline autoscroll constants remain edge zone width `56`, max zone fraction `0.5`, min step `4`, max step `28`, interval `1/60s`.
- [x] `UIX-009`: Track-size constants remain min height `32`, max height `200`, resize handle zone `6`.
- [x] `UIX-010`: Timeline layout constants remain min height `100`, max height `700`, default track height `50`, ruler height `24`, track header width `100`, drop zone height `60`, insert threshold `10`, and drag threshold `3`.
- [x] `UIX-011`: Media panel width constants remain default `500`, min `280`; inspector width constants remain default `260`, min `150`; agent panel min/max remain `240/640`; chat column max remains `640`.
- [x] `UIX-012`: Preview minimum size remains `400×320`.

## I. AppTheme token contract

- [x] `THM-001`: All UI styling must continue flowing through `AppTheme` tokens; introducing hardcoded spacing, font size, font weight, corner radius, border width, opacity, icon size, shadow, animation duration, or core color in UI code is a spec violation.
- [x] `THM-002`: Background tokens remain base `#0A0A0A`, surface `#161616`, raised `#1E1E1E`, prominent `#2C2C2C`, placeholder aliasing raised, and preview canvas black.
- [x] `THM-003`: Border tokens remain primary white `0.16`, subtle white `0.12`, divider white `0.44`; border widths remain hairline `0.5`, thin `1`, medium `1.5`, thick `2`.
- [x] `THM-004`: Accent tokens remain timecode color `(0.95, 0.6, 0.2)`, primary warm off-white `(0.961, 0.937, 0.894)`, and spotlight red/orange gradient.
- [x] `THM-005`: Text tokens remain primary white `1.0`, secondary white `0.80`, tertiary white `0.62`, muted white `0.34`.
- [x] `THM-006`: Opacity tokens remain opaque `1`, subtle `0.04`, hint `0.06`, faint `0.08`, soft `0.10`, muted `0.15`, moderate `0.25`, medium `0.35`, strong `0.55`, prominent `0.80`.
- [x] `THM-007`: Track colors remain video `#0091C2`, audio `#58A822`, image/text `#B72DD2`, and lottie `#E0A800`.
- [x] `THM-008`: Radius tokens remain xs `3`, xsSm `4`, sm `6`, md `10`, mdLg `12`, lg `14`, xl `20`; concentric radius is `max(outer - padding, 0)`.
- [x] `THM-009`: Spacing tokens remain xxs `2`, xs `4`, sm `6`, smMd `8`, md `10`, mdLg `12`, lg `14`, lgXl `16`, xl `20`, xlXxl `24`, xxl `28`.
- [x] `THM-010`: Font-size tokens remain micro `8`, xxs `9`, xs `10`, sm `11`, smMd `12`, md `13`, mdLg `14`, lg `15`, xl `18`, title1 `22`, title2 `28`, display `36`.
- [x] `THM-011`: Font-weight tokens remain light, regular, medium, semibold, and bold.
- [x] `THM-012`: Tracking tokens remain tight `-0.5`, normal `0`, wide `1.5`.
- [x] `THM-013`: Icon-size tokens remain xxs `12`, xs `14`, sm `18`, smMd `20`, md `22`, mdLg `24`, lg `26`, lgXl `28`, xl `30`.
- [x] `THM-014`: Component-size tokens remain caption preview max height `150`, caption preview max text width ratio `0.9`, tool image preview max height `50`, project card `150×120`, update overlay width `640`.
- [x] `THM-015`: Caption tokens remain default font size `48`, min `12`, max `300`, position range `0...1`, center snap value `0.5`, center snap threshold `0.02`, default center `(0.5, 0.9)`, and minimum display duration `0.7s`.
- [x] `THM-016`: Generation-panel tokens remain media area min height `120`, loading height `180`, prompt min height `40`, reference tile `80×56`.
- [x] `THM-017`: Media-panel tokens remain tab rail width `IconSize.lg + Spacing.sm * 2` and context row height `IconSize.md`.
- [x] `THM-018`: Shadows remain sm `(black 0.3, radius 1, x 0, y 0.5)`, md `(black 0.3, radius 4, x 0, y 2)`, lg `(black 0.25, radius 24, x 0, y 8)`.
- [x] `THM-019`: Animation duration tokens remain hover `0.15s` and transition `0.2s`.
- [x] `THM-020`: `panelHeaderBar()` remains full-width, height `Layout.panelHeaderHeight`, raised background, and bottom primary border of thin width.

## J. Settings surface and persisted preferences

- [x] `SETUI-001`: Settings tabs remain Account, General, Models, Agent, and Storage with SF Symbol icons `person.circle`, `gearshape`, `square.stack.3d.up`, `paperplane`, and `internaldrive`.
- [x] `SETUI-002`: The Account tab and identity strip are hidden when account backend configuration is misconfigured.
- [x] `SETUI-003`: If the selected Settings tab becomes hidden, Settings selects the first visible tab or General as fallback.
- [x] `SETUI-004`: General settings contain notification and privacy sections in that order.
- [x] `SETUI-005`: Models settings group enabled-model toggles by Image, Video, and Audio; search filters by case-insensitive display-name substring.
- [x] `SETUI-006`: When the model catalog is not loaded and no rows are visible, Models shows `Loading models…`; when loaded but filtered empty, it shows `No models match "<query>".`.
- [x] `SETUI-007`: Storage settings show cache path and size, clear playback-preview/waveform/filmstrip caches, toggle on-device media search, clear global search index, and remove the installed visual model.
- [x] `SETUI-008`: Storage cache display path replaces the home directory with `~`.
- [x] `SETUI-009`: Agent settings store the Anthropic API key in macOS Keychain, open `https://console.anthropic.com/settings/keys`, mask saved keys as 36 bullets plus the last 4 characters, and show all bullets for keys of length `<= 4`.
- [x] `SETUI-010`: MCP settings toggle the persisted MCP enabled preference and show status `Running on 127.0.0.1:<port>` or `Stopped`.
- [x] `SETUI-011`: The MCP enabled preference key remains `io.palmier.pro.mcp.enabled`, defaults to enabled when absent, and starts/stops the runtime service immediately when changed.
- [x] `SETUI-012`: The in-app agent model preference key remains `agentModel` and defaults to `sonnet46` when absent or invalid.

## K. Help, MCP instructions, and feedback UX

- [x] `HELP-001`: Help tabs remain `Shortcuts` and `MCP`.
- [x] `HELP-002`: MCP instructions use server URL `http://127.0.0.1:<port>` and endpoint `<serverURL>/mcp`.
- [x] `HELP-003`: MCP instructions continue exposing copyable Claude Code command `claude mcp add --transport http palmier-pro <endpoint>`.
- [x] `HELP-004`: MCP instructions continue exposing copyable Codex command `codex mcp add palmier-pro --url <endpoint>`.
- [x] `HELP-005`: MCP instructions continue exposing Cursor JSON config with server name `palmier-pro`, type `http`, and the MCP endpoint.
- [x] `HELP-006`: MCP instructions continue offering Claude Desktop installation through the bundled `.mcpb` extension.
- [x] `FBK-001`: Feedback starts with message empty, email empty, include screenshot enabled, may-contact enabled, not sending, no error, not sent.
- [x] `FBK-002`: Feedback submission trims message/email, requires non-empty message, and uses `Cmd+Return` as submit shortcut.
- [x] `FBK-003`: Cancel uses the cancel keyboard action; success Done uses the default keyboard action.
- [x] `FBK-004`: Feedback screenshot capture chooses key window, main window, or first visible non-feedback window, in that order.
- [x] `FBK-005`: Feedback captures the screenshot before the feedback window becomes key so the feedback window is not included.
- [x] `FBK-006`: Feedback submission sends message, optional email, may-contact only when a reply email exists, optional base64 PNG screenshot, app version, and OS version.

## L. Generation catalog schema and pricing contract

- [x] `CAT-001`: Model catalog configuration subscribes to Convex query `models:list` and does nothing when `AccountService.shared.convex` is unavailable.
- [x] `CAT-002`: Applying catalog entries rebuilds video, image, audio, upscale, and `byId` maps atomically, then sets `isLoaded = true` and clears `lastError`.
- [x] `CAT-003`: Catalog entry core fields remain `id`, `kind`, `displayName`, `allowedEndpoints`, `responseShape`, `uiCapabilities`, optional pricing fields, and optional qualities.
- [x] `CAT-004`: Catalog `kind` values remain `video`, `image`, `audio`, and `upscale`.
- [x] `CAT-005`: Catalog `responseShape` values remain `video`, `images`, `audio`, and `upscaledImage`.
- [x] `CAT-006`: Video capabilities include durations, optional resolutions, aspect ratios, first/last-frame support, max reference counts by image/video/audio, optional total reference limit, optional combined reference seconds by video/audio, frames-and-references exclusivity, reference tag noun, requires-source-video, and requires-reference-image.
- [x] `CAT-007`: Image capabilities include optional resolutions, aspect ratios, optional qualities, image-reference support, and max images.
- [x] `CAT-008`: Audio capabilities include category, voices, default voice, lyrics/instrumental/style support, optional durations, minimum prompt length, supported inputs, prompt label, and min/max seconds.
- [x] `CAT-009`: Upscale capabilities include speed label, p75 duration seconds, and supported clip types.
- [x] `CAT-010`: Unknown audio pricing modes fail catalog decode rather than becoming silent unknown pricing.
- [x] `CAT-011`: Model display-name lookup falls back to the model id when the id is not present in the registry.
- [x] `CAT-012`: Disabled-model preferences filter generation choices but must not mutate the live model catalog.

## M. Generation request payload and validation contract

- [x] `GPAY-001`: Video generation payload encodes `kind = video`, prompt, duration, aspect ratio, optional resolution, optional source video URL, optional start/end frame URLs, non-empty reference image/video/audio URL arrays, and `generateAudio`.
- [x] `GPAY-002`: Video validation rejects unsupported duration, aspect ratio, or resolution using the current `unsupportedValue` message format.
- [x] `GPAY-003`: Video `supportsReferences` is true when any max reference count by type is positive.
- [x] `GPAY-004`: Video `maxReferences` uses `maxTotalReferences` when present, otherwise the sum of image/video/audio max reference counts.
- [x] `GPAY-005`: Video audio-discount lookup first tries the selected resolution key, then the empty-string default key.
- [x] `GPAY-006`: Image generation payload encodes `kind = image`, prompt, aspect ratio, optional resolution, optional quality, non-empty `imageURLs`, and `numImages`.
- [x] `GPAY-007`: Image model `maxImages` clamps catalog max images into the current client range `1...4`.
- [x] `GPAY-008`: Image validation rejects unsupported aspect ratio, resolution, quality, reference image usage when unsupported, and `numImages` outside `1...maxImages`.
- [x] `GPAY-009`: Image resolution labels parse only lowercase/uppercase-insensitive `WxH` pairs with two integer components.
- [x] `GPAY-010`: Image resolution display labels map square to `Square`, non-square to `Landscape`/`Portrait`, and long edges `3840`, `2560`, `1920`, `1024/1536`, or other values to `4K`, `2K`, `1080p`, empty tier, or `<longEdge>p` respectively.
- [x] `GPAY-011`: Audio generation payload encodes `kind = audio`, prompt, optional voice, optional lyrics, optional style instructions, instrumental flag, optional duration seconds, and optional video URL.
- [x] `GPAY-012`: Audio category labels remain Speech, Music, and Sound Effects; unknown catalog categories default to Speech.
- [x] `GPAY-013`: Audio inputs default to text when absent; prompt label defaults to `Describe the sound`; min/max seconds default to `1` and `900`.
- [x] `GPAY-014`: Audio validation rejects prompts shorter than `minPromptLength`, unsupported voices, unsupported durations, and video spans outside min/max seconds.
- [x] `GPAY-015`: Upscale generation payload encodes `kind = upscale`, source URL, and duration seconds.
- [x] `GPAY-016`: Upscale models filter by supported `ClipType` parsed from catalog strings.

## N. Cost-estimation contract

- [x] `COST-001`: Video cost returns nil when there are no rates or duration is non-positive; otherwise it resolves rate by selected resolution or empty default key, applies no-audio discount when available, multiplies by duration, and rounds credits up.
- [x] `COST-002`: Image cost returns nil when there are no rates; otherwise it uses `resolution|quality` matrix pricing first, quality-only pricing second when qualities exist, then resolution/default pricing, multiplies by image count clamped to at least 1, and rounds credits up.
- [x] `COST-003`: Audio per-thousand-character cost requires non-empty prompt and rounds `rate * prompt.count / 1000` up.
- [x] `COST-004`: Audio per-second cost requires positive duration and rounds `rate * duration` up.
- [x] `COST-005`: Audio flat cost rounds the flat price up.
- [x] `COST-006`: Unknown audio pricing returns nil.
- [x] `COST-007`: Upscale cost uses `max(1, durationSeconds) * creditsPerSecond`, rounded up.
- [x] `COST-008`: Cost formatting returns `—` for nil, `0 credits` for `<= 0`, `1 credit` for exactly 1, and `<n> credits` otherwise.
- [x] `COST-009`: Rerun cost reconstruction dispatches through the live model registry by stored model id and uses stored generation input fields, defaulting video `generateAudio` to true and image `numImages` to 1 when absent.

## O. Backend configuration contract

- [x] `CFG-001`: Backend config reads `PalmierClerkPublishableKey`, `PalmierConvexDeploymentURL`, and `PalmierConvexHttpURL` from the main bundle info dictionary.
- [x] `CFG-002`: Empty backend config strings are treated as missing.
- [x] `CFG-003`: Backend config is considered configured only when Clerk publishable key and Convex deployment URL are present.
- [x] `CFG-004`: Convex HTTP URL may be absent without making the whole account backend misconfigured, but features requiring it must fail cleanly.

## Migration decisions to record explicitly

- `Decision:` Fronda should decide whether this file remains a compatibility appendix for the Swift/macOS app or becomes the source of truth for the first cross-platform packaging target.
- `Decision:` Several AppTheme constants are visual-identity tokens rather than pure behavior. Fronda should preserve them for visual parity unless a deliberate redesign is approved.
- `Decision:` Sparkle, Clerk, Convex, Sentry, Keychain, and macOS notification/window behavior are platform-specific. Fronda must either preserve them on macOS or document cross-platform substitutions with equivalent user-visible states.
- `Decision:` `Fronda` is the primary Rust product name, while current bundle names, executable names, package extensions, URL schemes, and MCP identifiers still use inherited Palmier identifiers. Any identifier migration should happen as one explicit compatibility change, not piecemeal.
