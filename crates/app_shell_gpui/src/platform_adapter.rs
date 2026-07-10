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

/// The program + args to reveal `path` in the OS file manager, selecting the
/// file where the platform supports it. `os` is `std::env::consts::OS`. Pure so
/// the argv is unit-tested for every platform; the spawn is platform I/O.
pub fn reveal_argv(path: &std::path::Path, os: &str) -> (String, Vec<String>) {
    match os {
        "windows" => (
            "explorer".into(),
            vec![format!("/select,{}", path.display())],
        ),
        "macos" => ("open".into(), vec!["-R".into(), path.display().to_string()]),
        // Linux/other: no portable "select", so open the containing folder.
        _ => {
            let dir = path.parent().unwrap_or(path);
            ("xdg-open".into(), vec![dir.display().to_string()])
        }
    }
}

/// Reveal `path` in the OS file manager (best-effort; ignores spawn failure).
pub fn reveal_in_file_manager(path: &std::path::Path) {
    let (program, args) = reveal_argv(path, std::env::consts::OS);
    let _ = std::process::Command::new(program).args(args).spawn();
}

/// The program + args to open `url` in the OS default browser. `os` is
/// `std::env::consts::OS`. Pure so the argv is unit-tested per platform; the
/// spawn is platform I/O.
pub fn open_url_argv(url: &str, os: &str) -> (String, Vec<String>) {
    match os {
        // `explorer <url>` hands http(s) URLs to the default browser (it exits
        // non-zero even on success, but the spawn result is ignored).
        "windows" => ("explorer".into(), vec![url.to_string()]),
        "macos" => ("open".into(), vec![url.to_string()]),
        _ => ("xdg-open".into(), vec![url.to_string()]),
    }
}

/// Open `url` in the OS default browser (best-effort; ignores spawn failure).
pub fn open_url(url: &str) {
    let (program, args) = open_url_argv(url, std::env::consts::OS);
    let _ = std::process::Command::new(program).args(args).spawn();
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

    #[test]
    fn reveal_argv_per_platform() {
        let p = std::path::Path::new("/media/out.mp4");
        assert_eq!(
            reveal_argv(p, "windows"),
            ("explorer".into(), vec!["/select,/media/out.mp4".into()])
        );
        assert_eq!(
            reveal_argv(p, "macos"),
            ("open".into(), vec!["-R".into(), "/media/out.mp4".into()])
        );
        // Linux opens the containing folder.
        assert_eq!(
            reveal_argv(p, "linux"),
            ("xdg-open".into(), vec!["/media".into()])
        );
    }

    #[test]
    fn open_url_argv_per_platform() {
        let url = "https://github.com/TimLai666/fronda/issues/new";
        assert_eq!(
            open_url_argv(url, "windows"),
            ("explorer".into(), vec![url.into()])
        );
        assert_eq!(open_url_argv(url, "macos"), ("open".into(), vec![url.into()]));
        assert_eq!(
            open_url_argv(url, "linux"),
            ("xdg-open".into(), vec![url.into()])
        );
    }
}
