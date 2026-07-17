//! Pane visibility persistence (EDT-003): media/inspector/agent flags stored
//! under `paneVisibility` in preferences.json (same file as mcp_service's
//! keys). Timeline/preview are session-only. Pure std + serde_json — no gpui.

use std::path::{Path, PathBuf};

use crate::pane::PaneVisibility;

pub const PANE_VISIBILITY_KEY: &str = "paneVisibility";

/// Default preferences file, shared with mcp_service.
pub fn default_prefs_path() -> PathBuf {
    crate::project_registry_store::fronda_config_dir().join("preferences.json")
}

/// The three panes EDT-003 persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistedPaneVisibility {
    pub media: bool,
    pub inspector: bool,
    pub agent: bool,
}

impl PersistedPaneVisibility {
    pub fn from_layout(visibility: &PaneVisibility) -> Self {
        Self {
            media: visibility.media,
            inspector: visibility.inspector,
            agent: visibility.agent,
        }
    }

    /// Apply the persisted flags; other panes keep their current state.
    pub fn apply_to(&self, visibility: &mut PaneVisibility) {
        visibility.media = self.media;
        visibility.inspector = self.inspector;
        visibility.agent = self.agent;
    }
}

/// Missing file, unreadable JSON, or a missing/malformed `paneVisibility`
/// object → `None`; boot keeps the defaults.
pub fn load_pane_visibility(path: &Path) -> Option<PersistedPaneVisibility> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let pane = value.get(PANE_VISIBILITY_KEY)?;
    Some(PersistedPaneVisibility {
        media: pane.get("media")?.as_bool()?,
        inspector: pane.get("inspector")?.as_bool()?,
        agent: pane.get("agent")?.as_bool()?,
    })
}

/// Read-modify-write: only the `paneVisibility` key is replaced, every other
/// key is preserved. A missing or corrupt file is rebuilt from scratch. The
/// write is atomic (sibling temp file + rename, project_io convention).
pub fn save_pane_visibility(path: &Path, visibility: PersistedPaneVisibility) {
    let mut root = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    root[PANE_VISIBILITY_KEY] = serde_json::json!({
        "media": visibility.media,
        "inspector": visibility.inspector,
        "agent": visibility.agent,
    });
    let Ok(mut bytes) = serde_json::to_vec_pretty(&root) else {
        return;
    };
    bytes.push(b'\n');
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut tmp_os = path.as_os_str().to_owned();
    tmp_os.push(".tmp");
    let tmp = PathBuf::from(tmp_os);
    if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{PaneId, PaneLayout};

    fn temp_prefs(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-pane-prefs-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn load_missing_file_returns_none() {
        let path = temp_prefs("missing.json");
        assert_eq!(load_pane_visibility(&path), None);
    }

    #[test]
    fn load_corrupt_json_returns_none() {
        let path = temp_prefs("corrupt.json");
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_pane_visibility(&path), None);
    }

    #[test]
    fn load_without_pane_key_returns_none() {
        let path = temp_prefs("no-key.json");
        std::fs::write(&path, r#"{"mcpServerEnabled": true}"#).unwrap();
        assert_eq!(load_pane_visibility(&path), None);
    }

    #[test]
    fn load_with_malformed_pane_key_returns_none() {
        let path = temp_prefs("malformed-key.json");
        std::fs::write(
            &path,
            r#"{"paneVisibility": {"media": "yes", "inspector": true, "agent": true}}"#,
        )
        .unwrap();
        assert_eq!(load_pane_visibility(&path), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let path = temp_prefs("roundtrip.json");
        let saved = PersistedPaneVisibility {
            media: false,
            inspector: true,
            agent: false,
        };
        save_pane_visibility(&path, saved);
        assert_eq!(load_pane_visibility(&path), Some(saved));
    }

    #[test]
    fn save_preserves_other_keys() {
        let path = temp_prefs("preserve.json");
        std::fs::write(&path, r#"{"mcpServerEnabled": false}"#).unwrap();
        save_pane_visibility(
            &path,
            PersistedPaneVisibility {
                media: true,
                inspector: false,
                agent: true,
            },
        );
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value.get("mcpServerEnabled"),
            Some(&serde_json::Value::Bool(false)),
            "existing keys must survive a pane save"
        );
        assert!(value.get(PANE_VISIBILITY_KEY).is_some());
    }

    #[test]
    fn save_over_corrupt_file_rebuilds() {
        let path = temp_prefs("rebuild.json");
        std::fs::write(&path, "{broken").unwrap();
        let saved = PersistedPaneVisibility {
            media: true,
            inspector: true,
            agent: false,
        };
        save_pane_visibility(&path, saved);
        assert_eq!(load_pane_visibility(&path), Some(saved));
    }

    #[test]
    fn save_creates_missing_parent_dir() {
        let dir = std::env::temp_dir()
            .join("fronda-pane-prefs-tests")
            .join("nested-parent");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("prefs.json");
        let saved = PersistedPaneVisibility {
            media: false,
            inspector: false,
            agent: false,
        };
        save_pane_visibility(&path, saved);
        assert_eq!(load_pane_visibility(&path), Some(saved));
    }

    #[test]
    fn save_leaves_no_temp_file() {
        let path = temp_prefs("no-temp.json");
        save_pane_visibility(
            &path,
            PersistedPaneVisibility {
                media: true,
                inspector: true,
                agent: true,
            },
        );
        let mut tmp_os = path.as_os_str().to_owned();
        tmp_os.push(".tmp");
        assert!(!PathBuf::from(tmp_os).exists(), "temp file must be renamed away");
    }

    #[test]
    fn edt_003_boot_round_trip_applies_three_panes_only() {
        let path = temp_prefs("boot-roundtrip.json");

        // Session one: user hides media + agent, then quits.
        let mut layout = PaneLayout::new();
        layout.toggle_pane(PaneId::Media);
        layout.toggle_pane(PaneId::Agent);
        save_pane_visibility(&path, PersistedPaneVisibility::from_layout(&layout.visibility));

        // Session two: fresh layout, boot applies the persisted flags.
        let mut next = PaneLayout::new();
        if let Some(saved) = load_pane_visibility(&path) {
            saved.apply_to(&mut next.visibility);
        }
        assert!(!next.visibility.media);
        assert!(next.visibility.inspector);
        assert!(!next.visibility.agent);
        // Timeline/preview are not persisted and keep their defaults.
        assert!(next.visibility.timeline);
        assert!(next.visibility.preview);
    }
}
