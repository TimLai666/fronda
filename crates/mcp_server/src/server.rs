use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use agent_contract::tools::all_tools;
use agent_contract::ToolExecutor;
use serde_json::{json, Value};

use crate::json_rpc::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};

pub struct McpConfig {
    pub host: String,
    pub port: u16,
    pub server_name: String,
    pub server_version: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),          // MCP-005: loopback only
            port: 19789,                       // MCP-006: default port
            server_name: "palmier-pro".into(), // MCP-001
            server_version: "1.0.0".into(),    // MCP-002
        }
    }
}

pub struct McpServer {
    config: McpConfig,
    executor: Arc<Mutex<ToolExecutor>>,
}

impl McpServer {
    pub fn new(config: McpConfig, executor: ToolExecutor) -> Self {
        Self {
            config,
            executor: Arc::new(Mutex::new(executor)),
        }
    }

    /// Start the server (blocking). Call in a background thread.
    pub fn start(&self) -> Result<(), String> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener =
            TcpListener::bind(&addr).map_err(|e| format!("Failed to bind to {addr}: {e}"))?;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let config = self.config.server_name.clone();
                    let version = self.config.server_version.clone();
                    let executor = Arc::clone(&self.executor);
                    thread::spawn(move || {
                        handle_connection(stream, &config, &version, &executor);
                    });
                }
                Err(e) => {
                    eprintln!("MCP connection error: {e}");
                }
            }
        }
        Ok(())
    }
}

fn handle_connection(
    mut stream: TcpStream,
    server_name: &str,
    server_version: &str,
    executor: &Arc<Mutex<ToolExecutor>>,
) {
    let mut buf = [0u8; 8192];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buf[..n]);

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
                let resp = handle_json_rpc(&req, executor);
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

fn handle_json_rpc(req: &JsonRpcRequest, executor: &Arc<Mutex<ToolExecutor>>) -> JsonRpcResponse {
    let id = req.id.clone();

    match req.method.as_str() {
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
        assert_eq!(config.server_name, "palmier-pro");
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
    fn tools_list_returns_42_tools() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/list".into(),
            params: json!({}),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, &exec);
        let result = resp.result.unwrap();
        let tools = result.get("tools").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            tools.len(),
            42,
            "MCP-003: exactly 42 tools (base + upstream PRs)"
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
        let resp = handle_json_rpc(&req, &exec);
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
        let resp = handle_json_rpc(&req, &exec);
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
        let resp = handle_json_rpc(&req, &exec);
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
        let resp = handle_json_rpc(&req, &exec);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[test]
    fn tools_call_split_clip_with_missing_args() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: json!(1),
            method: "tools/call".into(),
            params: json!({
                "name": "split_clip",
                "arguments": {},
            }),
        };
        let exec = make_executor();
        let resp = handle_json_rpc(&req, &exec);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        let content = result.get("content").and_then(|v| v.as_array()).unwrap();
        let text = content[0].get("text").and_then(|v| v.as_str()).unwrap();
        assert!(text.contains("Missing clipId") || text.contains("error"));
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
        let resp = handle_json_rpc(&req, &exec);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert!(result.get("isError") == Some(&json!(true)));
    }
}
