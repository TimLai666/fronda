//! MCP server contract tests.
//!
//! Validates the public API surface of the MCP server: config defaults,
//! JSON-RPC message format compliance, error codes, and tool set exposure.
//! Full request/response dispatch tests live in the inline `#[cfg(test)]`
//! module of `server.rs` (private-function access).

use mcp_server::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, McpConfig};

// ── MCP-001: Server name ─────────────────────────────────────────────────────

#[test]
fn mcp_001_server_name() {
    let config = McpConfig::default();
    assert_eq!(config.server_name, "fronda");
}

// ── MCP-002: Server version ──────────────────────────────────────────────────

#[test]
fn mcp_002_server_version() {
    let config = McpConfig::default();
    assert_eq!(config.server_version, "1.0.0");
}

// ── MCP-003: Exposes the same tool set as the in-app agent ───────────────────

#[test]
fn mcp_003_exposes_54_tools() {
    let tools = agent_contract::all_tools();
    assert_eq!(
        tools.len(),
        58,
        "MCP-003: 58 tools (57 + remove_words #160)"
    );
}

#[test]
fn mcp_003_tool_names_are_snake_case() {
    let tools = agent_contract::all_tools();
    for tool in &tools {
        assert!(
            !tool.name.contains('-'),
            "tool '{}' should not contain hyphens",
            tool.name
        );
        assert!(
            tool.name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_'),
            "tool '{}' has invalid characters",
            tool.name
        );
    }
}

#[test]
fn mcp_003_all_tool_names_are_unique() {
    let tools = agent_contract::all_tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 58, "all 58 tool names must be unique");
}

#[test]
fn mcp_003_each_tool_has_valid_json_schema() {
    let tools = agent_contract::all_tools();
    for tool in &tools {
        let schema = &tool.input_schema;
        assert_eq!(
            schema.get("type").and_then(|v| v.as_str()),
            Some("object"),
            "tool '{}' schema must be type object",
            tool.name
        );
        assert!(
            schema.get("properties").is_some(),
            "tool '{}' schema must have properties",
            tool.name
        );
    }
}

// ── MCP-004: Resources include palmier://models/video and palmier://models/image
// (Tested via inline unit tests in server.rs since handle_json_rpc is private.)
// Here we verify at the serialization level that a resources/list response
// can be constructed with the correct resource URIs.

#[test]
fn mcp_004_resources_list_response_format() {
    let expected_resources = serde_json::json!([
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
    ]);
    let response = JsonRpcResponse::success(
        serde_json::json!(1),
        serde_json::json!({ "resources": expected_resources }),
    );
    let json = serde_json::to_value(&response).unwrap();
    let resources = json
        .pointer("/result/resources")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(resources.len(), 2, "MCP-004: exactly 2 resources");
    let uris: Vec<&str> = resources
        .iter()
        .filter_map(|r| r.get("uri").and_then(|v| v.as_str()))
        .collect();
    assert!(
        uris.contains(&"palmier://models/video"),
        "must include video models resource"
    );
    assert!(
        uris.contains(&"palmier://models/image"),
        "must include image models resource"
    );
}

// ── MCP-005: Binds to 127.0.0.1 only ─────────────────────────────────────────

#[test]
fn mcp_005_binds_to_loopback() {
    let config = McpConfig::default();
    assert_eq!(config.host, "127.0.0.1");
}

// ── MCP-006: Default endpoint is http://127.0.0.1:19789/mcp ──────────────────

#[test]
fn mcp_006_default_port() {
    let config = McpConfig::default();
    assert_eq!(config.port, 19789);
}

#[test]
fn mcp_006_endpoint_string() {
    let config = McpConfig::default();
    let endpoint = format!("http://{}:{}/mcp", config.host, config.port);
    assert_eq!(endpoint, "http://127.0.0.1:19789/mcp");
}

// ── JSON-RPC: Protocol compliance ────────────────────────────────────────────

#[test]
fn json_rpc_initialize_request_format() {
    // Standard MCP initialize request
    let json = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    });
    let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, serde_json::json!(1));
    assert_eq!(req.method, "initialize");
    assert!(req.params.is_object());
    assert_eq!(
        req.params.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2024-11-05")
    );
}

#[test]
fn json_rpc_initialize_response_format() {
    // The server returns MethodNotFound for unhandled methods.
    // Verify the error response format matches JSON-RPC spec.
    let response = JsonRpcResponse::error(serde_json::json!(1), JsonRpcError::MethodNotFound);
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);
    assert!(json.get("result").is_none());
    assert_eq!(json["error"]["code"], -32601);
    assert_eq!(json["error"]["message"], "Method not found");
}

#[test]
fn json_rpc_tools_list_request_format() {
    let json = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });
    let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.method, "tools/list");
    assert_eq!(req.jsonrpc, "2.0");
}

#[test]
fn json_rpc_tools_list_response_format() {
    let tools: Vec<serde_json::Value> = agent_contract::all_tools()
        .into_iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            })
        })
        .collect();
    let response =
        JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({ "tools": tools }));
    let json = serde_json::to_value(&response).unwrap();

    // Verify envelope
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 1);

    // Verify tools array
    let tools_arr = json
        .pointer("/result/tools")
        .and_then(|v| v.as_array())
        .unwrap();
    assert_eq!(tools_arr.len(), 58);

    // Each tool entry has required fields
    for tool_val in tools_arr {
        assert!(tool_val.get("name").and_then(|v| v.as_str()).is_some());
        assert!(tool_val
            .get("description")
            .and_then(|v| v.as_str())
            .is_some());
        assert!(tool_val.get("inputSchema").is_some());
    }
}

#[test]
fn json_rpc_tools_call_request_format() {
    let json = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "get_timeline",
            "arguments": {}
        }
    });
    let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.id, 2);
    assert_eq!(req.method, "tools/call");
    assert_eq!(
        req.params.get("name").and_then(|v| v.as_str()),
        Some("get_timeline")
    );
    assert!(req.params.get("arguments").is_some());
}

#[test]
fn json_rpc_unknown_method_error() {
    let response = JsonRpcResponse::error(serde_json::json!(1), JsonRpcError::MethodNotFound);
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["error"]["code"], -32601);
    assert_eq!(json["error"]["message"], "Method not found");
}

#[test]
fn json_rpc_malformed_request_fails_to_deserialize() {
    // Completely invalid JSON for a JsonRpcRequest (missing required fields)
    let result: Result<JsonRpcRequest, _> = serde_json::from_str(r#"{"foo": "bar"}"#);
    assert!(
        result.is_err(),
        "malformed request without jsonrpc/id/method should fail to deserialize"
    );
}

#[test]
fn json_rpc_empty_body_fails_to_deserialize() {
    let result: Result<JsonRpcRequest, _> = serde_json::from_str("");
    assert!(result.is_err(), "empty string should fail to deserialize");
}

// ── JSON-RPC: Error codes (JSON-RPC 2.0 spec compliance) ─────────────────────

#[test]
fn json_rpc_error_codes_spec_compliance() {
    assert_eq!(JsonRpcError::ParseError.code(), -32700);
    assert_eq!(JsonRpcError::InvalidRequest.code(), -32600);
    assert_eq!(JsonRpcError::MethodNotFound.code(), -32601);
    assert_eq!(JsonRpcError::InvalidParams.code(), -32602);
    assert_eq!(JsonRpcError::InternalError.code(), -32603);
    assert_eq!(JsonRpcError::ToolError("x".into()).code(), -32000);
}

#[test]
fn json_rpc_error_messages() {
    assert_eq!(JsonRpcError::ParseError.message(), "Parse error");
    assert_eq!(JsonRpcError::InvalidRequest.message(), "Invalid Request");
    assert_eq!(JsonRpcError::MethodNotFound.message(), "Method not found");
    assert_eq!(JsonRpcError::InvalidParams.message(), "Invalid params");
    assert_eq!(JsonRpcError::InternalError.message(), "Internal error");
    assert_eq!(
        JsonRpcError::ToolError("something broke".into()).message(),
        "something broke"
    );
}

// ── JSON-RPC: Response serialization format ───────────────────────────────────

#[test]
fn json_rpc_success_response_envelope() {
    let response = JsonRpcResponse::success(serde_json::json!(42), serde_json::json!({"ok": true}));
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 42);
    assert_eq!(json["result"]["ok"], true);
    assert!(
        json.get("error").is_none(),
        "success must not have error field"
    );
}

#[test]
fn json_rpc_error_response_envelope() {
    let response = JsonRpcResponse::error(serde_json::json!("abc"), JsonRpcError::InternalError);
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], "abc");
    assert!(
        json.get("result").is_none(),
        "error must not have result field"
    );
    assert_eq!(json["error"]["code"], -32603);
    assert_eq!(json["error"]["message"], "Internal error");
}

#[test]
fn json_rpc_parse_error_from_invalid_json() {
    // When the server receives non-JSON, it builds a ParseError response.
    let response = JsonRpcResponse::error(serde_json::Value::Null, JsonRpcError::ParseError);
    let json = serde_json::to_string(&response).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["error"]["code"], -32700);
    assert_eq!(parsed["error"]["message"], "Parse error");
}

// ── JSON-RPC: ID matching (string, number, null) ─────────────────────────────

#[test]
fn json_rpc_id_types_string() {
    let req: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":"req-1","method":"ping","params":{}}"#)
            .unwrap();
    assert_eq!(req.id, "req-1");
}

#[test]
fn json_rpc_id_types_number() {
    let req: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#).unwrap();
    assert_eq!(req.id, 1);
}

#[test]
fn json_rpc_id_types_null() {
    // Notifications use null id
    let req: JsonRpcRequest =
        serde_json::from_str(r#"{"jsonrpc":"2.0","id":null,"method":"ping","params":{}}"#).unwrap();
    assert!(req.id.is_null());
}

// ── Tools/call response format (success path) ────────────────────────────────

#[test]
fn json_rpc_tools_call_success_response_format() {
    let content = serde_json::json!({
        "content": [{
            "type": "text",
            "text": "Timeline is ready"
        }]
    });
    let response = JsonRpcResponse::success(serde_json::json!(1), content);
    let json = serde_json::to_value(&response).unwrap();
    let text = json
        .pointer("/result/content/0/text")
        .and_then(|v| v.as_str());
    assert_eq!(text, Some("Timeline is ready"));
}

#[test]
fn json_rpc_tools_call_error_response_format() {
    let content = serde_json::json!({
        "content": [{
            "type": "text",
            "text": "Missing required argument: clipId"
        }],
        "isError": true
    });
    let response = JsonRpcResponse::success(serde_json::json!(1), content);
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(
        json.pointer("/result/isError"),
        Some(&serde_json::json!(true))
    );
    assert_eq!(
        json.pointer("/result/content/0/text"),
        Some(&serde_json::json!("Missing required argument: clipId"))
    );
}

// ── Issue #58: MCP server must not freeze on runaway tool calls ───────────────

#[test]
fn issue_058_timeout_constant_exists_and_reasonable() {
    let t = mcp_server::MCP_TOOL_EXECUTION_TIMEOUT_MS;
    // Must be between 5 seconds and 5 minutes
    assert!(t >= 5_000, "timeout must be at least 5 s; got {t} ms");
    assert!(t <= 300_000, "timeout must be at most 5 min; got {t} ms");
}

#[test]
fn issue_058_timeout_is_thirty_seconds() {
    assert_eq!(
        mcp_server::MCP_TOOL_EXECUTION_TIMEOUT_MS,
        30_000,
        "Issue #58: default timeout is 30 s"
    );
}

// ── Issue #122: Expose MCP server to local network ──────────────────────────

#[test]
fn issue_122_loopback_config_is_loopback_only() {
    let config = McpConfig::default();
    assert!(
        config.is_loopback_only(),
        "default config must be loopback-only"
    );
}

#[test]
fn issue_122_network_config_not_loopback() {
    let config = McpConfig::with_network_access("0.0.0.0", 19789, "secret-token");
    assert!(!config.is_loopback_only());
}

#[test]
fn issue_122_network_config_without_token_fails_validate() {
    let config = McpConfig {
        host: "0.0.0.0".into(),
        port: 19789,
        auth_token: None,
        ..Default::default()
    };
    assert!(
        config.validate().is_err(),
        "network access without auth_token must be rejected"
    );
}

#[test]
fn issue_122_network_config_with_token_passes_validate() {
    let config = McpConfig::with_network_access("0.0.0.0", 19789, "my-secret");
    assert!(config.validate().is_ok());
    assert_eq!(config.auth_token.as_deref(), Some("my-secret"));
}

#[test]
fn issue_122_loopback_without_token_passes_validate() {
    let config = McpConfig::default();
    assert!(
        config.validate().is_ok(),
        "loopback does not need auth_token"
    );
}
