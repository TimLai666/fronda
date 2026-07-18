//! Pane visibility persistence (EDT-003): media/inspector/agent flags stored
//! under `paneVisibility` in preferences.json (same file as mcp_service's
//! keys). Timeline/preview are session-only. Pure std + serde_json — no gpui.
//! Also home of the shared `whisperModelPath` read/write (settings UI writes
//! it in every build; the `transcribe-local` feature reads it).

use std::path::{Path, PathBuf};

use crate::pane::PaneVisibility;

pub const PANE_VISIBILITY_KEY: &str = "paneVisibility";
pub const PANE_SIZES_KEY: &str = "paneSizes";

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

/// Divider positions persisted under `paneSizes` (Swift NSSplitView autosave
/// parity). `None` = key absent or unusable; the caller keeps its default
/// (or preset sentinel) for that pane.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PersistedPaneSizes {
    pub agent: Option<f32>,
    pub media: Option<f32>,
    pub inspector: Option<f32>,
    pub timeline_height: Option<f32>,
}

impl PersistedPaneSizes {
    /// Viewport-independent clamp to the pane_resize limits. The media tab
    /// rail and the preview-min space guard need the live viewport, so the
    /// first editor render runs the full `clamp_resize` afterwards.
    pub fn clamped(self) -> Self {
        use crate::pane_resize as pr;
        Self {
            agent: self.agent.map(|v| v.clamp(pr::AGENT_MIN, pr::AGENT_MAX)),
            media: self.media.map(|v| v.max(pr::MEDIA_MIN)),
            inspector: self.inspector.map(|v| v.max(pr::INSPECTOR_MIN)),
            timeline_height: self
                .timeline_height
                .map(|v| v.clamp(pr::TIMELINE_MIN, pr::TIMELINE_MAX)),
        }
    }
}

/// A stored size is only usable when it is a finite positive number — the
/// negative preset sentinels must never round-trip.
fn usable_size(v: f32) -> Option<f32> {
    (v.is_finite() && v > 0.0).then_some(v)
}

fn size_field(pane: &serde_json::Value, key: &str) -> Option<f32> {
    usable_size(pane.get(key)?.as_f64()? as f32)
}

/// Missing file, unreadable JSON, or a missing/malformed `paneSizes` object
/// → `None`. Individual missing/malformed/non-positive fields degrade to
/// `None` per field.
pub fn load_pane_sizes(path: &Path) -> Option<PersistedPaneSizes> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let pane = value.get(PANE_SIZES_KEY)?;
    Some(PersistedPaneSizes {
        agent: size_field(pane, "agent"),
        media: size_field(pane, "media"),
        inspector: size_field(pane, "inspector"),
        timeline_height: size_field(pane, "timelineHeight"),
    })
}

/// Read-modify-write like [`save_pane_visibility`], but merging per field:
/// a `None` (unresolved) size never erases a previously stored one.
pub fn save_pane_sizes(path: &Path, sizes: PersistedPaneSizes) {
    let mut root = read_prefs_root(path);
    let mut obj = match root.get(PANE_SIZES_KEY) {
        Some(serde_json::Value::Object(existing)) => existing.clone(),
        _ => serde_json::Map::new(),
    };
    for (key, value) in [
        ("agent", sizes.agent),
        ("media", sizes.media),
        ("inspector", sizes.inspector),
        ("timelineHeight", sizes.timeline_height),
    ] {
        if let Some(v) = value.and_then(usable_size) {
            obj.insert(key.to_string(), serde_json::json!(v));
        }
    }
    root[PANE_SIZES_KEY] = serde_json::Value::Object(obj);
    write_prefs_root(path, &root);
}

/// Current prefs JSON root; a missing or corrupt file rebuilds from scratch.
fn read_prefs_root(path: &Path) -> serde_json::Value {
    let mut root = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !root.is_object() {
        root = serde_json::json!({});
    }
    root
}

/// Atomic write (sibling temp file + rename, project_io convention).
fn write_prefs_root(path: &Path, root: &serde_json::Value) {
    let Ok(mut bytes) = serde_json::to_vec_pretty(root) else {
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

pub const WHISPER_MODEL_PATH_KEY: &str = "whisperModelPath";

/// `whisperModelPath` from preferences.json. Missing file, unreadable JSON,
/// missing key, or a non-string/empty value → `None`.
pub fn load_whisper_model_path(path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = value.get(WHISPER_MODEL_PATH_KEY)?.as_str()?.trim();
    (!raw.is_empty()).then(|| PathBuf::from(raw))
}

/// Read-modify-write like [`save_pane_visibility`]: blank input removes the
/// key, anything else stores the trimmed path. Other keys survive; the write
/// is atomic.
pub fn save_whisper_model_path(path: &Path, raw: &str) {
    let mut root = read_prefs_root(path);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        if let Some(obj) = root.as_object_mut() {
            obj.remove(WHISPER_MODEL_PATH_KEY);
        }
    } else {
        root[WHISPER_MODEL_PATH_KEY] = serde_json::Value::String(trimmed.to_string());
    }
    write_prefs_root(path, &root);
}

pub const GENERATION_URL_KEY: &str = "generationEndpointUrl";
pub const GENERATION_TOKEN_KEY: &str = "generationEndpointToken";

/// The generation endpoint (URL, token) from preferences.json — a GUI-set
/// config, not env. `None` unless BOTH are present and non-blank, so the
/// generate tools keep their honest error when unconfigured.
pub fn load_generation_endpoint(path: &Path) -> Option<(String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let url = value.get(GENERATION_URL_KEY)?.as_str()?.trim();
    let token = value.get(GENERATION_TOKEN_KEY)?.as_str()?.trim();
    (!url.is_empty() && !token.is_empty()).then(|| (url.to_string(), token.to_string()))
}

/// Read-modify-write each field: blank removes its key, otherwise stores the
/// trimmed value. Other keys survive; the write is atomic.
pub fn save_generation_endpoint(path: &Path, url: &str, token: &str) {
    let mut root = read_prefs_root(path);
    for (key, raw) in [(GENERATION_URL_KEY, url), (GENERATION_TOKEN_KEY, token)] {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            if let Some(obj) = root.as_object_mut() {
                obj.remove(key);
            }
        } else {
            root[key] = serde_json::Value::String(trimmed.to_string());
        }
    }
    write_prefs_root(path, &root);
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
    let mut root = read_prefs_root(path);
    root[PANE_VISIBILITY_KEY] = serde_json::json!({
        "media": visibility.media,
        "inspector": visibility.inspector,
        "agent": visibility.agent,
    });
    write_prefs_root(path, &root);
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
    fn load_pane_sizes_missing_file_returns_none() {
        let path = temp_prefs("sizes-missing.json");
        assert_eq!(load_pane_sizes(&path), None);
    }

    #[test]
    fn load_pane_sizes_without_key_returns_none() {
        let path = temp_prefs("sizes-no-key.json");
        std::fs::write(&path, r#"{"paneVisibility": {"media": true}}"#).unwrap();
        assert_eq!(load_pane_sizes(&path), None);
    }

    #[test]
    fn pane_sizes_round_trip() {
        let path = temp_prefs("sizes-roundtrip.json");
        let saved = PersistedPaneSizes {
            agent: Some(320.0),
            media: Some(500.0),
            inspector: Some(280.0),
            timeline_height: Some(260.0),
        };
        save_pane_sizes(&path, saved);
        assert_eq!(load_pane_sizes(&path), Some(saved));
    }

    #[test]
    fn sizes_and_visibility_coexist_and_preserve_other_keys() {
        let path = temp_prefs("sizes-coexist.json");
        std::fs::write(&path, r#"{"mcpServerEnabled": true}"#).unwrap();
        let vis = PersistedPaneVisibility {
            media: false,
            inspector: true,
            agent: true,
        };
        let sizes = PersistedPaneSizes {
            agent: Some(300.0),
            media: Some(420.0),
            inspector: Some(260.0),
            timeline_height: Some(300.0),
        };
        save_pane_visibility(&path, vis);
        save_pane_sizes(&path, sizes);
        assert_eq!(load_pane_visibility(&path), Some(vis));
        assert_eq!(load_pane_sizes(&path), Some(sizes));
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            value.get("mcpServerEnabled"),
            Some(&serde_json::Value::Bool(true)),
            "unrelated keys must survive both saves"
        );
    }

    #[test]
    fn save_pane_sizes_merges_unresolved_fields() {
        // A save from a session where media/timeline are still preset
        // sentinels must not erase the stored values.
        let path = temp_prefs("sizes-merge.json");
        std::fs::write(
            &path,
            r#"{"paneSizes": {"media": 500.0, "timelineHeight": 240.0}}"#,
        )
        .unwrap();
        save_pane_sizes(
            &path,
            PersistedPaneSizes {
                agent: Some(300.0),
                media: None,
                inspector: Some(260.0),
                timeline_height: None,
            },
        );
        assert_eq!(
            load_pane_sizes(&path),
            Some(PersistedPaneSizes {
                agent: Some(300.0),
                media: Some(500.0),
                inspector: Some(260.0),
                timeline_height: Some(240.0),
            })
        );
    }

    #[test]
    fn load_pane_sizes_filters_malformed_and_nonpositive_fields() {
        let path = temp_prefs("sizes-malformed.json");
        std::fs::write(
            &path,
            r#"{"paneSizes": {"agent": "wide", "media": -5.0, "inspector": 0.0, "timelineHeight": 250.0}}"#,
        )
        .unwrap();
        assert_eq!(
            load_pane_sizes(&path),
            Some(PersistedPaneSizes {
                agent: None,
                media: None,
                inspector: None,
                timeline_height: Some(250.0),
            })
        );
    }

    #[test]
    fn save_pane_sizes_drops_sentinel_values() {
        // Defensive: a negative sentinel handed to save must not reach disk.
        let path = temp_prefs("sizes-sentinel.json");
        save_pane_sizes(
            &path,
            PersistedPaneSizes {
                agent: Some(-1.0),
                media: Some(f32::NAN),
                inspector: Some(260.0),
                timeline_height: None,
            },
        );
        assert_eq!(
            load_pane_sizes(&path),
            Some(PersistedPaneSizes {
                agent: None,
                media: None,
                inspector: Some(260.0),
                timeline_height: None,
            })
        );
    }

    #[test]
    fn clamped_applies_static_pane_limits() {
        use crate::pane_resize as pr;
        let clamped = PersistedPaneSizes {
            agent: Some(100.0),
            media: Some(10.0),
            inspector: Some(10.0),
            timeline_height: Some(900.0),
        }
        .clamped();
        assert_eq!(clamped.agent, Some(pr::AGENT_MIN));
        assert_eq!(clamped.media, Some(pr::MEDIA_MIN));
        assert_eq!(clamped.inspector, Some(pr::INSPECTOR_MIN));
        assert_eq!(clamped.timeline_height, Some(pr::TIMELINE_MAX));
        let upper = PersistedPaneSizes {
            agent: Some(900.0),
            media: Some(2000.0),
            inspector: None,
            timeline_height: Some(50.0),
        }
        .clamped();
        assert_eq!(upper.agent, Some(pr::AGENT_MAX));
        // Media/inspector upper bound is the viewport space guard, applied on
        // the first editor render — not statically.
        assert_eq!(upper.media, Some(2000.0));
        assert_eq!(upper.inspector, None);
        assert_eq!(upper.timeline_height, Some(pr::TIMELINE_MIN));
    }

    #[test]
    fn generation_endpoint_round_trips_and_trims() {
        let path = temp_prefs("gen-endpoint-roundtrip.json");
        save_generation_endpoint(&path, "  http://127.0.0.1:8787  ", "  tok-abc  ");
        assert_eq!(
            load_generation_endpoint(&path),
            Some(("http://127.0.0.1:8787".to_string(), "tok-abc".to_string()))
        );
    }

    #[test]
    fn generation_endpoint_needs_both_url_and_token() {
        // Only one field set → None (no backend installed, honest error kept).
        let url_only = temp_prefs("gen-url-only.json");
        save_generation_endpoint(&url_only, "http://gw", "");
        assert_eq!(load_generation_endpoint(&url_only), None);
        let token_only = temp_prefs("gen-token-only.json");
        save_generation_endpoint(&token_only, "", "tok");
        assert_eq!(load_generation_endpoint(&token_only), None);
    }

    #[test]
    fn generation_endpoint_blank_save_removes_keys_and_preserves_others() {
        let path = temp_prefs("gen-endpoint-clear.json");
        std::fs::write(&path, r#"{"mcpServerEnabled": true}"#).unwrap();
        save_generation_endpoint(&path, "http://gw", "tok");
        save_generation_endpoint(&path, "  ", "  ");
        assert_eq!(load_generation_endpoint(&path), None);
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(value.get(GENERATION_URL_KEY).is_none());
        assert!(value.get(GENERATION_TOKEN_KEY).is_none());
        assert_eq!(value.get("mcpServerEnabled"), Some(&serde_json::Value::Bool(true)));
    }

    #[test]
    fn generation_endpoint_load_missing_or_malformed_returns_none() {
        assert_eq!(load_generation_endpoint(&temp_prefs("gen-missing.json")), None);
        let corrupt = temp_prefs("gen-corrupt.json");
        std::fs::write(&corrupt, "{not json").unwrap();
        assert_eq!(load_generation_endpoint(&corrupt), None);
    }

    #[test]
    fn whisper_model_path_round_trips_and_trims() {
        let path = temp_prefs("whisper-roundtrip.json");
        save_whisper_model_path(&path, "  /models/ggml-base.bin  ");
        assert_eq!(
            load_whisper_model_path(&path),
            Some(PathBuf::from("/models/ggml-base.bin"))
        );
    }

    #[test]
    fn whisper_model_blank_save_removes_the_key() {
        let path = temp_prefs("whisper-clear.json");
        save_whisper_model_path(&path, "/models/a.bin");
        save_whisper_model_path(&path, "   ");
        assert_eq!(load_whisper_model_path(&path), None);
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            value.get(WHISPER_MODEL_PATH_KEY).is_none(),
            "blank save must remove the key, not store an empty string"
        );
    }

    #[test]
    fn whisper_model_save_preserves_other_keys() {
        let path = temp_prefs("whisper-preserve.json");
        std::fs::write(
            &path,
            r#"{"mcpServerEnabled": true, "paneVisibility": {"media": true, "inspector": true, "agent": false}}"#,
        )
        .unwrap();
        save_whisper_model_path(&path, "/models/base.gguf");
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            value.get("mcpServerEnabled"),
            Some(&serde_json::Value::Bool(true))
        );
        assert!(value.get(PANE_VISIBILITY_KEY).is_some());
        assert_eq!(
            load_whisper_model_path(&path),
            Some(PathBuf::from("/models/base.gguf"))
        );
    }

    #[test]
    fn whisper_model_load_missing_malformed_or_empty_returns_none() {
        assert_eq!(load_whisper_model_path(&temp_prefs("whisper-missing.json")), None);
        let corrupt = temp_prefs("whisper-corrupt.json");
        std::fs::write(&corrupt, "{not json").unwrap();
        assert_eq!(load_whisper_model_path(&corrupt), None);
        let non_string = temp_prefs("whisper-non-string.json");
        std::fs::write(&non_string, r#"{"whisperModelPath": 7}"#).unwrap();
        assert_eq!(load_whisper_model_path(&non_string), None);
        let empty = temp_prefs("whisper-empty.json");
        std::fs::write(&empty, r#"{"whisperModelPath": "  "}"#).unwrap();
        assert_eq!(load_whisper_model_path(&empty), None);
    }

    #[test]
    fn whisper_model_save_over_corrupt_file_rebuilds() {
        let path = temp_prefs("whisper-rebuild.json");
        std::fs::write(&path, "{broken").unwrap();
        save_whisper_model_path(&path, "/models/base.bin");
        assert_eq!(
            load_whisper_model_path(&path),
            Some(PathBuf::from("/models/base.bin"))
        );
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
