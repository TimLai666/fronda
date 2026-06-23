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

        // All no-ops — reached here without panic
    }
}
