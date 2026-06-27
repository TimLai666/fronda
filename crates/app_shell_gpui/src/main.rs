use app_shell_gpui::app_root::open_main_window;
use app_shell_gpui::assets::FrondaAssets;
use gpui::App;

/// The Fronda desktop app entry point.
///
/// Boot sequence (BOOT-001):
/// 1. Initialize gpui platform with embedded SVG assets
/// 2. Open main window with AppRoot (starts at Home)
/// 3. Activate the app
fn main() {
    gpui_platform::application()
        .with_assets(FrondaAssets)
        .run(move |cx: &mut App| {
            open_main_window(cx);
        });
}
