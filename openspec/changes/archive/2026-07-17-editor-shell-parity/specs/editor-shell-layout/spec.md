## ADDED Requirements

### Requirement: Default preset skeleton matches Swift split structure

With the Default layout preset and all panes visible, the editor SHALL arrange panes as an outer horizontal split of the Agent column and a preset area, where the preset area is a vertical split whose upper region contains Media, Preview, and Inspector columns (left to right) and whose lower region contains the Toolbar row and the Timeline pane spanning the full preset-area width. The lower region's initial height SHALL be 30% of the preset area height, clamped to the timeline height limits.

#### Scenario: Default preset with all panes visible

- **WHEN** the editor opens with the Default preset and all panes visible
- **THEN** the Timeline (with its Toolbar) spans from the Agent column's right edge to the window's right edge, below the Media, Preview, and Inspector columns

#### Scenario: Timeline spans full width when side panes hidden

- **WHEN** the Media and Inspector panes are hidden in the Default preset
- **THEN** the upper region shows only the Preview column and the lower Toolbar + Timeline region still spans the full preset-area width

### Requirement: Media preset skeleton matches Swift split structure

With the Media layout preset, the editor SHALL arrange the preset area as a horizontal split of the Media column (initial width 30% of the preset area) and a right region, where the right region is a vertical split of an upper Preview | Inspector row (initial height 55%) and a lower Toolbar + Timeline region.

#### Scenario: Media preset arrangement

- **WHEN** the user switches to the Media preset with all panes visible
- **THEN** the Media pane occupies the left column, the Preview and Inspector panes share the upper right region, and the Toolbar + Timeline occupy the lower right region

### Requirement: Vertical preset skeleton matches Swift split structure

With the Vertical layout preset, the editor SHALL arrange the preset area as a horizontal split (initial 50/50) of a left region and the Preview column, where the left region is a vertical split of an upper Media | Inspector row (initial height 55%) and a lower Toolbar + Timeline region.

#### Scenario: Vertical preset arrangement

- **WHEN** the user switches to the Vertical preset with all panes visible
- **THEN** the Media and Inspector panes share the upper left region, the Toolbar + Timeline occupy the lower left region, and the Preview pane occupies the full-height right column

### Requirement: Agent column is a preset-independent outer column

The Agent pane SHALL render as the outermost left column in every layout preset, and switching presets SHALL NOT move the Agent pane into the preset area.

#### Scenario: Agent column survives preset switches

- **WHEN** the user switches between Default, Media, and Vertical presets with the Agent pane visible
- **THEN** the Agent pane remains the leftmost full-height column in all three presets

### Requirement: Preset switching preserves pane visibility

Applying a layout preset SHALL NOT modify any pane's visibility state. Pane visibility SHALL change only through explicit pane toggles or pane maximize/unmaximize operations.

#### Scenario: Media preset keeps hidden and visible panes as-is

- **WHEN** the Inspector pane is visible and the Agent pane is hidden and the user switches to the Media preset
- **THEN** the Inspector pane remains visible and the Agent pane remains hidden

### Requirement: Panel card shell

Every visible pane SHALL render as a rounded surface-colored card inset by half the panel gap on all sides against the base-colored background, so adjacent panes are separated by a visible base-colored gap equal to the panel gap.

#### Scenario: Adjacent panes show a gap

- **WHEN** two panes render side by side
- **THEN** a base-colored gap of the panel-gap width separates their surface-colored rounded cards

### Requirement: Draggable pane dividers

The gaps between the Agent/Media/Inspector columns and their neighbors, and the gap above the Toolbar + Timeline region, SHALL act as drag handles that resize the adjacent pane. Dragging SHALL clamp so that: the Agent width stays within 240 to 640 points, the Media width is at least 280 points plus the tab-rail width, the Inspector width is at least 150 points, the Timeline height stays within its existing 100 to 700 point limits, and no drag reduces the Preview column below 400 points wide.

#### Scenario: Dragging the media divider resizes within limits

- **WHEN** the user drags the divider between the Media and Preview columns
- **THEN** the Media column width follows the pointer, never drops below 280 points plus the tab-rail width, and never forces the Preview column below 400 points

#### Scenario: Divider cursor feedback

- **WHEN** the pointer hovers over a pane divider gap
- **THEN** the cursor changes to the matching resize cursor (column-resize for vertical dividers, row-resize for the timeline divider)

### Requirement: Preview empty-state canvas

When no composited frame is available, the Preview pane SHALL render a surface-colored panel containing a centered base-colored canvas rectangle whose aspect ratio matches the active timeline's dimensions, outlined with a subtle border.

#### Scenario: Empty project shows canvas bounds

- **WHEN** a new empty project opens in the editor
- **THEN** the Preview pane shows a centered canvas rectangle with the timeline's aspect ratio against the surface-colored panel background

### Requirement: Welcome overlay stays within the window

The first-run welcome card SHALL render centered in the window, and its action buttons SHALL remain fully visible and clickable at any window size at or above the minimum supported window size.

#### Scenario: Small window keeps buttons reachable

- **WHEN** the welcome overlay shows in a 1280 by 720 window
- **THEN** the entire card including its action buttons is inside the window bounds

### Requirement: Windows menu shortcuts

On non-macOS platforms, menu actions with declared shortcuts SHALL be triggerable via their keyboard shortcuts with the platform primary modifier (Ctrl), dispatched through the same code path as their menu items. Editing shortcuts owned by text inputs (Ctrl+A, Ctrl+C, Ctrl+V, Ctrl+X, Ctrl+Z, Ctrl+Y) SHALL NOT be bound as menu shortcuts.

#### Scenario: Ctrl+N creates a new project

- **WHEN** the user presses Ctrl+N on Windows with no blocking dialog open
- **THEN** the NewProject menu action runs, matching the Home screen's New Project button behavior

### Requirement: Title bar menu bar on non-macOS

On non-macOS platforms, the title bar SHALL show the application menu (Fronda), File, Edit, View, and Help menus whose items mirror the application menu definition, execute their menu actions when clicked, and display the items' shortcut hints. The menu definition SHALL have a single source shared with the shortcut bindings. (The Swift baseline `MainMenu.swift` defines exactly these five groups; there is no Window menu.)

#### Scenario: File menu opens and executes

- **WHEN** the user clicks File in the title bar and selects New Project
- **THEN** the menu closes and the NewProject action executes

### Requirement: Media rail tab icons

The media panel tab rail SHALL render each tab (media, captions, music) with an SVG glyph icon instead of placeholder letter text, keeping the existing active-tab styling.

#### Scenario: Rail shows icons

- **WHEN** the media panel renders its tab rail
- **THEN** each of the three tabs shows its SVG icon and no placeholder letters are visible

### Requirement: Desktop feature enables the platform font backend

The `desktop-app` cargo feature of `fronda-app-shell-gpui` SHALL enable `gpui_platform/font-kit` so the desktop binary can rasterize text glyphs on macOS. A regression test SHALL assert that the feature declaration continues to include `gpui_platform/font-kit`.

#### Scenario: macOS build renders text

- **WHEN** the desktop binary built with the `desktop-app` feature runs on macOS
- **THEN** all UI text labels render alongside the vector shapes, panel backgrounds, borders, and icons

#### Scenario: Regression test pins the feature declaration

- **WHEN** the `desktop-app` feature declaration stops including `gpui_platform/font-kit`
- **THEN** the test `desktop_app_enables_macos_font_backend` fails

### Requirement: Native macOS application menu

On macOS, the app SHALL register a native application menu at startup, translated from the same single-source menu definition used by the non-macOS title bar menu (application menu, File, Edit, View, and Help groups with the Swift baseline's separator grouping, and Layout as a View submenu). Selecting a menu item SHALL dispatch the same action through the same code path as the non-macOS title bar menu. Menu actions with declared Command shortcuts SHALL be triggerable via those shortcuts, dispatching the same actions, with the shortcuts shown as the menu items' key equivalents. Editing shortcuts owned by text inputs (Cmd+A, Cmd+C, Cmd+V, Cmd+X, Cmd+Z) SHALL keep priority inside focused text inputs.

#### Scenario: Menu bar appears with the shared groups

- **WHEN** the app launches on macOS
- **THEN** the native menu bar shows the application menu, File, Edit, View, and Help, whose items mirror the shared menu definition

#### Scenario: Layout switches through the View menu

- **WHEN** the user selects Default, Media, or Vertical from the View menu's Layout submenu on macOS
- **THEN** the editor switches to that layout preset, identically to the non-macOS title bar menu's layout items

#### Scenario: Command shortcut dispatches the same action

- **WHEN** the user presses Cmd+N on macOS with no blocking dialog open
- **THEN** the NewProject menu action runs, matching the File menu item and the Home screen's New Project button behavior

#### Scenario: Text inputs keep editing shortcuts

- **WHEN** focus is inside a text input on macOS and the user presses Cmd+C
- **THEN** the text input's copy handling runs instead of the menu action
