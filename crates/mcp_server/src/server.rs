use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use agent_contract::tools::all_tools;
use agent_contract::ToolExecutor;
use serde_json::{json, Value};

use crate::json_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

/// Maximum milliseconds a single tool-call may hold the executor mutex (Issue #58).
///
/// Prevents a runaway agent-driven multi-step edit from making the MCP server
/// unresponsive. The caller should enforce this via a per-request deadline;
/// the value is exported so platform glue can thread it into `tokio::timeout`.
pub const MCP_TOOL_EXECUTION_TIMEOUT_MS: u64 = 30_000;

pub struct McpConfig {
    pub host: String,
    pub port: u16,
    pub server_name: String,
    pub server_version: String,
    /// Optional bearer token for request authentication (Issue #122).
    ///
    /// When `Some(token)`, the server rejects requests that do not include
    /// `Authorization: Bearer <token>` in the HTTP headers.
    /// Required when `host` is not loopback (network exposure).
    pub auth_token: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),       // MCP-005: loopback only
            port: 19789,                    // MCP-006: default port
            server_name: "fronda".into(),   // MCP-001
            server_version: "1.0.0".into(), // MCP-002
            auth_token: None,
        }
    }
}

impl McpConfig {
    /// Whether the server is bound to loopback only (127.0.0.1 / ::1).
    pub fn is_loopback_only(&self) -> bool {
        self.host == "127.0.0.1" || self.host == "::1" || self.host == "localhost"
    }

    /// Issue #122: validate config — network exposure requires an auth token.
    ///
    /// Returns `Err` if the host is not loopback and no auth token is set.
    pub fn validate(&self) -> Result<(), String> {
        if !self.is_loopback_only() && self.auth_token.is_none() {
            return Err(format!(
                "MCP server bound to '{}' (network-accessible) requires an auth_token. \
                 Set auth_token or bind to 127.0.0.1 for local-only access.",
                self.host
            ));
        }
        Ok(())
    }

    /// Build a network-accessible config with authentication.
    ///
    /// Issue #122: use this to expose the MCP server to the local network
    /// (e.g. host = "0.0.0.0") with a bearer token for access control.
    pub fn with_network_access(
        host: impl Into<String>,
        port: u16,
        auth_token: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            port,
            auth_token: Some(auth_token.into()),
            ..Default::default()
        }
    }
}

pub struct McpServer {
    config: McpConfig,
    executor: Arc<Mutex<ToolExecutor>>,
}

impl McpServer {
    pub fn new(config: McpConfig, executor: ToolExecutor) -> Self {
        Self::with_shared_executor(config, Arc::new(Mutex::new(executor)))
    }

    /// Serve an externally owned executor so the shell UI and the MCP
    /// server operate on the same project state.
    pub fn with_shared_executor(config: McpConfig, executor: Arc<Mutex<ToolExecutor>>) -> Self {
        Self { config, executor }
    }

    /// Start the server (blocking). Call in a background thread.
    pub fn start(&self) -> Result<(), String> {
        self.config.validate()?;
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener =
            TcpListener::bind(&addr).map_err(|e| format!("Failed to bind to {addr}: {e}"))?;
        run_accept_loop(
            listener,
            self.config.server_name.clone(),
            self.config.server_version.clone(),
            self.config.auth_token.clone(),
            Arc::clone(&self.executor),
            Arc::new(AtomicBool::new(false)),
        );
        Ok(())
    }

    /// Bind and serve on a background thread, returning a handle that can stop
    /// the server. Bind errors are returned synchronously.
    pub fn spawn(self) -> Result<McpServerHandle, String> {
        self.config.validate()?;
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener =
            TcpListener::bind(&addr).map_err(|e| format!("Failed to bind to {addr}: {e}"))?;
        let local = listener
            .local_addr()
            .map_err(|e| format!("Failed to read local addr: {e}"))?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let loop_shutdown = Arc::clone(&shutdown);
        let name = self.config.server_name.clone();
        let version = self.config.server_version.clone();
        let auth_token = self.config.auth_token.clone();
        let executor = Arc::clone(&self.executor);
        let thread = thread::spawn(move || {
            run_accept_loop(listener, name, version, auth_token, executor, loop_shutdown);
        });

        Ok(McpServerHandle {
            shutdown,
            host: self.config.host.clone(),
            port: local.port(),
            thread: Mutex::new(Some(thread)),
        })
    }
}

/// Handle to a running MCP server started with [`McpServer::spawn`].
pub struct McpServerHandle {
    shutdown: Arc<AtomicBool>,
    host: String,
    port: u16,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

impl McpServerHandle {
    /// The port the server is actually bound to (resolved when port 0 was requested).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop the server and wait for the accept loop to exit. Idempotent.
    pub fn stop(&self) {
        if self.shutdown.swap(true, Ordering::SeqCst) {
            return;
        }
        // Wake the blocking accept so the loop observes the flag.
        let _ = TcpStream::connect((self.host.as_str(), self.port));
        if let Ok(mut guard) = self.thread.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }
}

fn run_accept_loop(
    listener: TcpListener,
    server_name: String,
    server_version: String,
    auth_token: Option<String>,
    executor: Arc<Mutex<ToolExecutor>>,
    shutdown: Arc<AtomicBool>,
) {
    for stream in listener.incoming() {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                let name = server_name.clone();
                let version = server_version.clone();
                let auth_token = auth_token.clone();
                let executor = Arc::clone(&executor);
                thread::spawn(move || {
                    handle_connection(stream, &name, &version, auth_token.as_deref(), &executor);
                });
            }
            Err(e) => {
                eprintln!("MCP connection error: {e}");
            }
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    server_name: &str,
    server_version: &str,
    auth_token: Option<&str>,
    executor: &Arc<Mutex<ToolExecutor>>,
) {
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buf[..n]);

    // Issue #122: when a token is configured, reject any request lacking a matching
    // `Authorization: Bearer <token>` BEFORE doing any work — a network-exposed
    // server must not serve tool calls unauthenticated.
    if let Some(token) = auth_token {
        let head = request.split("\r\n\r\n").next().unwrap_or("");
        if !request_has_bearer(head, token) {
            let response = build_http_response(401, "application/json", "{\"error\":\"unauthorized\"}");
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
            return;
        }
    }

    // Parse the HTTP request to get the body
    let body = match request.split("\r\n\r\n").nth(1) {
        Some(b) => b.trim(),
        None => "",
    };

    let response = if body.is_empty() {
        let info = json!({
            "server": server_name,
            "version": server_version,
            "endpoint": "/mcp"
        });
        build_http_response(
            200,
            "application/json",
            &serde_json::to_string(&info).unwrap(),
        )
    } else {
        match serde_json::from_str::<JsonRpcRequest>(body) {
            Ok(req) => {
                let resp = handle_json_rpc(&req, server_name, server_version, executor);
                let body = serde_json::to_string(&resp).unwrap();
                build_http_response(200, "application/json", &body)
            }
            Err(_) => {
                let resp = JsonRpcResponse::error(Value::Null, JsonRpcError::ParseError);
                let body = serde_json::to_string(&resp).unwrap();
                build_http_response(400, "application/json", &body)
            }
        }
    };

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}

/// Constant-time byte comparison (avoids a token-content timing side-channel beyond
/// the unavoidable length check).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// True if the HTTP request head carries `Authorization: Bearer <token>` matching
/// `expected` (header name + scheme case-insensitive; token compared constant-time).
fn request_has_bearer(head: &str, expected: &str) -> bool {
    for line in head.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("authorization") {
            let value = value.trim();
            let mut parts = value.splitn(2, ' ');
            let scheme = parts.next().unwrap_or("");
            let tok = parts.next().unwrap_or("").trim();
            if scheme.eq_ignore_ascii_case("bearer") {
                return ct_eq(tok.as_bytes(), expected.as_bytes());
            }
        }
    }
    false
}

fn handle_json_rpc(
    req: &JsonRpcRequest,
    server_name: &str,
    server_version: &str,
    executor: &Arc<Mutex<ToolExecutor>>,
) -> JsonRpcResponse {
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {},
                },
                "serverInfo": {
                    "name": server_name,
                    "version": server_version,
                },
            }),
        ),

        "tools/list" => {
            let tools: Vec<Value> = all_tools()
                .into_iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();

            JsonRpcResponse::success(
                id,
                json!({
                    "tools": tools,
                }),
            )
        }

        "tools/call" => {
            let name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = req.params.get("arguments").cloned().unwrap_or(json!({}));

            let tools = all_tools();
            let tool = tools.iter().find(|t| t.name == name);

            match tool {
                Some(_) => {
                    let mut exec = match executor.lock() {
                        Ok(e) => e,
                        Err(_) => {
                            return JsonRpcResponse::error(
                                id,
                                JsonRpcError::ToolError("Executor lock poisoned".into()),
                            );
                        }
                    };

                    match exec.execute(name, &arguments) {
                        Ok(content) => JsonRpcResponse::success(id, content),
                        Err(msg) => JsonRpcResponse::success(
                            id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": msg,
                                }],
                                "isError": true,
                            }),
                        ),
                    }
                }
                None => JsonRpcResponse::error(id, JsonRpcError::MethodNotFound),
            }
        }

        "resources/list" => JsonRpcResponse::success(
            id,
            json!({
                "resources": [
                    {
                        "uri": "palmier://models/video",
                        "name": "Video Generation Models",
                        "description": "Available video generation models and their status",
                        "mimeType": "application/json",
                    },
                    {
                        "uri": "palmier://models/image",
                        "name": "Image Generation Models",
                        "description": "Available image generation models and their status",
                        "mimeType": "application/json",
                    },
                ]
            }),
        ),

        "resources/read" => {
            let uri = req.params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match uri {
                "palmier://models/video" | "palmier://models/image" => JsonRpcResponse::success(
                    id,
                    json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": serde_json::to_string_pretty(&json!({
                                "models": [],
                                "loaded": false,
                            }))
                            .unwrap(),
                        }]
                    }),
                ),
                _ => JsonRpcResponse::error(id, JsonRpcError::InvalidParams),
            }
        }

        _ => JsonRpcResponse::error(id, JsonRpcError::MethodNotFound),
    }
}

fn build_http_response(status: u16, content_type: &str, body: &str) -> String {
    let status_line = match status {
        200 => "200 OK",
        400 => "400 Bad Request",
        401 => "401 Unauthorized",
        404 => "404 Not Found",
        500 => "500 Internal Server Error",
        _ => "200 OK",
    };

    format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_contract::ToolExecutor;
    use core_model::{MediaManifest, Timeline};

    fn make_executor() -> Arc<Mutex<ToolExecutor>> {
        Arc::new(Mutex::new(ToolExecutor::new(
            Timeline::default(),
            MediaManifest::default(),
        )))
    }

    #[test]
    fn mcp_001_server_name() {
        let config = McpConfig::default();
        assert_eq!(config.server_name, "fronda");
    }

    #[test]
    fn mcp_002_server_version() {
        let config = McpConfig::default();
        assert_eq!(config.server_version, "1.0.0");
    }

    #[test]
    fn mcp_005_binds_to_loopback() {
        let config = McpConfig::default();
        assert_eq!(config.host, "127.0.0.1");
    }

    #[test]
    fn mcp_006_default_port() {
        let config = McpConfig::default();
        assert_eq!(config.port, 19789);
    }

    // ---- Issue #122: MCP local network + auth token -----------------------

    #[test]
    fn issue_122_default_config_is_loopback() {
        let config = McpConfig::default();
        assert!(config.is_loopback_only());
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn issue_122_loopback_config_valid_without_auth() {
        let config = McpConfig::default();
        assert!(config.validate().is_ok(), "loopback needs no auth");
    }

    #[test]
    fn issue_122_network_config_without_auth_rejected() {
        let config = McpConfig {
            host: "0.0.0.0".into(),
            auth_token: None,
            ..Default::default()
        };
        assert!(!config.is_loopback_only());
        let err = config.validate().unwrap_err();
        assert!(err.contains("auth_token"), "err={err}");
    }

    #[test]
    fn issue_122_network_config_with_auth_valid() {
        let config = McpConfig::with_network_access("0.0.0.0", 19789, "secret-token");
        assert!(!config.is_loopback_only());
        assert!(config.auth_token.is_some());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn issue_122_localhost_is_loopback() {
        let config = McpConfig {
            host: "localhost".into(),
            ..Default::default()
        };
        assert!(config.is_loopback_only());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn issue_122_ipv6_loopback_is_loopback() {
        let config = McpConfig {
            host: "::1".into(),
            ..Default::default()
        };
        assert!(config.is_loopback_only());
        assert!(config.validate().is_ok());
    }

    fn spawn_on_ephemeral_port() -> McpServerHandle {
        let config = McpConfig {
            port: 0,
            ..Default::default()
        };
        let executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        McpServer::new(config, executor).spawn().unwrap()
    }

    #[test]
    fn spawn_stop_releases_port() {
        let handle = spawn_on_ephemeral_port();
        let port = handle.port();
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok());
        handle.stop();
        let rebind = TcpListener::bind(("127.0.0.1", port));
        assert!(rebind.is_ok(), "port should be released after stop");
    }

    #[test]
    fn stop_is_idempotent() {
        let handle = spawn_on_ephemeral_port();
        handle.stop();
        handle.stop();
        handle.stop();
    }

    fn http_rpc(port: u16, body: &str) -> Value {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let req = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        let json_body = resp.split("\r\n\r\n").nth(1).unwrap();
        serde_json::from_str(json_body).unwrap()
    }

    fn http_raw(port: u16, auth: Option<&str>, body: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let auth_header = match auth {
            Some(t) => format!("Authorization: Bearer {t}\r\n"),
            None => String::new(),
        };
        let req = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n{auth_header}Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        resp
    }

    #[test]
    fn issue_122_auth_token_enforced_on_requests() {
        // Loopback + token → validate() passes AND auth is enforced on every request.
        let config = McpConfig {
            port: 0,
            auth_token: Some("secret-token".into()),
            ..Default::default()
        };
        let executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        let handle = McpServer::new(config, executor).spawn().unwrap();
        let port = handle.port();
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_timeline","arguments":{}}}"#;

        let unauth = http_raw(port, None, call);
        assert!(unauth.starts_with("HTTP/1.1 401"), "missing token → 401: {}", unauth.lines().next().unwrap_or(""));
        let wrong = http_raw(port, Some("nope"), call);
        assert!(wrong.starts_with("HTTP/1.1 401"), "wrong token → 401");
        let ok = http_raw(port, Some("secret-token"), call);
        assert!(ok.starts_with("HTTP/1.1 200"), "correct token → 200: {}", ok.lines().next().unwrap_or(""));
        assert!(ok.contains("\"result\""), "authorized call returns a result");
        handle.stop();
    }

    #[test]
    fn issue_122_network_bind_without_token_is_rejected_at_spawn() {
        let config = McpConfig {
            host: "0.0.0.0".into(),
            port: 0,
            auth_token: None,
            ..Default::default()
        };
        let executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        assert!(
            McpServer::new(config, executor).spawn().is_err(),
            "network bind without a token must be rejected at spawn"
        );
    }

    #[test]
    fn shared_executor_state_visible_both_ways() {
        let shared = Arc::new(Mutex::new(ToolExecutor::new(
            Timeline::default(),
            MediaManifest::default(),
        )));
        let config = McpConfig {
            port: 0,
            ..Default::default()
        };
        let handle = McpServer::with_shared_executor(config, Arc::clone(&shared))
            .spawn()
            .unwrap();
        let port = handle.port();

        // External change is visible over MCP.
        shared.lock().unwrap().timeline_mut().fps = 60;
        let resp = http_rpc(
            port,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_timeline","arguments":{}}}"#,
        );
        let text = resp
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(
            text.contains("60"),
            "external fps change not visible: {text}"
        );

        // MCP mutation is visible externally.
        let resp = http_rpc(
            port,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_folder","arguments":{"name":"B-roll"}}}"#,
        );
        assert!(resp.get("result").is_some(), "create_folder failed: {resp}");
        {
            let exec = shared.lock().unwrap();
            assert!(exec
                .media_manifest()
                .folders
                .iter()
                .any(|f| f.name == "B-roll"));
            assert_eq!(exec.revision(), 1);
        }
        handle.stop();
    }

    #[test]
    fn spawn_bind_conflict_returns_err() {
        let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = occupied.local_addr().unwrap().port();
        let config = McpConfig {
            port,
            ..Default::default()
        };
        let executor = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        match McpServer::new(config, executor).spawn() {
            Err(e) => assert!(e.contains("Failed to bind")),
            Ok(_) => panic!("bind conflict must surface as Err"),
        }
    }

    #[test]
    fn initialize_returns_server_info() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "initialize".into(),
            params: json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0.0"},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        let result = resp.result.unwrap();
        assert_eq!(
            result.pointer("/serverInfo/name").and_then(|v| v.as_str()),
            Some("fronda")
        );
        assert_eq!(
            result
                .pointer("/serverInfo/version")
                .and_then(|v| v.as_str()),
            Some("1.0.0")
        );
        assert!(result.get("capabilities").is_some());
    }

    #[test]
    fn json_rpc_parse_error() {
        let err = JsonRpcError::ParseError;
        assert_eq!(err.code(), -32700);
    }

    #[test]
    fn json_rpc_method_not_found() {
        let err = JsonRpcError::MethodNotFound;
        assert_eq!(err.code(), -32601);
    }

    #[test]
    fn json_rpc_response_success() {
        let resp = JsonRpcResponse::success(json!(1), json!({"ok": true}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, json!(1));
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn json_rpc_response_error() {
        let resp = JsonRpcResponse::error(json!(1), JsonRpcError::MethodNotFound);
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn tools_list_returns_54_tools() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/list".into(),
            params: json!({}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        let result = resp.result.unwrap();
        let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            tools.len(),
            59,
            "MCP-003: 59 tools (58 + create_matte #242)"
        );
    }

    #[test]
    fn resources_list_returns_two_resources() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "resources/list".into(),
            params: json!({}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        let result = resp.result.unwrap();
        let resources = result.get("resources").and_then(|v| v.as_array()).unwrap();
        assert_eq!(resources.len(), 2, "MCP-004: exactly 2 resources");
    }

    #[test]
    fn unknown_method_returns_error() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "unknown_method".into(),
            params: json!({}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn tools_call_get_timeline_returns_timeline() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/call".into(),
            params: json!({
                "name": "get_timeline",
                "arguments": {},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let content = result.get("content").and_then(|v| v.as_array()).unwrap();
        let text = content[0].get("text").and_then(|v| v.as_str()).unwrap();
        assert!(text.contains("fps"));
    }

    #[test]
    fn tools_call_unknown_tool_returns_method_not_found() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/call".into(),
            params: json!({
                "name": "nonexistent_tool",
                "arguments": {},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn tools_call_split_clips_with_missing_args() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/call".into(),
            params: json!({
                "name": "split_clips",
                "arguments": {},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let content = result.get("content").and_then(|v| v.as_array()).unwrap();
        let text = content[0].get("text").and_then(|v| v.as_str()).unwrap();
        assert!(text.contains("Provide either") || text.contains("error"));
    }

    #[test]
    fn tools_call_generate_video_returns_notice() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/call".into(),
            params: json!({
                "name": "generate_video",
                "arguments": {},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result.get("isError") == Some(&json!(true)));
    }

    // ── MCP-007: resources/read ───────────────────────────────────────

    #[test]
    fn mcp_007_resources_read_video_models() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "resources/read".into(),
            params: json!({"uri": "palmier://models/video"}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(
            resp.error.is_none(),
            "MCP-007: video models resource should be readable"
        );
        let result = resp.result.unwrap();
        let contents = result.get("contents").and_then(|v| v.as_array()).unwrap();
        assert_eq!(contents.len(), 1);
        let c = &contents[0];
        assert_eq!(
            c.get("uri").and_then(|v| v.as_str()).unwrap(),
            "palmier://models/video"
        );
        assert_eq!(
            c.get("mimeType").and_then(|v| v.as_str()).unwrap(),
            "application/json"
        );
    }

    #[test]
    fn mcp_008_resources_read_image_models() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "resources/read".into(),
            params: json!({"uri": "palmier://models/image"}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(
            resp.error.is_none(),
            "MCP-008: image models resource should be readable"
        );
        let result = resp.result.unwrap();
        let contents = result.get("contents").and_then(|v| v.as_array()).unwrap();
        assert_eq!(contents.len(), 1);
        let c = &contents[0];
        assert_eq!(
            c.get("uri").and_then(|v| v.as_str()).unwrap(),
            "palmier://models/image"
        );
    }

    #[test]
    fn mcp_009_resources_read_unknown_uri_returns_error() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "resources/read".into(),
            params: json!({"uri": "palmier://unknown"}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, "fronda", "1.0.0", &exec);
        assert!(resp.error.is_some(), "unknown URI should return error");
        assert_eq!(resp.error.unwrap().code, -32602); // InvalidParams
    }
}
