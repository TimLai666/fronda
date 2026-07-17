//! Native macOS application menu, translated from the shared `menu.rs`
//! definition. Registered at boot via `App::set_menus`; every item
//! dispatches the same `RunMenuAction` values as the non-macOS title-bar
//! menu, so both platforms share one dispatch path.

use gpui::{Menu, MenuItem};

use crate::global_shortcuts::RunMenuAction;
use crate::menu::{self, MenuAction, MenuGroup};

/// The five top-level menus (Fronda / File / Edit / View / Help) with Swift
/// `MainMenu.swift`'s separator grouping; the View menu nests Layout as a
/// submenu with short labels.
pub fn native_menus() -> Vec<Menu> {
    [
        MenuGroup::App,
        MenuGroup::File,
        MenuGroup::Edit,
        MenuGroup::View,
        MenuGroup::Help,
    ]
    .into_iter()
    .map(|group| Menu::new(group.label()).items(group_items(group)))
    .collect()
}

fn group_items(group: MenuGroup) -> Vec<MenuItem> {
    let mut items = Vec::new();
    for (i, section) in menu::menu_sections(group).into_iter().enumerate() {
        if i > 0 {
            items.push(MenuItem::separator());
        }
        if section.iter().all(is_layout_action) {
            items.push(MenuItem::submenu(Menu::new("Layout").items(
                section.into_iter().map(|action| {
                    MenuItem::action(action.short_label(), RunMenuAction { action })
                }),
            )));
        } else {
            for action in section {
                items.push(MenuItem::action(action.label(), RunMenuAction { action }));
            }
        }
    }
    items
}

fn is_layout_action(action: &MenuAction) -> bool {
    matches!(
        action,
        MenuAction::LayoutDefault | MenuAction::LayoutMedia | MenuAction::LayoutVertical
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_dispatches(item: &MenuItem, expected: MenuAction, expected_name: &str) {
        match item {
            MenuItem::Action { name, action, .. } => {
                assert_eq!(name.as_ref(), expected_name);
                assert!(
                    action.partial_eq(&RunMenuAction {
                        action: expected.clone()
                    }),
                    "item {expected_name:?} does not dispatch RunMenuAction {expected:?}"
                );
            }
            _ => panic!("expected an action item for {expected:?}"),
        }
    }

    fn collect_actions(items: &[MenuItem], out: &mut Vec<MenuAction>) {
        for item in items {
            match item {
                MenuItem::Action { action, .. } => {
                    for candidate in menu::all_menus()
                        .into_iter()
                        .flat_map(|(_, actions)| actions)
                    {
                        if action.partial_eq(&RunMenuAction {
                            action: candidate.clone(),
                        }) {
                            out.push(candidate);
                            break;
                        }
                    }
                }
                MenuItem::Submenu(submenu) => collect_actions(&submenu.items, out),
                MenuItem::Separator | MenuItem::SystemMenu(_) => {}
            }
        }
    }

    #[test]
    fn five_groups_in_swift_order() {
        let names: Vec<String> = native_menus()
            .iter()
            .map(|m| m.name.to_string())
            .collect();
        assert_eq!(names, ["Fronda", "File", "Edit", "View", "Help"]);
    }

    #[test]
    fn separators_match_swift_section_boundaries() {
        for (menu, group) in native_menus().iter().zip([
            MenuGroup::App,
            MenuGroup::File,
            MenuGroup::Edit,
            MenuGroup::View,
            MenuGroup::Help,
        ]) {
            let separators = menu
                .items
                .iter()
                .filter(|i| matches!(i, MenuItem::Separator))
                .count();
            assert_eq!(
                separators,
                menu::menu_sections(group).len() - 1,
                "separator count diverges from Swift sections in {group:?}"
            );
        }
    }

    #[test]
    fn every_item_dispatches_the_shared_menu_action() {
        // Flattened dispatch payloads must equal the shared definition,
        // group by group, in order — one RunMenuAction per menu action.
        for (menu, (group, actions)) in native_menus().iter().zip(menu::all_menus()) {
            let mut dispatched = Vec::new();
            collect_actions(&menu.items, &mut dispatched);
            assert_eq!(dispatched, actions, "dispatch payloads diverge in {group:?}");
        }
    }

    #[test]
    fn view_layout_is_a_submenu_with_short_labels() {
        let menus = native_menus();
        let view = &menus[3];
        let submenu = view
            .items
            .iter()
            .find_map(|item| match item {
                MenuItem::Submenu(m) => Some(m),
                _ => None,
            })
            .expect("View must contain the Layout submenu");
        assert_eq!(submenu.name.as_ref(), "Layout");
        assert_eq!(submenu.items.len(), 3);
        assert_dispatches(&submenu.items[0], MenuAction::LayoutDefault, "Default");
        assert_dispatches(&submenu.items[1], MenuAction::LayoutMedia, "Media");
        assert_dispatches(&submenu.items[2], MenuAction::LayoutVertical, "Vertical");
    }

    #[test]
    fn file_menu_structure_matches_swift() {
        let menus = native_menus();
        let file = &menus[1];
        assert_dispatches(&file.items[0], MenuAction::NewProject, "New Project");
        assert_dispatches(&file.items[1], MenuAction::OpenProject, "Open Project…");
        assert!(matches!(file.items[2], MenuItem::Separator));
        assert_dispatches(&file.items[3], MenuAction::SaveProject, "Save Project");
        assert_dispatches(&file.items[4], MenuAction::SaveProjectAs, "Save Project As…");
        assert!(matches!(file.items[5], MenuItem::Separator));
        assert_dispatches(&file.items[6], MenuAction::ImportMedia, "Import Media…");
        assert_dispatches(&file.items[7], MenuAction::ImportTimeline, "Import Timeline…");
        assert!(matches!(file.items[8], MenuItem::Separator));
        assert_dispatches(&file.items[9], MenuAction::Export, "Export…");
        assert_eq!(file.items.len(), 10);
    }
}
