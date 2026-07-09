#[cfg(feature = "desktop-app")]
pub mod account_view;
pub mod agent_bridge;
#[cfg(feature = "desktop-app")]
pub mod agent_panel_view;
#[cfg(feature = "desktop-app")]
pub mod ai_edit_tab_view;
pub mod anthropic_transport;
#[cfg(feature = "desktop-app")]
pub mod app_root;
#[cfg(feature = "desktop-app")]
pub mod assets;
pub mod audio_export;
pub mod audio_source;
pub mod export_host;
pub mod project_lister;
pub mod project_navigator;
#[cfg(feature = "desktop-app")]
pub mod chat_history_view;
#[cfg(feature = "desktop-app")]
pub mod chat_view;
#[cfg(feature = "desktop-app")]
pub mod crop_overlay_view;
pub mod editor_state_hub;
#[cfg(feature = "desktop-app")]
pub mod editor_view;
pub mod export_model;
#[cfg(feature = "desktop-app")]
pub mod export_view;
#[cfg(feature = "desktop-app")]
pub mod feedback_view;
#[cfg(feature = "desktop-app")]
pub mod generation_view;
#[cfg(feature = "desktop-app")]
pub mod help_view;
pub mod home_model;
#[cfg(feature = "desktop-app")]
pub mod home_view;
pub mod inspector_model;
#[cfg(feature = "desktop-app")]
pub mod inspector_view;
#[cfg(feature = "desktop-app")]
pub mod keyframes_view;
pub mod matte_writer;
pub mod mcp_service;
pub mod media_import;
pub mod media_panel_model;
#[cfg(feature = "desktop-app")]
pub mod media_panel_view;
#[cfg(feature = "desktop-app")]
pub mod mention_popover_view;
pub mod menu;
pub mod multi_session;
pub mod pane;
pub mod platform_adapter;
pub mod preview_guides;
pub mod preview_model;
pub mod preview_render;
#[cfg(feature = "desktop-app")]
pub mod preview_view;
#[cfg(feature = "desktop-app")]
pub mod project_activity_view;
pub mod project_registry_store;
#[cfg(feature = "desktop-app")]
pub mod settings_mismatch_view;
#[cfg(feature = "desktop-app")]
pub mod settings_view;
pub mod skill_store;
#[cfg(feature = "desktop-app")]
pub mod text_field;
#[cfg(feature = "desktop-app")]
pub mod text_input;
#[cfg(feature = "desktop-app")]
pub mod theme;
pub mod timeline_model;
#[cfg(feature = "desktop-app")]
pub mod timeline_view;
#[cfg(feature = "desktop-app")]
pub mod titlebar_view;
pub mod toolbar_model;
#[cfg(feature = "desktop-app")]
pub mod toolbar_view;
#[cfg(feature = "desktop-app")]
pub mod tour_overlay_view;
#[cfg(feature = "desktop-app")]
pub mod transform_overlay_view;
#[cfg(feature = "desktop-app")]
pub mod update_overlay_view;
pub mod video_export;
pub mod video_thumbnails;
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
pub const SHELL_HEADLINE: &str = "Cross-platform Rust editor shell";
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

    #[test]
    fn shell_headline_is_not_future_tense_scaffolding_copy() {
        assert!(!SHELL_HEADLINE.contains("rewrite"));
        assert!(!SHELL_HEADLINE.contains("scaffold"));
    }
}
