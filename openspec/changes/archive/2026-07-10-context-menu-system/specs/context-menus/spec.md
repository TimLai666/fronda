## ADDED Requirements

### Requirement: Context menu component

A reusable context menu SHALL open at the pointer on right-click, close on outside click or Escape, and render items with hover highlight, separators, and a destructive style for dangerous actions.

#### Scenario: Open and dismiss

- **WHEN** the user right-clicks a surface with a menu and then clicks elsewhere
- **THEN** the menu appears at the pointer and disappears without triggering any item

### Requirement: Project card menu

Right-clicking a recent-project card SHALL offer Open, Reveal in File Manager, Remove from Recents, and Delete Project (with a confirmation step before deletion).

#### Scenario: Delete requires confirmation

- **WHEN** the user picks Delete Project
- **THEN** a confirmation appears and the project is deleted only after confirming

### Requirement: Asset and folder menus

Right-clicking a media asset SHALL offer Rename, Delete, and Reveal; right-clicking a folder SHALL offer Rename and Delete (assets move to the parent).

#### Scenario: Rename from the menu

- **WHEN** the user picks Rename on an asset
- **THEN** an inline editable field appears seeded with the current name and Enter commits via the standard rename path
