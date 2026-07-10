//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::generation_view::GenerationView;
use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, IconSize, Layout, MediaPanel,
    Opacity, Radius, Spacing, Text,
};
use core_model::{ClipType, MediaFolder, MediaManifest, MediaManifestEntry};
use gpui::{
    div, prelude::*, px, AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render,
    SharedString, Styled, Window,
};

// ── Library view state (pure logic; media-library-ui spec) ──────────────────

/// Grid organization (Swift MediaTab.ViewMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryViewMode {
    #[default]
    Folders,
    Flat,
    Grouped,
}

impl LibraryViewMode {
    pub fn all() -> [LibraryViewMode; 3] {
        [Self::Folders, Self::Flat, Self::Grouped]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Folders => "Folders",
            Self::Flat => "Flat",
            Self::Grouped => "Grouped",
        }
    }
}

/// Grid ordering (Swift MediaTab.SortMode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySortKey {
    Name,
    #[default]
    DateAdded,
    Duration,
    Type,
}

impl LibrarySortKey {
    pub fn all() -> [LibrarySortKey; 4] {
        [Self::Name, Self::DateAdded, Self::Duration, Self::Type]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::DateAdded => "Date Added",
            Self::Duration => "Duration",
            Self::Type => "Type",
        }
    }
}

/// View-only media-library state (search, navigation, selection). Pure and
/// unit-testable; the manifest itself stays in the shared executor.
#[derive(Debug, Clone, Default)]
pub struct LibraryState {
    pub search_query: String,
    pub view_mode: LibraryViewMode,
    pub sort_key: LibrarySortKey,
    /// Empty = all types pass.
    pub type_filter: Vec<ClipType>,
    pub filter_ai: bool,
    pub current_folder: Option<String>,
    pub selection: Vec<String>,
    /// Last plainly-clicked or toggled id; shift-click extends from here.
    pub selection_anchor: Option<String>,
}

impl LibraryState {
    pub fn trimmed_query(&self) -> &str {
        self.search_query.trim()
    }

    pub fn search_active(&self) -> bool {
        !self.trimmed_query().is_empty()
    }

    pub fn has_active_filters(&self) -> bool {
        !self.type_filter.is_empty() || self.filter_ai
    }

    pub fn toggle_type_filter(&mut self, t: ClipType) {
        if let Some(pos) = self.type_filter.iter().position(|x| *x == t) {
            self.type_filter.remove(pos);
        } else {
            self.type_filter.push(t);
        }
    }

    pub fn clear_filters(&mut self) {
        self.type_filter.clear();
        self.filter_ai = false;
    }

    /// Plain click: the id becomes the whole selection and the anchor.
    pub fn select_click(&mut self, id: &str) {
        self.selection = vec![id.to_string()];
        self.selection_anchor = Some(id.to_string());
    }

    /// Ctrl/cmd-click: toggle membership; the id becomes the anchor.
    pub fn select_toggle(&mut self, id: &str) {
        if let Some(pos) = self.selection.iter().position(|x| x == id) {
            self.selection.remove(pos);
        } else {
            self.selection.push(id.to_string());
        }
        self.selection_anchor = Some(id.to_string());
    }

    /// Shift-click: select the contiguous span between the anchor and the id
    /// in `ordered` (the current display order). Falls back to a plain click
    /// when there is no usable anchor. The anchor is kept so a further
    /// shift-click re-extends from the same origin.
    pub fn select_range(&mut self, ordered: &[String], id: &str) {
        let to = ordered.iter().position(|x| x == id);
        let from = self
            .selection_anchor
            .as_ref()
            .and_then(|a| ordered.iter().position(|x| x == a));
        let (Some(from), Some(to)) = (from, to) else {
            self.select_click(id);
            return;
        };
        let (lo, hi) = (from.min(to), from.max(to));
        self.selection = ordered[lo..=hi].to_vec();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selection_anchor = None;
    }
}

/// Stable sort key for ClipType (Swift sorts by rawValue).
fn clip_type_key(t: &ClipType) -> &'static str {
    match t {
        ClipType::Audio => "audio",
        ClipType::Image => "image",
        ClipType::Lottie => "lottie",
        ClipType::Sequence => "sequence",
        ClipType::Shape => "shape",
        ClipType::Text => "text",
        ClipType::Video => "video",
    }
}

/// Type filter + AI filter + name-substring search (Swift passesFilters).
fn entry_passes(entry: &MediaManifestEntry, state: &LibraryState) -> bool {
    let type_ok = state.type_filter.is_empty() || state.type_filter.contains(&entry.r#type);
    let ai_ok = !state.filter_ai || entry.generation_input.is_some();
    let q = state.trimmed_query().to_lowercase();
    let name_ok = q.is_empty() || entry.name.to_lowercase().contains(&q);
    type_ok && ai_ok && name_ok
}

fn sort_entries<'a>(
    mut entries: Vec<&'a MediaManifestEntry>,
    key: LibrarySortKey,
) -> Vec<&'a MediaManifestEntry> {
    match key {
        // Manifest order is insertion order (Swift .dateAdded keeps it).
        LibrarySortKey::DateAdded => {}
        LibrarySortKey::Name => {
            entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
        LibrarySortKey::Duration => {
            entries.sort_by(|a, b| b.duration.total_cmp(&a.duration));
        }
        LibrarySortKey::Type => {
            entries.sort_by(|a, b| clip_type_key(&a.r#type).cmp(clip_type_key(&b.r#type)));
        }
    }
    entries
}

/// Filtered + sorted assets of one folder bucket.
pub fn visible_entries_in<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
    folder: Option<&str>,
) -> Vec<&'a MediaManifestEntry> {
    sort_entries(
        manifest
            .entries
            .iter()
            .filter(|e| e.folder_id.as_deref() == folder && entry_passes(e, state))
            .collect(),
        state.sort_key,
    )
}

/// Assets visible in the grid: filter → folder scope → sort. An active search
/// spans the whole library (Swift switches to searchResults); Flat and Grouped
/// span the library; Folders shows the current folder's bucket.
pub fn visible_entries<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<&'a MediaManifestEntry> {
    if state.search_active() || state.view_mode != LibraryViewMode::Folders {
        sort_entries(
            manifest
                .entries
                .iter()
                .filter(|e| entry_passes(e, state))
                .collect(),
            state.sort_key,
        )
    } else {
        visible_entries_in(manifest, state, state.current_folder.as_deref())
    }
}

/// Folder tiles: only in Folders view while not searching — the current
/// folder's subfolders.
pub fn visible_folders<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<&'a MediaFolder> {
    if state.search_active() || state.view_mode != LibraryViewMode::Folders {
        return Vec::new();
    }
    manifest
        .folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == state.current_folder.as_deref())
        .collect()
}

/// Breadcrumb chain root→leaf for a folder. Cycle-safe.
pub fn folder_path<'a>(
    manifest: &'a MediaManifest,
    folder_id: Option<&str>,
) -> Vec<&'a MediaFolder> {
    let mut path: Vec<&MediaFolder> = Vec::new();
    let mut cur = folder_id;
    while let Some(id) = cur {
        let Some(f) = manifest.folders.iter().find(|f| f.id == id) else {
            break;
        };
        if path.iter().any(|p| p.id == f.id) {
            break; // cycle guard
        }
        path.push(f);
        cur = f.parent_folder_id.as_deref();
    }
    path.reverse();
    path
}

/// Subfolder + asset count shown on a folder tile.
pub fn folder_child_count(manifest: &MediaManifest, folder_id: &str) -> usize {
    manifest
        .folders
        .iter()
        .filter(|f| f.parent_folder_id.as_deref() == Some(folder_id))
        .count()
        + manifest
            .entries
            .iter()
            .filter(|e| e.folder_id.as_deref() == Some(folder_id))
            .count()
}

/// Grouped-view sections: root bucket first (skipped when empty), then every
/// folder ordered by its full path, each with its filtered + sorted assets.
pub fn grouped_sections<'a>(
    manifest: &'a MediaManifest,
    state: &LibraryState,
) -> Vec<(Option<&'a str>, String, Vec<&'a MediaManifestEntry>)> {
    let mut sections = Vec::new();
    let root = visible_entries_in(manifest, state, None);
    if !root.is_empty() {
        sections.push((None, "Library".to_string(), root));
    }
    let mut folders: Vec<(&MediaFolder, String)> = manifest
        .folders
        .iter()
        .map(|f| {
            let title = folder_path(manifest, Some(&f.id))
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            (f, title)
        })
        .collect();
    folders.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    for (f, title) in folders {
        sections.push((
            Some(f.id.as_str()),
            title,
            visible_entries_in(manifest, state, Some(&f.id)),
        ));
    }
    sections
}

/// Simple tooltip capsule for tab buttons.
struct TabTooltip {
    label: SharedString,
}

impl Render for TabTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(Spacing::SM))
            .py(px(Spacing::XXS))
            .rounded(px(Radius::SM))
            .bg(Background::PROMINENT)
            .text_color(Text::PRIMARY)
            .text_size(px(FontSize::XS))
            .child(self.label.clone())
    }
}

/// Which toolbar dropdown is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarMenu {
    View,
    Sort,
    Filter,
    Overflow,
}

/// Media panel gpui entity.
pub struct MediaPanelView {
    pub state: MediaPanelState,
    /// Library view state (search / folders / sort / filter / selection).
    pub library: LibraryState,
    /// Snapshot of the shared manifest (rebuilt on revision bumps) so render
    /// and pure helpers work without holding the executor lock.
    manifest: MediaManifest,
    /// Search index status line from the executor ("" = nothing to show).
    search_status: String,
    focus_handle: FocusHandle,
    /// AI generation panel embedded in the media tab (Swift: GenerationView).
    pub generation: Entity<GenerationView>,
    /// Last seen shared-state revision; manifest changes rebuild the grid.
    state_revision: u64,
    /// Live search box (Swift: TextField bound to searchQuery).
    search_field: Entity<crate::text_field::TextField>,
    /// Inline folder rename (same pattern as the timeline tab rename).
    folder_rename_field: Entity<crate::text_field::TextField>,
    /// Folder id being renamed inline, if any.
    folder_editing: Option<String>,
    open_menu: Option<ToolbarMenu>,
}

impl MediaPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let gen = cx.new(|cx| GenerationView::new(cx));
        let search_field = cx.new(|cx| crate::text_field::TextField::new(cx, "Search"));
        cx.subscribe(&search_field, |this, field, event, cx| {
            if matches!(event, crate::text_field::TextFieldEvent::Edited) {
                this.library.search_query = field.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let folder_rename_field =
            cx.new(|cx| crate::text_field::TextField::new(cx, "Folder name"));
        cx.subscribe(&folder_rename_field, |this, _field, event, cx| {
            if matches!(event, crate::text_field::TextFieldEvent::Submitted) {
                this.commit_folder_rename(cx);
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            state: MediaPanelState::new(),
            library: LibraryState::default(),
            manifest: MediaManifest::default(),
            search_status: String::new(),
            focus_handle: cx.focus_handle(),
            generation: gen,
            state_revision: u64::MAX,
            search_field,
            folder_rename_field,
            folder_editing: None,
            open_menu: None,
        };
        view.sync_from_shared_state();
        view
    }

    /// Rebuild grid data from the shared manifest when the revision moved.
    fn sync_from_shared_state(&mut self) -> bool {
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if revision == self.state_revision {
            return false;
        }
        self.state_revision = revision;
        let executor = hub.executor();
        let Ok(exec) = executor.lock() else {
            return false;
        };
        let root = hub.project_root();
        self.state
            .sync_from_manifest(exec.media_manifest(), root.as_deref());
        self.manifest = exec.media_manifest().clone();
        self.search_status = exec.search_status().to_string();
        drop(exec);
        // Prune view state that points at deleted things (Swift
        // pruneStaleFolderState).
        let folder_exists =
            |id: &String| self.manifest.folders.iter().any(|f| &f.id == id);
        if self.library.current_folder.as_ref().is_some_and(|id| !folder_exists(id)) {
            self.library.current_folder = None;
        }
        if self.folder_editing.as_ref().is_some_and(|id| !folder_exists(id)) {
            self.folder_editing = None;
        }
        self.library
            .selection
            .retain(|id| self.manifest.entries.iter().any(|e| &e.id == id));
        true
    }

    pub fn select_tab(&mut self, tab: MediaPanelTab, cx: &mut Context<Self>) {
        self.state.select_tab(tab);
        cx.notify();
    }

    /// Run a tool on the shared executor; tool errors leave the UI unchanged.
    fn run_shared_tool(tool: &str, args: serde_json::Value) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            if let Err(reason) = exec.execute(tool, &args) {
                eprintln!("{tool} failed: {reason}");
            }
        }
    }

    /// Batch delete: every selected asset through delete_media.
    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        for id in std::mem::take(&mut self.library.selection) {
            Self::run_shared_tool("delete_media", serde_json::json!({ "mediaId": id }));
        }
        self.library.clear_selection();
        cx.notify();
    }

    /// New Folder in the current folder; opens the inline rename on it
    /// (Swift createNewFolderInCurrent).
    fn create_folder_in_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut args = serde_json::json!({ "name": "New Folder" });
        if let Some(parent) = &self.library.current_folder {
            args["parentFolderId"] = serde_json::Value::String(parent.clone());
        }
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let new_id = executor.lock().ok().and_then(|mut exec| {
            exec.execute("create_folder", &args).ok()?;
            exec.media_manifest().folders.last().map(|f| f.id.clone())
        });
        if let Some(id) = new_id {
            self.folder_editing = Some(id);
            self.folder_rename_field.update(cx, |field, cx| {
                field.set_text("New Folder", cx);
            });
            window.focus(&self.folder_rename_field.focus_handle(cx), cx);
        }
        cx.notify();
    }

    /// Commit an in-progress folder rename (Enter or click-away; Swift
    /// commits on focus loss). An empty name cancels.
    fn commit_folder_rename(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.folder_editing.take() {
            let name = self.folder_rename_field.read(cx).text().trim().to_string();
            if !name.is_empty() {
                Self::run_shared_tool(
                    "rename_folder",
                    serde_json::json!({ "folderId": id, "name": name }),
                );
            }
        }
    }

    /// Begin inline rename of a folder tile.
    fn begin_folder_rename(
        &mut self,
        id: &str,
        seed: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.folder_editing = Some(id.to_string());
        let seed = seed.to_string();
        self.folder_rename_field.update(cx, |field, cx| {
            field.set_text(seed, cx);
        });
        window.focus(&self.folder_rename_field.focus_handle(cx), cx);
        cx.notify();
    }

    /// Asset ids in the grid's current display order (folders excluded) —
    /// the ordering shift-click ranges over.
    fn ordered_visible_ids(&self) -> Vec<String> {
        if !self.library.search_active() && self.library.view_mode == LibraryViewMode::Grouped {
            grouped_sections(&self.manifest, &self.library)
                .into_iter()
                .flat_map(|(_, _, entries)| entries.into_iter().map(|e| e.id.clone()))
                .collect()
        } else {
            visible_entries(&self.manifest, &self.library)
                .into_iter()
                .map(|e| e.id.clone())
                .collect()
        }
    }

    /// Click on an asset tile with the mouse-down modifiers applied.
    fn handle_asset_click(&mut self, id: &str, e: &gpui::MouseDownEvent, cx: &mut Context<Self>) {
        self.open_menu = None;
        if e.modifiers.shift {
            let ordered = self.ordered_visible_ids();
            self.library.select_range(&ordered, id);
        } else if e.modifiers.platform || e.modifiers.control {
            self.library.select_toggle(id);
        } else {
            self.library.select_click(id);
        }
        cx.notify();
    }

    /// Escape cancels the folder rename, then an open menu, then the
    /// selection (the rename TextField lets Escape bubble here).
    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key.as_str() != "escape" {
            return;
        }
        if self.folder_editing.take().is_some() || self.open_menu.take().is_some() {
            cx.stop_propagation();
            cx.notify();
        } else if !self.library.selection.is_empty() {
            self.library.clear_selection();
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Focusable for MediaPanelView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Tab button: 26px square (Swift: IconSize.lg = 26).
/// Active: white@10% bg + 2.5px left-edge capsule in BorderColors::PRIMARY
/// (Swift: HoverHighlight(isActive) + Capsule overlay on leading edge).
fn tab_btn(id: &str, label: &str, is_active: bool) -> gpui::Stateful<gpui::Div> {
    let btn_size = IconSize::LG; // 26px
    let bg = if is_active {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.10,
        }
    } else {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        }
    };
    div()
        .id(id.to_string())
        .relative()
        .w(px(btn_size))
        .h(px(btn_size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .bg(bg)
        .text_color(if is_active {
            Text::PRIMARY
        } else {
            Text::TERTIARY
        })
        .text_size(px(FontSize::SM_MD))
        .child(label.to_string())
        // Left-edge accent capsule (Swift: Capsule overlay at topLeading)
        .when(is_active, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(5.0))
                    .w(px(2.5))
                    .h(px(16.0))
                    .rounded_full()
                    .bg(BorderColors::PRIMARY),
            )
        })
}

/// Media library empty state (Swift emptyStateView).
fn media_empty_state() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(Spacing::XS))
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::TITLE_1))
                .child("No media yet"),
        )
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child("Drop files here or import from disk"),
        )
}

/// Owned per-tile data assembled each render.
struct AssetTileData {
    id: String,
    name: String,
    kind: ClipType,
    image: Option<std::path::PathBuf>,
    selected: bool,
}

/// 80×60 thumbnail + name strip (Swift AssetThumbnailView) with a selection
/// ring; click handling is attached by the caller.
fn asset_tile(data: &AssetTileData) -> gpui::Stateful<gpui::Div> {
    let mut thumb = div()
        .w(px(80.0))
        .h(px(60.0))
        .rounded(px(Radius::XS_SM))
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center();
    if let Some(path) = &data.image {
        thumb = thumb.child(
            gpui::img(path.clone())
                .size_full()
                .object_fit(gpui::ObjectFit::Cover),
        );
    } else {
        let hue = crate::media_panel_model::tile_hue(&data.id);
        thumb = thumb
            .bg(gpui::Hsla {
                h: hue,
                s: 0.35,
                l: 0.18,
                a: 1.0,
            })
            .text_color(gpui::Hsla {
                h: hue,
                s: 0.60,
                l: 0.65,
                a: 1.0,
            })
            .text_size(px(FontSize::LG))
            .child(crate::media_panel_model::tile_icon(&data.kind).to_string());
    }
    if data.selected {
        thumb = thumb.border_2().border_color(Accent::PRIMARY);
    }
    div()
        .id(SharedString::from(format!("tile-{}", data.id)))
        .flex()
        .flex_col()
        .w(px(80.0))
        .cursor_pointer()
        .child(thumb)
        .child(
            div()
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_color(if data.selected {
                    Text::PRIMARY
                } else {
                    Text::SECONDARY
                })
                .text_size(px(FontSize::XS))
                .overflow_hidden()
                .child(data.name.clone()),
        )
}

/// 22×22 toolbar icon button (Swift toolbarMenuIcon).
fn toolbar_icon(id: &str, glyph: &str, color: gpui::Hsla) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .w(px(IconSize::MD))
        .h(px(IconSize::MD))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .text_color(color)
        .text_size(px(FontSize::SM))
        .child(glyph.to_string())
}

/// Dropdown row with a leading check column; on_click attached by the caller.
fn menu_row(id: SharedString, label: String, checked: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .px(px(Spacing::MD))
        .py(px(Spacing::XS))
        .cursor_pointer()
        .child(
            div()
                .w(px(IconSize::XXS))
                .text_size(px(FontSize::XS))
                .text_color(Accent::PRIMARY)
                .child(if checked { "✓" } else { "" }),
        )
        .child(
            div()
                .text_size(px(FontSize::SM))
                .text_color(Text::SECONDARY)
                .child(label),
        )
}

fn menu_divider() -> gpui::Div {
    div()
        .h(px(BorderWidth::HAIRLINE))
        .mx(px(Spacing::SM))
        .my(px(Spacing::XXS))
        .bg(BorderColors::SUBTLE)
}

fn section_header(title: &str, count: usize) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .child(
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::SECONDARY)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::MUTED)
                .child(count.to_string()),
        )
}

impl MediaPanelView {
    fn asset_tile_data(&self, e: &MediaManifestEntry) -> AssetTileData {
        let source_path = self
            .state
            .items
            .iter()
            .find(|i| i.id == e.id)
            .and_then(|i| i.source_path.clone());
        let image = match e.r#type {
            ClipType::Image => source_path,
            ClipType::Video => source_path
                .as_deref()
                .and_then(crate::video_thumbnails::request_thumbnail),
            _ => None,
        };
        AssetTileData {
            id: e.id.clone(),
            name: e.name.clone(),
            kind: e.r#type,
            image,
            selected: self.library.selection.iter().any(|s| s == &e.id),
        }
    }

    /// Asset tile with selection mouse handling.
    fn render_asset_tile(
        &self,
        e: &MediaManifestEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let data = self.asset_tile_data(e);
        let id = data.id.clone();
        asset_tile(&data).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.handle_asset_click(&id, ev, cx);
            }),
        )
    }

    /// Folder tile: div-drawn folder glyph, count badge, double-click to
    /// open, double-click the name to rename inline.
    fn render_folder_tile(&self, folder: &MediaFolder, cx: &mut Context<Self>) -> AnyElement {
        let id = folder.id.clone();
        let name = folder.name.clone();
        let count = folder_child_count(&self.manifest, &folder.id);
        let editing = self.folder_editing.as_deref() == Some(folder.id.as_str());
        let open_id = id.clone();
        let rename_id = id.clone();
        let rename_seed = name.clone();
        let accent = gpui::Hsla {
            a: 0.85,
            ..Accent::PRIMARY
        };
        let name_strip: AnyElement = if editing {
            div()
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_size(px(FontSize::XS))
                .text_color(Text::PRIMARY)
                .child(self.folder_rename_field.clone())
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("folder-name-{id}")))
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::XS))
                .overflow_hidden()
                .child(name.clone())
                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    if e.click_count() == 2 {
                        cx.stop_propagation();
                        this.begin_folder_rename(&rename_id, &rename_seed, window, cx);
                    }
                }))
                .into_any_element()
        };
        div()
            .id(SharedString::from(format!("folder-{id}")))
            .flex()
            .flex_col()
            .w(px(80.0))
            .cursor_pointer()
            .child(
                div()
                    .relative()
                    .w(px(80.0))
                    .h(px(60.0))
                    .rounded(px(Radius::XS_SM))
                    .bg(gpui::Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::SUBTLE,
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    // Folder glyph: tab + body, drawn with divs.
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .child(div().w(px(18.0)).h(px(5.0)).rounded(px(Radius::XS)).bg(accent))
                            .child(
                                div()
                                    .w(px(44.0))
                                    .h(px(28.0))
                                    .rounded(px(Radius::XS_SM))
                                    .bg(accent),
                            ),
                    )
                    .when(count > 0, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(Spacing::XS))
                                .right(px(Spacing::XS))
                                .px(px(Spacing::SM))
                                .py(px(Spacing::XXS))
                                .rounded_full()
                                .bg(Background::PROMINENT)
                                .text_size(px(FontSize::XXS))
                                .text_color(Text::PRIMARY)
                                .child(count.to_string()),
                        )
                    }),
            )
            .child(name_strip)
            .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                if e.click_count() == 2 && this.folder_editing.is_none() {
                    this.library.current_folder = Some(open_id.clone());
                    this.library.clear_selection();
                    this.open_menu = None;
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    /// Wrap grid of folder tiles + asset tiles inside the scroll body.
    fn render_wrap_grid(
        &self,
        folders: &[&MediaFolder],
        entries: &[&MediaManifestEntry],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mut grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(Spacing::SM_MD))
            .p(px(Spacing::SM_MD));
        for folder in folders {
            grid = grid.child(self.render_folder_tile(folder, cx));
        }
        for e in entries {
            grid = grid.child(self.render_asset_tile(e, cx));
        }
        grid
    }

    /// Scroll container with background click clearing selection and menus.
    fn grid_scroll(&self, id: &str, content: gpui::Div, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(id.to_string())
            .flex_1()
            .overflow_y_scroll()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    let renaming = this.folder_editing.is_some();
                    if renaming {
                        this.commit_folder_rename(cx);
                    }
                    if renaming || this.open_menu.is_some() || !this.library.selection.is_empty()
                    {
                        this.open_menu = None;
                        this.library.clear_selection();
                        cx.notify();
                    }
                }),
            )
            .child(content)
            .into_any_element()
    }

    /// Search results: name matches under a "Files" header (moment/transcript
    /// sections need a search-index host, not yet wired on the Rust side).
    fn render_search_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let entries = visible_entries(&self.manifest, &self.library);
        let content = if entries.is_empty() {
            div().p(px(Spacing::SM_MD)).child(
                div()
                    .pt(px(Spacing::XL))
                    .w_full()
                    .flex()
                    .justify_center()
                    .text_size(px(FontSize::SM))
                    .text_color(Text::TERTIARY)
                    .child(format!("No matches for \u{201c}{}\u{201d}", self.library.trimmed_query())),
            )
        } else {
            div()
                .flex()
                .flex_col()
                .child(section_header("Files", entries.len()))
                .child(self.render_wrap_grid(&[], &entries, cx))
        };
        self.grid_scroll("media-grid-scroll", content, cx)
    }

    /// Grouped view: Library section + one per folder, ordered by path.
    fn render_grouped(&self, cx: &mut Context<Self>) -> AnyElement {
        let sections = grouped_sections(&self.manifest, &self.library);
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD))
            .p(px(Spacing::SM_MD));
        for (i, (folder_id, title, entries)) in sections.iter().enumerate() {
            let mut header = div()
                .id(SharedString::from(format!("media-group-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(Text::PRIMARY)
                        .child(title.clone()),
                )
                .child(
                    div()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::MUTED)
                        .child(entries.len().to_string()),
                );
            if let Some(fid) = folder_id {
                let open_id = fid.to_string();
                header = header.cursor_pointer().on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.library.view_mode = LibraryViewMode::Folders;
                        this.library.current_folder = Some(open_id.clone());
                        this.library.clear_selection();
                        cx.notify();
                    },
                ));
            }
            let body: AnyElement = if entries.is_empty() {
                div()
                    .py(px(Spacing::SM))
                    .text_size(px(FontSize::XS))
                    .text_color(Text::MUTED)
                    .child("Empty")
                    .into_any_element()
            } else {
                self.render_wrap_grid(&[], entries, cx).into_any_element()
            };
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(header)
                    .child(
                        div()
                            .h(px(BorderWidth::HAIRLINE))
                            .bg(BorderColors::SUBTLE),
                    )
                    .child(body),
            );
        }
        self.grid_scroll("media-grid-scroll", col, cx)
    }

    /// The grid body for the current state.
    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let lib_empty = self.manifest.entries.is_empty() && self.manifest.folders.is_empty();
        if lib_empty {
            return media_empty_state().into_any_element();
        }
        if self.library.search_active() {
            return self.render_search_results(cx);
        }
        match self.library.view_mode {
            LibraryViewMode::Folders => {
                let folders = visible_folders(&self.manifest, &self.library);
                let entries = visible_entries(&self.manifest, &self.library);
                let grid = self.render_wrap_grid(&folders, &entries, cx);
                self.grid_scroll("media-grid-scroll", grid, cx)
            }
            LibraryViewMode::Flat => {
                let entries = visible_entries(&self.manifest, &self.library);
                let grid = self.render_wrap_grid(&[], &entries, cx);
                self.grid_scroll("media-grid-scroll", grid, cx)
            }
            LibraryViewMode::Grouped => self.render_grouped(cx),
        }
    }

    /// Toolbar: actions row, search row, context bar (Swift MediaTab.toolbar).
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_filters = self.library.has_active_filters();
        let clear_visible = !self.library.search_query.is_empty();
        let sel_count = self.library.selection.len();
        let item_count = visible_folders(&self.manifest, &self.library).len()
            + visible_entries(&self.manifest, &self.library).len();
        let in_folder_crumbs =
            self.library.view_mode == LibraryViewMode::Folders && !self.library.search_active();

        // Context path: breadcrumb chips in Folders view, mode title otherwise.
        let context_path: AnyElement = if in_folder_crumbs {
            let mut crumbs: Vec<(Option<String>, String)> = vec![(None, "Library".to_string())];
            for f in folder_path(&self.manifest, self.library.current_folder.as_deref()) {
                crumbs.push((Some(f.id.clone()), f.name.clone()));
            }
            let last = crumbs.len() - 1;
            let mut row = div().flex().flex_row().items_center().gap(px(Spacing::XS));
            for (i, (target, label)) in crumbs.into_iter().enumerate() {
                if i > 0 {
                    row = row.child(
                        div()
                            .text_size(px(FontSize::XXS))
                            .text_color(Text::MUTED)
                            .child("›"),
                    );
                }
                let is_leaf = i == last;
                let mut chip = div()
                    .id(SharedString::from(format!("media-crumb-{i}")))
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XXS))
                    .rounded(px(Radius::XS_SM))
                    .text_size(px(FontSize::XS))
                    .text_color(if is_leaf {
                        Text::PRIMARY
                    } else {
                        Text::TERTIARY
                    })
                    .child(label);
                if !is_leaf {
                    chip = chip.cursor_pointer().on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.library.current_folder = target.clone();
                            this.library.clear_selection();
                            cx.notify();
                        },
                    ));
                }
                row = row.child(chip);
            }
            row.into_any_element()
        } else {
            let title = if self.library.search_active() {
                "Search".to_string()
            } else {
                self.library.view_mode.title().to_string()
            };
            div()
                .text_size(px(FontSize::XS))
                .text_color(Text::PRIMARY)
                .child(title)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .px(px(Spacing::SM))
            .pt(px(Spacing::SM))
            .pb(px(Spacing::XS))
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::SUBTLE)
            // Actions row: Import + Generate + overflow ⋯ | index status
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .child(
                        div()
                            .id("btn-import-media")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .h(px(IconSize::MD_LG))
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child("+ Import"),
                    )
                    .child(
                        div()
                            .id("btn-generate-media")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .h(px(IconSize::MD_LG))
                            .rounded(px(Radius::SM))
                            .bg(Accent::PRIMARY)
                            .cursor_pointer()
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::SM))
                            .child("✦ Generate"),
                    )
                    .child(
                        toolbar_icon("btn-media-overflow", "⋯", Text::TERTIARY).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::Overflow) => None,
                                    _ => Some(ToolbarMenu::Overflow),
                                };
                                cx.notify();
                            }),
                        ),
                    )
                    .child(div().flex_1())
                    .when(!self.search_status.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(FontSize::XS))
                                .text_color(Text::TERTIARY)
                                .child(self.search_status.clone()),
                        )
                    }),
            )
            // Search row: live field + view/sort/filter menu buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .flex_1()
                            .px(px(Spacing::SM_MD))
                            .h(px(IconSize::MD))
                            .rounded_full()
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .bg(gpui::Hsla {
                                h: 0.0,
                                s: 0.0,
                                l: 1.0,
                                a: Opacity::SUBTLE,
                            })
                            .text_size(px(FontSize::XS))
                            .text_color(Text::PRIMARY)
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child("⌕"),
                            )
                            .child(div().flex_1().child(self.search_field.clone()))
                            .when(clear_visible, |el| {
                                el.child(
                                    div()
                                        .id("media-search-clear")
                                        .cursor_pointer()
                                        .text_size(px(FontSize::XS))
                                        .text_color(Text::MUTED)
                                        .child("✕")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.search_field.update(cx, |field, cx| {
                                                field.set_text("", cx);
                                            });
                                            this.library.search_query.clear();
                                            cx.notify();
                                        })),
                                )
                            }),
                    )
                    .child(
                        toolbar_icon("btn-media-view", "⊞", Text::TERTIARY).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::View) => None,
                                    _ => Some(ToolbarMenu::View),
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        toolbar_icon("btn-media-sort", "↕", Text::TERTIARY).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.open_menu = match this.open_menu {
                                    Some(ToolbarMenu::Sort) => None,
                                    _ => Some(ToolbarMenu::Sort),
                                };
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        toolbar_icon(
                            "btn-media-filter",
                            "≡",
                            if has_filters {
                                Accent::PRIMARY
                            } else {
                                Text::TERTIARY
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_menu = match this.open_menu {
                                Some(ToolbarMenu::Filter) => None,
                                _ => Some(ToolbarMenu::Filter),
                            };
                            cx.notify();
                        })),
                    ),
            )
            // Context bar: breadcrumb/mode title | Delete (n) + item count
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .h(px(MediaPanel::CONTEXT_ROW_HEIGHT))
                    .child(div().flex_1().child(context_path))
                    .when(sel_count > 0, |el| {
                        el.child(
                            div()
                                .id("media-delete-selected")
                                .px(px(Spacing::SM))
                                .py(px(Spacing::XXS))
                                .rounded(px(Radius::SM))
                                .bg(Background::PROMINENT)
                                .cursor_pointer()
                                .text_size(px(FontSize::XS))
                                .text_color(Text::PRIMARY)
                                .child(format!("Delete ({sel_count})"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.delete_selection(cx);
                                })),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(Text::MUTED)
                            .child(if item_count == 1 {
                                "1 item".to_string()
                            } else {
                                format!("{item_count} items")
                            }),
                    ),
            )
    }

    /// Open dropdown, absolutely positioned over the grid (painted last so it
    /// stacks above the body).
    fn render_menu_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.open_menu?;
        let actions_bottom = Spacing::SM + Layout::PANEL_HEADER_HEIGHT + Spacing::XXS;
        let search_bottom =
            Spacing::SM + Layout::PANEL_HEADER_HEIGHT * 2.0 + Spacing::XS + Spacing::XXS;
        let mut panel = div()
            .id("media-menu")
            .absolute()
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .rounded(px(Radius::SM))
            .flex()
            .flex_col()
            .py(px(Spacing::XS));
        panel = match menu {
            ToolbarMenu::Overflow => panel.top(px(actions_bottom)).left(px(Spacing::SM)),
            _ => panel.top(px(search_bottom)).right(px(Spacing::SM)),
        };
        panel = match menu {
            ToolbarMenu::View => {
                let mut p = panel;
                for (i, mode) in LibraryViewMode::all().into_iter().enumerate() {
                    let checked = self.library.view_mode == mode;
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-view-{i}")),
                            mode.title().to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.view_mode = mode;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p
            }
            ToolbarMenu::Sort => {
                let mut p = panel;
                for (i, key) in LibrarySortKey::all().into_iter().enumerate() {
                    let checked = self.library.sort_key == key;
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-sort-{i}")),
                            key.title().to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.sort_key = key;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p
            }
            ToolbarMenu::Filter => {
                let mut p = panel;
                // Only types a MediaAsset can carry (Swift filterableTypes).
                for (i, (t, label)) in [
                    (ClipType::Video, "Video"),
                    (ClipType::Audio, "Audio"),
                    (ClipType::Image, "Image"),
                ]
                .into_iter()
                .enumerate()
                {
                    let checked = self.library.type_filter.contains(&t);
                    p = p.child(
                        menu_row(
                            SharedString::from(format!("media-filter-{i}")),
                            label.to_string(),
                            checked,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.library.toggle_type_filter(t);
                            this.open_menu = None;
                            cx.notify();
                        })),
                    );
                }
                p.child(menu_divider())
                    .child(
                        menu_row(
                            SharedString::from("media-filter-ai"),
                            "AI Generated".to_string(),
                            self.library.filter_ai,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library.filter_ai = !this.library.filter_ai;
                            this.open_menu = None;
                            cx.notify();
                        })),
                    )
                    .child(menu_divider())
                    .child(
                        menu_row(
                            SharedString::from("media-filter-clear"),
                            "Clear Filters".to_string(),
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.library.clear_filters();
                            this.open_menu = None;
                            cx.notify();
                        })),
                    )
            }
            ToolbarMenu::Overflow => panel.child(
                menu_row(
                    SharedString::from("media-new-folder"),
                    "New Folder".to_string(),
                    false,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_menu = None;
                    this.create_folder_in_current(window, cx);
                })),
            ),
        };
        Some(panel.into_any_element())
    }

    /// The whole Media tab: toolbar + body + generation strip + menu overlay.
    fn render_media_tab(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("media-tab-root")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(self.render_toolbar(cx))
            .child(self.render_body(cx))
            // GenerationView anchored to BOTTOM with padding (Swift:
            // .padding(.horizontal, sm).padding(.bottom, sm))
            .child(
                div()
                    .px(px(Spacing::SM))
                    .pb(px(Spacing::SM))
                    .child(self.generation.clone()),
            )
            .children(self.render_menu_overlay(cx))
            .into_any_element()
    }
}

fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XXS))
        .child(text.to_uppercase())
}

fn row_value(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(28.0))
        .px(px(Spacing::MD_LG))
        .child(
            div()
                .flex_1()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(value.to_string()),
        )
}

fn generate_btn(id: &str) -> impl IntoElement {
    use crate::theme::Accent;
    div()
        .id(id.to_string())
        .w_full()
        .h(px(32.0))
        .rounded(px(crate::theme::Radius::SM))
        .bg(Accent::PRIMARY)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(Background::BASE)
        .text_size(px(FontSize::SM))
        .child("Generate")
}

/// Captions tab: Source, Style, and Placement sections + Generate button.
fn captions_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(Background::SURFACE)
        .child(
            div()
                .id("captions-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::MD))
                .gap(px(Spacing::LG))
                // Source section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Source"))
                        .child(row_value("Input", "Auto")),
                )
                // Style section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Style"))
                        .child(row_value("Font Size", "36"))
                        .child(row_value("Case", "Auto"))
                        .child(row_value("Censor Profanity", "Off")),
                )
                // Placement
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Placement"))
                        .child(row_value("Position", "Bottom Center")),
                ),
        )
        // Generate bar at bottom (matches Swift generateBar)
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::SM_MD))
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .child(generate_btn("btn-gen-captions")),
        )
}

/// Music tab: Source, Model, Prompt, Duration + Generate button.
fn music_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(Background::SURFACE)
        .child(
            div()
                .id("music-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::MD))
                .gap(px(Spacing::LG))
                // Source section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Source"))
                        .child(row_value("Input", "Video to Music"))
                        .child(row_value("Video", "Whole timeline")),
                )
                // Model
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Model"))
                        .child(row_value("Model", "ElevenLabs Music ⌄")),
                )
                // Prompt area
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Prompt"))
                        .child(
                            div()
                                .h(px(80.0))
                                .rounded(px(crate::theme::Radius::SM))
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                .bg(Background::RAISED)
                                .px(px(Spacing::SM_MD))
                                .py(px(Spacing::SM))
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::SM))
                                .child("Describe the music…"),
                        ),
                ),
        )
        // Generate bar at bottom
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::SM_MD))
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .child(generate_btn("btn-gen-music")),
        )
}

impl Render for MediaPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sync_from_shared_state() {
            cx.notify();
        }
        let active = self.state.active_tab.clone();
        let media_active = active == MediaPanelTab::Media;
        let captions_active = active == MediaPanelTab::Captions;
        let music_active = active == MediaPanelTab::Music;

        div()
            .id("media-panel")
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::SURFACE)
            // ── Left tab rail ──
            .child(
                div()
                    .id("tab-rail-container")
                    .flex()
                    .flex_row()
                    .h_full()
                    .child(
                        div()
                            .id("tab-rail")
                            .flex()
                            .flex_col()
                            .items_center()
                            .w(px(MediaPanel::TAB_RAIL_WIDTH))
                            .h_full()
                            .pt(px(Spacing::SM))
                            .pb(px(Spacing::SM))
                            .gap(px(Spacing::XS))
                            .bg(Background::RAISED)
                            .child(
                                tab_btn("tab-media", "M", media_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Media, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Media".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-captions", "C", captions_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Captions, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Captions".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-music", "♪", music_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Music, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Music".into(),
                                        })
                                        .into()
                                    }),
                            ),
                    )
                    // Hairline border separator
                    .child(div().w(px(1.0)).h_full().bg(BorderColors::PRIMARY)),
            )
            // ── Tab content area ──
            .child(
                div()
                    .id("tab-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(Background::SURFACE)
                    .child(match active {
                        MediaPanelTab::Media => self.render_media_tab(cx),
                        MediaPanelTab::Captions => captions_tab_content().into_any_element(),
                        MediaPanelTab::Music => music_tab_content().into_any_element(),
                    }),
            )
    }
}

#[cfg(test)]
mod library_tests {
    use super::*;

    /// Library fixture: root has A-roll.mp4 + music.wav; folder f1 "Shoot"
    /// holds B-roll.mp4 + Sunset.png (AI-generated); f2 "Nested" is inside f1
    /// and holds take.mov.
    fn manifest() -> MediaManifest {
        serde_json::from_str(
            r#"{"version":1,
                "entries":[
                    {"id":"m1","name":"A-roll.mp4","type":"video","source":{"project":{"relativePath":"media/a.mp4"}},"duration":5.0},
                    {"id":"m2","name":"music.wav","type":"audio","source":{"project":{"relativePath":"media/b.wav"}},"duration":30.0},
                    {"id":"m3","name":"B-roll.mp4","type":"video","source":{"project":{"relativePath":"media/c.mp4"}},"duration":9.0,"folderId":"f1"},
                    {"id":"m4","name":"Sunset.png","type":"image","source":{"project":{"relativePath":"media/d.png"}},"duration":0.0,"folderId":"f1",
                     "generationInput":{"prompt":"sunset","model":"m","duration":5,"aspectRatio":"16:9"}},
                    {"id":"m5","name":"take.mov","type":"video","source":{"project":{"relativePath":"media/e.mov"}},"duration":2.0,"folderId":"f2"}
                ],
                "folders":[
                    {"id":"f1","name":"Shoot"},
                    {"id":"f2","name":"Nested","parentFolderId":"f1"}
                ]}"#,
        )
        .unwrap()
    }

    fn ids(entries: &[&MediaManifestEntry]) -> Vec<String> {
        entries.iter().map(|e| e.id.clone()).collect()
    }

    fn state() -> LibraryState {
        LibraryState::default()
    }

    // ── 1.1 visible_entries: search dimension ──

    #[test]
    fn search_filters_by_name_substring_and_clear_restores() {
        let m = manifest();
        let mut s = state();
        s.search_query = "roll".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3"]);
        s.search_query.clear();
        // Folders view, root bucket after clearing.
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2"]);
    }

    #[test]
    fn search_is_case_insensitive_and_trims() {
        let m = manifest();
        let mut s = state();
        s.search_query = "  B-ROLL  ".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m3"]);
        s.search_query = "   ".into();
        assert!(!s.search_active(), "whitespace-only query is not a search");
    }

    #[test]
    fn search_spans_all_folders_even_inside_one() {
        let m = manifest();
        let mut s = state();
        s.current_folder = Some("f1".into());
        s.search_query = "a".into();
        let got = ids(&visible_entries(&m, &s));
        assert!(
            got.contains(&"m1".to_string()),
            "root asset found from inside f1"
        );
        assert!(got.contains(&"m5".to_string()), "nested asset found too");
    }

    // ── 1.1 visible_entries: filter dimension ──

    #[test]
    fn type_filter_restricts_and_toggle_roundtrips() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.toggle_type_filter(ClipType::Audio);
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2"]);
        s.toggle_type_filter(ClipType::Video);
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2", "m3", "m5"]);
        s.toggle_type_filter(ClipType::Audio);
        s.toggle_type_filter(ClipType::Video);
        assert!(!s.has_active_filters(), "toggling off clears the filter");
        assert_eq!(visible_entries(&m, &s).len(), 5);
    }

    #[test]
    fn ai_filter_keeps_generated_only() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.filter_ai = true;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m4"]);
        assert!(s.has_active_filters());
    }

    #[test]
    fn filters_and_search_combine_with_and() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.type_filter = vec![ClipType::Video];
        s.search_query = "roll".into();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3"]);
        s.filter_ai = true;
        assert!(visible_entries(&m, &s).is_empty());
    }

    #[test]
    fn clear_filters_resets_types_and_ai() {
        let mut s = state();
        s.type_filter = vec![ClipType::Audio];
        s.filter_ai = true;
        s.clear_filters();
        assert!(!s.has_active_filters());
        assert!(s.type_filter.is_empty());
        assert!(!s.filter_ai);
    }

    // ── 1.1 visible_entries: folder scope dimension ──

    #[test]
    fn folders_mode_scopes_to_current_folder() {
        let m = manifest();
        let mut s = state();
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2"], "root bucket");
        s.current_folder = Some("f1".into());
        assert_eq!(ids(&visible_entries(&m, &s)), ["m3", "m4"]);
        s.current_folder = Some("f2".into());
        assert_eq!(ids(&visible_entries(&m, &s)), ["m5"]);
    }

    #[test]
    fn flat_mode_spans_library_and_hides_folders() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.current_folder = Some("f1".into());
        assert_eq!(visible_entries(&m, &s).len(), 5, "folder scope ignored");
        assert!(visible_folders(&m, &s).is_empty(), "no folder tiles in flat");
    }

    #[test]
    fn visible_folders_lists_current_subfolders_only() {
        let m = manifest();
        let mut s = state();
        let root: Vec<&str> = visible_folders(&m, &s)
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(root, ["f1"]);
        s.current_folder = Some("f1".into());
        let inner: Vec<&str> = visible_folders(&m, &s)
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(inner, ["f2"]);
        s.search_query = "Shoot".into();
        assert!(
            visible_folders(&m, &s).is_empty(),
            "search view has no folder tiles"
        );
    }

    // ── 1.1 visible_entries: sort dimension ──

    #[test]
    fn sort_name_is_case_insensitive() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Name;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m3", "m2", "m4", "m5"]);
    }

    #[test]
    fn sort_date_added_keeps_manifest_order() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::DateAdded;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m1", "m2", "m3", "m4", "m5"]);
    }

    #[test]
    fn sort_duration_is_descending() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Duration;
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2", "m3", "m1", "m5", "m4"]);
    }

    #[test]
    fn sort_type_groups_by_kind() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Flat;
        s.sort_key = LibrarySortKey::Type;
        // audio < image < video; stable within a kind.
        assert_eq!(ids(&visible_entries(&m, &s)), ["m2", "m4", "m1", "m3", "m5"]);
    }

    // ── 1.1 grouped sections + folder helpers ──

    #[test]
    fn grouped_sections_cover_root_and_folders_by_path() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Grouped;
        let sections = grouped_sections(&m, &s);
        let titles: Vec<&str> = sections.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Library", "Shoot", "Shoot / Nested"]);
        assert_eq!(ids(&sections[0].2), ["m1", "m2"]);
        assert_eq!(ids(&sections[1].2), ["m3", "m4"]);
        assert_eq!(ids(&sections[2].2), ["m5"]);
    }

    #[test]
    fn grouped_sections_skip_empty_root_and_filter_buckets() {
        let m = manifest();
        let mut s = state();
        s.view_mode = LibraryViewMode::Grouped;
        s.type_filter = vec![ClipType::Image];
        let sections = grouped_sections(&m, &s);
        let titles: Vec<&str> = sections.iter().map(|(_, t, _)| t.as_str()).collect();
        assert_eq!(
            titles,
            ["Shoot", "Shoot / Nested"],
            "empty root section skipped"
        );
        assert_eq!(ids(&sections[0].2), ["m4"]);
        assert!(sections[1].2.is_empty(), "empty folder sections stay visible");
    }

    #[test]
    fn folder_path_walks_to_root_and_survives_cycles() {
        let m = manifest();
        let path: Vec<&str> = folder_path(&m, Some("f2"))
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        assert_eq!(path, ["f1", "f2"]);
        assert!(folder_path(&m, None).is_empty());
        assert!(folder_path(&m, Some("ghost")).is_empty());

        let cyclic: MediaManifest = serde_json::from_str(
            r#"{"version":1,"entries":[],"folders":[
                {"id":"a","name":"A","parentFolderId":"b"},
                {"id":"b","name":"B","parentFolderId":"a"}]}"#,
        )
        .unwrap();
        let _ = folder_path(&cyclic, Some("a")); // must terminate
    }

    #[test]
    fn folder_child_count_sums_subfolders_and_assets() {
        let m = manifest();
        assert_eq!(folder_child_count(&m, "f1"), 3, "f2 + m3 + m4");
        assert_eq!(folder_child_count(&m, "f2"), 1);
        assert_eq!(folder_child_count(&m, "ghost"), 0);
    }

    // ── 1.2 selection ──

    fn ordered() -> Vec<String> {
        ["m1", "m2", "m3", "m4", "m5"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn click_replaces_selection_and_sets_anchor() {
        let mut s = state();
        s.selection = vec!["m1".into(), "m2".into()];
        s.select_click("m3");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m3"));
    }

    #[test]
    fn toggle_adds_and_removes() {
        let mut s = state();
        s.select_toggle("m1");
        s.select_toggle("m3");
        assert_eq!(s.selection, ["m1", "m3"]);
        s.select_toggle("m1");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(
            s.selection_anchor.as_deref(),
            Some("m1"),
            "toggle moves the anchor"
        );
    }

    #[test]
    fn range_selects_inclusive_span_in_order() {
        let mut s = state();
        s.select_click("m2");
        s.select_range(&ordered(), "m5");
        assert_eq!(s.selection, ["m2", "m3", "m4", "m5"]);
    }

    #[test]
    fn range_works_backwards() {
        let mut s = state();
        s.select_click("m4");
        s.select_range(&ordered(), "m1");
        assert_eq!(s.selection, ["m1", "m2", "m3", "m4"]);
    }

    #[test]
    fn range_reextends_from_same_anchor() {
        let mut s = state();
        s.select_click("m2");
        s.select_range(&ordered(), "m5");
        s.select_range(&ordered(), "m3");
        assert_eq!(
            s.selection,
            ["m2", "m3"],
            "second shift-click re-extends from anchor"
        );
        assert_eq!(s.selection_anchor.as_deref(), Some("m2"));
    }

    #[test]
    fn range_without_anchor_falls_back_to_click() {
        let mut s = state();
        s.select_range(&ordered(), "m3");
        assert_eq!(s.selection, ["m3"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m3"));
    }

    #[test]
    fn range_with_vanished_anchor_falls_back_to_click() {
        let mut s = state();
        s.select_click("ghost");
        s.select_range(&ordered(), "m2");
        assert_eq!(s.selection, ["m2"]);
        assert_eq!(s.selection_anchor.as_deref(), Some("m2"));
    }

    #[test]
    fn clear_selection_resets_ids_and_anchor() {
        let mut s = state();
        s.select_click("m1");
        s.clear_selection();
        assert!(s.selection.is_empty());
        assert!(s.selection_anchor.is_none());
    }
}
