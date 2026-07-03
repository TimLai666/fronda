//! MCP server lifecycle for the desktop shell (Swift: MCPService).
//!
//! Owns the preference-gated start/stop of the MCP server and reports
//! state through `McpServerStatus`. Pure std + workspace crates — no gpui.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use agent_contract::ToolExecutor;
use app_contract::agent_panel_model::McpServerStatus;
use app_contract::settings_storage::{MCP_DEFAULT_ENABLED, MCP_DEFAULT_PORT, MCP_ENABLED_KEY};
#[cfg(test)]
use core_model::{MediaManifest, Timeline};
use mcp_server::{McpConfig, McpServer, McpServerHandle};

/// Platform config file for Fronda preferences.
fn default_prefs_path() -> PathBuf {
    crate::project_registry_store::fronda_config_dir().join("preferences.json")
}

fn read_enabled_preference(path: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(path) else {
        return MCP_DEFAULT_ENABLED;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return MCP_DEFAULT_ENABLED;
    };
    value
        .get(MCP_ENABLED_KEY)
        .and_then(|v| v.as_bool())
        .unwrap_or(MCP_DEFAULT_ENABLED)
}

fn write_enabled_preference(path: &Path, enabled: bool) {
    let mut value = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !value.is_object() {
        value = serde_json::json!({});
    }
    value[MCP_ENABLED_KEY] = serde_json::Value::Bool(enabled);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, text);
    }
}

/// MCP server lifecycle manager. One per process — see [`McpService::global`].
pub struct McpService {
    handle: Option<McpServerHandle>,
    status: McpServerStatus,
    prefs_path: PathBuf,
    port: u16,
    /// Shared project state — restarts serve the same executor.
    executor: Arc<Mutex<ToolExecutor>>,
}

impl McpService {
    #[cfg(test)]
    fn with_prefs_path(prefs_path: PathBuf) -> Self {
        Self::with_prefs_path_and_executor(
            prefs_path,
            Arc::new(Mutex::new(ToolExecutor::new(
                Timeline::default(),
                MediaManifest::default(),
            ))),
        )
    }

    fn with_prefs_path_and_executor(
        prefs_path: PathBuf,
        executor: Arc<Mutex<ToolExecutor>>,
    ) -> Self {
        Self {
            handle: None,
            status: McpServerStatus::Stopped,
            prefs_path,
            port: MCP_DEFAULT_PORT,
            executor,
        }
    }

    /// Process-wide instance — exactly one MCP server per app, serving the
    /// shared editor state from [`crate::editor_state_hub::EditorStateHub`].
    pub fn global() -> &'static Mutex<McpService> {
        static INSTANCE: OnceLock<Mutex<McpService>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Mutex::new(McpService::with_prefs_path_and_executor(
                default_prefs_path(),
                crate::editor_state_hub::EditorStateHub::global().executor(),
            ))
        })
    }

    /// SETUI-011: enabled when the preference is absent.
    pub fn is_enabled_preference(&self) -> bool {
        read_enabled_preference(&self.prefs_path)
    }

    pub fn status(&self) -> &McpServerStatus {
        &self.status
    }

    /// Start the server if the preference allows it (app boot path).
    pub fn start_if_enabled(&mut self) {
        if self.is_enabled_preference() {
            self.start();
        } else {
            self.status = McpServerStatus::Stopped;
        }
    }

    /// Start the server. Bind failure becomes `Failed(reason)` — never panics.
    pub fn start(&mut self) {
        if self.handle.is_some() {
            return;
        }
        self.status = McpServerStatus::Starting;
        let config = McpConfig {
            port: self.port,
            ..Default::default()
        };
        match McpServer::with_shared_executor(config, Arc::clone(&self.executor)).spawn() {
            Ok(handle) => {
                self.status = McpServerStatus::Running {
                    port: handle.port(),
                };
                self.handle = Some(handle);
            }
            Err(reason) => {
                self.status = McpServerStatus::Failed(reason);
            }
        }
    }

    /// Stop the server. Idempotent.
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
        self.status = McpServerStatus::Stopped;
    }

    /// Persist the preference and apply it immediately.
    pub fn set_enabled(&mut self, enabled: bool) {
        write_enabled_preference(&self.prefs_path, enabled);
        if enabled {
            self.start();
        } else {
            self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_prefs(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-mcp-service-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    fn service_on_free_port(name: &str) -> McpService {
        let mut svc = McpService::with_prefs_path(temp_prefs(name));
        // Ephemeral port so parallel tests never collide on 19789.
        svc.port = 0;
        svc
    }

    #[test]
    fn preference_absent_means_enabled() {
        let svc = service_on_free_port("absent.json");
        assert!(svc.is_enabled_preference(), "SETUI-011: default enabled");
    }

    #[test]
    fn preference_roundtrip() {
        let mut svc = service_on_free_port("roundtrip.json");
        svc.set_enabled(false);
        assert!(!svc.is_enabled_preference());
        svc.set_enabled(true);
        assert!(svc.is_enabled_preference());
        svc.stop();
    }

    #[test]
    fn start_if_enabled_respects_disabled_preference() {
        let mut svc = service_on_free_port("disabled.json");
        write_enabled_preference(&svc.prefs_path.clone(), false);
        svc.start_if_enabled();
        assert_eq!(*svc.status(), McpServerStatus::Stopped);
    }

    #[test]
    fn start_transitions_to_running_and_stop_to_stopped() {
        let mut svc = service_on_free_port("lifecycle.json");
        svc.start();
        assert!(
            matches!(svc.status(), McpServerStatus::Running { .. }),
            "status={:?}",
            svc.status()
        );
        svc.stop();
        assert_eq!(*svc.status(), McpServerStatus::Stopped);
        svc.stop(); // idempotent
        assert_eq!(*svc.status(), McpServerStatus::Stopped);
    }

    #[test]
    fn restart_preserves_shared_executor() {
        let hub = crate::editor_state_hub::EditorStateHub::new();
        let mut svc =
            McpService::with_prefs_path_and_executor(temp_prefs("restart.json"), hub.executor());
        svc.port = 0;
        svc.start();
        assert!(matches!(svc.status(), McpServerStatus::Running { .. }));
        svc.stop();
        svc.start();
        assert!(Arc::ptr_eq(&svc.executor, &hub.executor()));
        svc.stop();
    }

    #[test]
    fn bind_conflict_becomes_failed_with_message() {
        let occupied = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = occupied.local_addr().unwrap().port();
        let mut svc = McpService::with_prefs_path(temp_prefs("conflict.json"));
        svc.port = port;
        svc.start();
        match svc.status() {
            McpServerStatus::Failed(reason) => {
                assert!(reason.contains("Failed to bind"), "reason={reason}")
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
