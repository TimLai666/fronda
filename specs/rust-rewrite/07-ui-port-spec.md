# UI Port Spec — gpui Views

Ported from Swift Palmier Pro views to Rust gpui.

## Design System

All colors/spacing/radii/fonts come from `AppTheme` (Rust `theme.rs`).
Values match Swift `AppTheme.swift` exactly.

## Views

### 1. MediaPanelView
- Source: `Sources/PalmierPro/MediaPanel/MediaPanelView.swift`
- Left tab rail: 3 icon buttons (folder, captions.bubble, music.note), 26px wide
- Selected tab gets left capsule indicator
- Hover tooltip (capsule with text) on tab buttons
- Right border hairline
- Tab content area switches between MediaTab / CaptionTab / MusicTab
- Animated transitions (easeInOut 0.2s)

### 2. ToolbarView
- Source: `Sources/PalmierPro/Toolbar/ToolbarView.swift`
- Horizontal strip 38px high
- Undo/Redo buttons, Tool mode (Pointer/Razor), Split/Trim, Add Text
- Zoom slider (log-mapped) with +/- magnifying glass icons

### 3. PreviewView
- Source: `Sources/PalmierPro/Preview/PreviewContainerView.swift`
- Black canvas area
- Transport bar with play/pause, scrub bar
- Zoom/pan support (cmd+scroll)

### 4. TimelineView
- Sources: `TimelineContainerView.swift`, `TimelineHeaderView.swift`, `TimelineView.swift`
- Horizontal + vertical scroll
- Ruler header with timecodes
- Track rows with left header (color strip + label + mute/hide/sync buttons)
- Track resize handles

### 5. InspectorView
- Source: `Sources/PalmierPro/Inspector/InspectorView.swift`
- Tab bar: Video, Audio, Speed, Text, Transform, AI Edit
- Collapsible sections with headers
- Property rows: volume slider, fade dropdown, speed field
- Reset buttons on section headers

## Layout
Default layout (Swift):
```
| Media (280px min) | Preview + Toolbar + Timeline + Agent | Inspector (150px min) |
```
Panel divider: 1px hairline border, `BorderColors::PRIMARY`
Toolbar: 38px between Preview and Timeline
Agent chat column max width: 640px
