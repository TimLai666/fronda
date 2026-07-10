//! Transport-agnostic agent turn loop for the Anthropic Messages API.
//!
//! This owns the tool-use conversation: send a request, parse the response, run
//! any requested tools via an `execute_tool` closure, feed the results back, and
//! repeat until the model stops calling tools. The network is abstracted behind
//! [`LlmTransport`] so the loop is pure and unit-testable with a scripted mock;
//! a concrete HTTP client is a thin adapter that implements the one `send`
//! method. Tools go through a closure (not a borrowed executor) so a caller can
//! lock a shared executor per tool call rather than across HTTP round-trips.

use crate::tools::ToolDefinition;
use serde_json::{json, Value};

/// One request/response exchange with the model. Implementors serialize
/// `request` (an Anthropic Messages API body), POST it, and return the decoded
/// JSON response — or an error string on transport/parse failure.
pub trait LlmTransport {
    fn send(&mut self, request: &Value) -> Result<Value, String>;
}

/// A `tool_use` block the model emitted.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

/// The parts of a Messages API response the loop acts on.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedResponse {
    pub text: String,
    pub tool_uses: Vec<ToolUse>,
    pub stop_reason: String,
}

/// Extract concatenated text, tool-use blocks, and stop reason from a response.
pub fn parse_response(resp: &Value) -> ParsedResponse {
    let mut text = String::new();
    let mut tool_uses = Vec::new();
    if let Some(blocks) = resp.get("content").and_then(Value::as_array) {
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                    }
                }
                Some("tool_use") => {
                    tool_uses.push(ToolUse {
                        id: block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        input: block.get("input").cloned().unwrap_or_else(|| json!({})),
                    });
                }
                _ => {}
            }
        }
    }
    let stop_reason = resp
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    ParsedResponse {
        text,
        tool_uses,
        stop_reason,
    }
}

/// Record of one tool the loop executed and its outcome.
#[derive(Debug, Clone)]
pub struct ToolCallRecord {
    pub name: String,
    pub input: Value,
    pub result: Result<Value, String>,
}

/// Result of a completed agent turn.
#[derive(Debug, Clone)]
pub struct AgentOutcome {
    pub final_text: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCallRecord>,
}

fn tools_json(tools: &[ToolDefinition]) -> Vec<Value> {
    let last = tools.len().saturating_sub(1);
    tools
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut v = json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.input_schema,
            });
            // Cache the (static) tool set across turns: a breakpoint on the last
            // tool caches all of them (Anthropic prompt caching).
            if i == last {
                v["cache_control"] = json!({ "type": "ephemeral" });
            }
            v
        })
        .collect()
}

/// Serialize a tool result into a `tool_result` content block. Object/array
/// results are stringified compactly; the model reads JSON text fine.
fn tool_result_block(id: &str, result: &Result<Value, String>) -> Value {
    match result {
        Ok(v) => {
            let content = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            json!({ "type": "tool_result", "tool_use_id": id, "content": content })
        }
        Err(e) => json!({
            "type": "tool_result",
            "tool_use_id": id,
            "content": e,
            "is_error": true,
        }),
    }
}

/// Run one agent turn to completion.
///
/// Starts from `user_message`, then loops: build the request, `send` it, and if
/// the model emitted tool calls, run each via `execute_tool`, append the
/// assistant message and the `tool_result` blocks, and resend. Returns when the
/// model produces a response with no tool calls, or errors if `max_iterations`
/// is exceeded (a runaway-loop backstop).
///
/// `execute_tool(name, input)` runs a single tool. Passing a closure (rather than
/// the executor directly) lets a caller hold a shared executor's lock only for
/// the duration of each tool call — never across the `send` HTTP round-trips —
/// so a background agent turn cannot freeze other lock users.
#[allow(clippy::too_many_arguments)]
pub fn run_agent_turn(
    transport: &mut dyn LlmTransport,
    mut execute_tool: impl FnMut(&str, &Value) -> Result<Value, String>,
    model: &str,
    max_tokens: u32,
    system: &str,
    tools: &[ToolDefinition],
    user_message: &str,
    max_iterations: u32,
) -> Result<AgentOutcome, String> {
    let tools = tools_json(tools);
    let mut messages: Vec<Value> = vec![json!({
        "role": "user",
        "content": [{ "type": "text", "text": user_message }],
    })];
    let mut tool_calls = Vec::new();

    for iteration in 0..max_iterations {
        let mut request = json!({
            "model": model,
            "max_tokens": max_tokens,
            // System prompt as a cached content block — it's static across the
            // turn's iterations, so caching it cuts input-token cost.
            "system": [{
                "type": "text",
                "text": system,
                "cache_control": { "type": "ephemeral" },
            }],
            "tools": tools,
            "messages": messages,
        });
        // Upstream #268: Sonnet 5 requests carry output_config.effort = low.
        crate::prompt_caching::apply_model_request_extras(&mut request, model);
        let resp = transport.send(&request)?;
        let parsed = parse_response(&resp);

        if parsed.tool_uses.is_empty() {
            return Ok(AgentOutcome {
                final_text: parsed.text,
                iterations: iteration + 1,
                tool_calls,
            });
        }

        // Echo the assistant turn verbatim so tool_use ids line up.
        let assistant_content = resp
            .get("content")
            .cloned()
            .unwrap_or_else(|| Value::Array(vec![]));
        messages.push(json!({ "role": "assistant", "content": assistant_content }));

        let mut result_blocks = Vec::with_capacity(parsed.tool_uses.len());
        for tu in &parsed.tool_uses {
            let result = execute_tool(&tu.name, &tu.input);
            result_blocks.push(tool_result_block(&tu.id, &result));
            tool_calls.push(ToolCallRecord {
                name: tu.name.clone(),
                input: tu.input.clone(),
                result,
            });
        }
        messages.push(json!({ "role": "user", "content": result_blocks }));
    }

    Err(format!(
        "agent did not finish within {max_iterations} iterations"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolExecutor;

    /// Replays a fixed script of responses, one per `send`, and captures the
    /// requests it was given so tests can assert on the conversation.
    struct ScriptedTransport {
        responses: Vec<Value>,
        calls: usize,
        seen_requests: Vec<Value>,
    }

    impl ScriptedTransport {
        fn new(responses: Vec<Value>) -> Self {
            Self {
                responses,
                calls: 0,
                seen_requests: Vec::new(),
            }
        }
    }

    impl LlmTransport for ScriptedTransport {
        fn send(&mut self, request: &Value) -> Result<Value, String> {
            self.seen_requests.push(request.clone());
            let resp = self
                .responses
                .get(self.calls)
                .cloned()
                .ok_or_else(|| format!("no scripted response #{}", self.calls))?;
            self.calls += 1;
            Ok(resp)
        }
    }

    fn text_response(text: &str) -> Value {
        json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "text", "text": text }],
        })
    }

    #[test]
    fn parses_mixed_text_and_tool_use() {
        let resp = json!({
            "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "let me check" },
                { "type": "tool_use", "id": "t1", "name": "get_timeline", "input": {} },
            ],
        });
        let parsed = parse_response(&resp);
        assert_eq!(parsed.text, "let me check");
        assert_eq!(parsed.stop_reason, "tool_use");
        assert_eq!(parsed.tool_uses.len(), 1);
        assert_eq!(parsed.tool_uses[0].name, "get_timeline");
    }

    #[test]
    fn returns_immediately_when_no_tools_requested() {
        let mut transport = ScriptedTransport::new(vec![text_response("all done")]);
        let mut executor = ToolExecutor::new(core_model::Timeline::default(), core_model::MediaManifest::default());
        let outcome = run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-x",
            1024,
            "system",
            &[],
            "hello",
            8,
        )
        .unwrap();
        assert_eq!(outcome.final_text, "all done");
        assert_eq!(outcome.iterations, 1);
        assert!(outcome.tool_calls.is_empty());
    }

    #[test]
    fn sonnet5_turn_requests_carry_low_effort() {
        // Upstream #268: the live loop's request bodies set output_config.effort
        // for Sonnet 5-family models, and only for them.
        let mut transport = ScriptedTransport::new(vec![text_response("done")]);
        let mut executor = ToolExecutor::new(
            core_model::Timeline::default(),
            core_model::MediaManifest::default(),
        );
        run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-sonnet-5",
            1024,
            "system",
            &[],
            "hi",
            8,
        )
        .unwrap();
        assert_eq!(
            transport.seen_requests[0]["output_config"]["effort"],
            "low"
        );

        let mut transport = ScriptedTransport::new(vec![text_response("done")]);
        run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-haiku-4-5-20251001",
            1024,
            "system",
            &[],
            "hi",
            8,
        )
        .unwrap();
        assert!(
            transport.seen_requests[0].get("output_config").is_none(),
            "non-sonnet5 models must not set output_config"
        );
    }

    #[test]
    fn executes_tool_then_finishes() {
        let responses = vec![
            json!({
                "stop_reason": "tool_use",
                "content": [
                    { "type": "tool_use", "id": "t1", "name": "get_timeline", "input": {} },
                ],
            }),
            text_response("here is your timeline"),
        ];
        let mut transport = ScriptedTransport::new(responses);
        let mut executor = ToolExecutor::new(core_model::Timeline::default(), core_model::MediaManifest::default());
        let outcome = run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-x",
            1024,
            "system",
            &[],
            "show me the timeline",
            8,
        )
        .unwrap();

        assert_eq!(outcome.iterations, 2);
        assert_eq!(outcome.final_text, "here is your timeline");
        assert_eq!(outcome.tool_calls.len(), 1);
        assert_eq!(outcome.tool_calls[0].name, "get_timeline");
        assert!(outcome.tool_calls[0].result.is_ok());

        // The second request must carry the tool_result feeding back the call.
        let second = &transport.seen_requests[1];
        let msgs = second["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3, "user, assistant(tool_use), user(tool_result)");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "t1");
    }

    #[test]
    fn unknown_tool_reports_error_result_and_continues() {
        let responses = vec![
            json!({
                "stop_reason": "tool_use",
                "content": [
                    { "type": "tool_use", "id": "t1", "name": "no_such_tool", "input": {} },
                ],
            }),
            text_response("recovered"),
        ];
        let mut transport = ScriptedTransport::new(responses);
        let mut executor = ToolExecutor::new(core_model::Timeline::default(), core_model::MediaManifest::default());
        let outcome = run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-x",
            1024,
            "system",
            &[],
            "do something",
            8,
        )
        .unwrap();
        assert_eq!(outcome.final_text, "recovered");
        assert!(outcome.tool_calls[0].result.is_err());
        // The tool_result block is flagged as an error for the model.
        let second = &transport.seen_requests[1];
        let block = &second["messages"][2]["content"][0];
        assert_eq!(block["is_error"], true);
    }

    #[test]
    fn request_marks_system_and_last_tool_for_caching() {
        let mut transport = ScriptedTransport::new(vec![text_response("done")]);
        let mut executor =
            ToolExecutor::new(core_model::Timeline::default(), core_model::MediaManifest::default());
        let tools = crate::all_tools();
        run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "m",
            1024,
            "SYS",
            &tools,
            "hi",
            4,
        )
        .unwrap();
        let req = &transport.seen_requests[0];
        assert_eq!(req["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(req["system"][0]["text"], "SYS");
        let arr = req["tools"].as_array().unwrap();
        assert_eq!(arr.last().unwrap()["cache_control"]["type"], "ephemeral");
        assert!(arr[0].get("cache_control").is_none(), "only the last tool is a breakpoint");
    }

    #[test]
    fn errors_when_iteration_cap_exceeded() {
        // Always asks for a tool → never terminates on its own.
        let looping = json!({
            "stop_reason": "tool_use",
            "content": [
                { "type": "tool_use", "id": "t1", "name": "get_timeline", "input": {} },
            ],
        });
        let mut transport = ScriptedTransport::new(vec![looping.clone(), looping.clone(), looping]);
        let mut executor = ToolExecutor::new(core_model::Timeline::default(), core_model::MediaManifest::default());
        let outcome = run_agent_turn(
            &mut transport,
            |name, args| executor.execute(name, args),
            "claude-x",
            1024,
            "system",
            &[],
            "loop forever",
            3,
        );
        assert!(outcome.is_err());
    }
}
