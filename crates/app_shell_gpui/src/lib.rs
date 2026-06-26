#[cfg(feature = "desktop-app")]
pub mod agent_panel_view;
#[cfg(feature = "desktop-app")]
pub mod app_root;
#[cfg(feature = "desktop-app")]
pub mod chat_view;
#[cfg(feature = "desktop-app")]
pub mod editor_view;
#[cfg(feature = "desktop-app")]
pub mod feedback_view;
#[cfg(feature = "desktop-app")]
pub mod help_view;
pub mod home_model;
#[cfg(feature = "desktop-app")]
pub mod home_view;
pub mod menu;
pub mod pane;
pub mod platform_adapter;
#[cfg(feature = "desktop-app")]
pub mod settings_view;
pub mod window;

#[cfg(feature = "desktop-app")]
pub use app_root::{open_main_window, AppRoot};
pub use home_model::{HomeAction, HomeLayout, ProjectCard};
pub use menu::{
    all_menus, all_shortcuts, route_shortcut, MenuAction, MenuGroup, Modifiers, Shortcut,
};
pub use pane::{LayoutPreset, PaneId, PaneLayout, PaneVisibility};
pub use platform_adapter::{NoopPlatformAdapter, PlatformAdapter};
pub use window::{WindowConfig, WindowKind};

pub const APP_NAME: &str = "Fronda";
pub const SHELL_HEADLINE: &str = "Rust rewrite scaffold";
pub const SHELL_STATUS: &str = "Palmier Pro compatibility baseline active";

pub fn launch_status_lines() -> [&'static str; 3] {
    [APP_NAME, SHELL_HEADLINE, SHELL_STATUS]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_status_lines_begin_with_product_name() {
        let lines = launch_status_lines();
        assert_eq!(lines[0], APP_NAME);
        assert_eq!(lines[1], SHELL_HEADLINE);
        assert_eq!(lines[2], SHELL_STATUS);
    }
}
