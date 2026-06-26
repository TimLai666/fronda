/// Platform operations that differ across operating systems.
///
/// Platform-specific behavior is wrapped behind this trait so that core logic
/// does not depend on platform APIs directly.
pub trait PlatformAdapter {
    fn show_file_picker(&self) -> Option<String>;
    fn show_save_dialog(&self, default_name: &str) -> Option<String>;
    fn reveal_in_file_manager(&self, path: &str);
    fn trash_file(&self, path: &str) -> Result<(), String>;
    fn notify(&self, title: &str, body: &str);
    fn open_url(&self, url: &str);
    fn clipboard_text(&self) -> Option<String>;
    fn set_clipboard_text(&self, text: &str);

    /// APP-002: Whether the platform should show the Home window when the user
    /// reopens the app (e.g. macOS Dock click) with no visible windows.
    ///
    /// The macOS platform adapter hooks `applicationShouldHandleReopen` and
    /// calls `open_main_window(cx)` when this returns `true`.
    fn should_reopen_to_home(&self) -> bool {
        true
    }

    /// APP-006: Activate the app and reveal a generated asset by its ID in
    /// the best matching open project.
    ///
    /// Called when the user taps a generation-complete notification banner.
    /// On platforms without notification support this is a no-op.
    fn reveal_generated_asset(&self, _asset_id: &str) {}
}

/// Adapter that performs no platform operations.
///
/// Useful for cross-platform compilation without platform dependencies,
/// headless testing, and environments where platform APIs are unavailable.
pub struct NoopPlatformAdapter;

impl PlatformAdapter for NoopPlatformAdapter {
    fn show_file_picker(&self) -> Option<String> {
        None
    }

    fn show_save_dialog(&self, _default_name: &str) -> Option<String> {
        None
    }

    fn reveal_in_file_manager(&self, _path: &str) {}

    fn trash_file(&self, _path: &str) -> Result<(), String> {
        Ok(())
    }

    fn notify(&self, _title: &str, _body: &str) {}

    fn open_url(&self, _url: &str) {}

    fn clipboard_text(&self) -> Option<String> {
        None
    }

    fn set_clipboard_text(&self, _text: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_adapter_trait_defined() {
        fn takes_adapter(_: &dyn PlatformAdapter) {}
        let adapter = NoopPlatformAdapter;
        // Verifies the trait object is usable
        takes_adapter(&adapter);
    }

    #[test]
    fn noop_adapter_no_panic() {
        let a = NoopPlatformAdapter;

        let _ = a.show_file_picker();
        let _ = a.show_save_dialog("test.txt");
        a.reveal_in_file_manager("/some/path");
        let _ = a.trash_file("/some/path");
        a.notify("title", "body");
        a.open_url("https://example.com");
        let _ = a.clipboard_text();
        a.set_clipboard_text("hello");
        a.reveal_generated_asset("asset-123");

        // All no-ops — reached here without panic
    }

    #[test]
    fn app_002_noop_adapter_reopen_default_true() {
        let a = NoopPlatformAdapter;
        assert!(a.should_reopen_to_home());
    }

    #[test]
    fn app_006_reveal_generated_asset_no_panic() {
        let a = NoopPlatformAdapter;
        a.reveal_generated_asset("gen-asset-abc");
    }
}
