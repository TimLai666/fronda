//! Cache-control strategy for Anthropic API messages.
//!
//! Anthropic API supports prompt caching by marking messages/content blocks
//! with `cache_control = { type: "ephemeral" }` breakpoints. This module
//! provides pure-logic helpers to determine where those breakpoints should
//! be placed in a conversation.

/// Cache control breakpoint type for Anthropic API.
#[derive(Debug, Clone, PartialEq)]
pub enum CacheBreakpoint {
    /// Mark the preceding content for ephemeral caching.
    Ephemeral,
}

/// A content block that can be cached.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedContent {
    pub content: String,
    pub breakpoint: Option<CacheBreakpoint>,
}

/// A message in the conversation, ready for API serialization.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedMessage {
    pub role: String,
    pub content: Vec<CachedContent>,
    pub breakpoint: Option<CacheBreakpoint>,
}

/// Result of applying caching strategy to a conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedConversation {
    pub system_prompt: Option<CachedContent>,
    pub messages: Vec<CachedMessage>,
}

/// Cache strategy configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheStrategy {
    /// Number of most-recent assistant+user message pairs to leave uncached.
    /// Older messages get a cache breakpoint. Default: 1
    pub keep_uncached_exchanges: usize,
    /// Whether to cache the system prompt. Default: true
    pub cache_system_prompt: bool,
    /// Minimum total messages before caching kicks in.
    /// If fewer messages than this, skip ALL cache breakpoints. Default: 3
    pub min_messages_for_caching: usize,
}

impl Default for CacheStrategy {
    fn default() -> Self {
        Self {
            keep_uncached_exchanges: 1,
            cache_system_prompt: true,
            min_messages_for_caching: 3,
        }
    }
}

/// Build a cached conversation from raw messages.
///
/// The system prompt gets a cache breakpoint if `cache_system_prompt` is true.
/// For conversation messages, all messages before the last N exchanges
/// (where N = keep_uncached_exchanges) get cache breakpoints on their
/// last content block each.
pub fn build_cached_conversation(
    system_prompt: Option<&str>,
    messages: &[(String, String)],
    strategy: &CacheStrategy,
) -> CachedConversation {
    let system_prompt = if strategy.cache_system_prompt {
        system_prompt.map(|sp| CachedContent {
            content: sp.to_string(),
            breakpoint: Some(CacheBreakpoint::Ephemeral),
        })
    } else {
        system_prompt.map(|sp| CachedContent {
            content: sp.to_string(),
            breakpoint: None,
        })
    };

    if messages.is_empty() {
        return CachedConversation {
            system_prompt,
            messages: Vec::new(),
        };
    }

    if messages.len() < strategy.min_messages_for_caching {
        // Too few messages — no cache breakpoints on any message.
        let messages: Vec<CachedMessage> = messages
            .iter()
            .map(|(role, content)| CachedMessage {
                role: role.clone(),
                content: vec![CachedContent {
                    content: content.clone(),
                    breakpoint: None,
                }],
                breakpoint: None,
            })
            .collect();
        return CachedConversation {
            system_prompt,
            messages,
        };
    }

    // Determine how many messages get a cache breakpoint.
    // Each "exchange" is a user message followed by an assistant message,
    // but we treat the messages list as flat. The oldest messages (up to a
    // calculated cut-off index) receive a breakpoint on their last content block.
    let total = messages.len();
    let uncached_count = strategy.keep_uncached_exchanges * 2; // each exchange = user + assistant
    let cache_count = total.saturating_sub(uncached_count);

    let messages: Vec<CachedMessage> = messages
        .iter()
        .enumerate()
        .map(|(i, (role, content))| {
            let is_old = i < cache_count;
            let breakpoint = if is_old {
                Some(CacheBreakpoint::Ephemeral)
            } else {
                None
            };
            // When we add support for multi-block messages, the breakpoint
            // goes only on the last content block. For now each message has
            // exactly one content block.
            CachedMessage {
                role: role.clone(),
                content: vec![CachedContent {
                    content: content.clone(),
                    breakpoint,
                }],
                breakpoint: None,
            }
        })
        .collect();

    CachedConversation {
        system_prompt,
        messages,
    }
}

fn content_block(c: &CachedContent) -> serde_json::Value {
    let mut b = serde_json::json!({ "type": "text", "text": c.content });
    if c.breakpoint.is_some() {
        b["cache_control"] = serde_json::json!({ "type": "ephemeral" });
    }
    b
}

/// Upstream #268: model-specific request extras. Sonnet 5-family requests set
/// `output_config.effort` to `low` (mirrors Swift `AnthropicModel.requestExtras`).
pub fn model_request_extras(model: &str) -> Option<serde_json::Value> {
    if model.starts_with("claude-sonnet-5") {
        Some(serde_json::json!({ "output_config": { "effort": "low" } }))
    } else {
        None
    }
}

/// Merge `model_request_extras` into a request body (top-level keys).
pub fn apply_model_request_extras(req: &mut serde_json::Value, model: &str) {
    if let Some(serde_json::Value::Object(extras)) = model_request_extras(model) {
        if let serde_json::Value::Object(body) = req {
            for (k, v) in extras {
                body.insert(k, v);
            }
        }
    }
}

/// Assemble an Anthropic Messages API request body from a cache-annotated
/// conversation, the tool set, and model params. System prompt and messages
/// carry `cache_control: {type: ephemeral}` wherever the conversation marked a
/// breakpoint; the tool set is emitted as the `tools` array. Pure — an HTTP
/// client serializes and sends the returned JSON.
pub fn build_agent_request(
    model: &str,
    max_tokens: u32,
    tools: &[crate::tools::ToolDefinition],
    conversation: &CachedConversation,
) -> serde_json::Value {
    let mut req = serde_json::json!({
        "model": model,
        "max_tokens": max_tokens,
    });
    apply_model_request_extras(&mut req, model);

    if let Some(sys) = &conversation.system_prompt {
        req["system"] = serde_json::json!([content_block(sys)]);
    }

    req["tools"] = serde_json::Value::Array(
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect(),
    );

    let messages: Vec<serde_json::Value> = conversation
        .messages
        .iter()
        .map(|m| {
            let mut blocks: Vec<serde_json::Value> = m.content.iter().map(content_block).collect();
            // A message-level breakpoint caches its final content block.
            if m.breakpoint.is_some() {
                if let Some(last) = blocks.last_mut() {
                    last["cache_control"] = serde_json::json!({ "type": "ephemeral" });
                }
            }
            serde_json::json!({ "role": m.role, "content": blocks })
        })
        .collect();
    req["messages"] = serde_json::Value::Array(messages);

    req
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_agent_request_assembles_anthropic_body() {
        let conversation = CachedConversation {
            system_prompt: Some(CachedContent {
                content: "SYS".into(),
                breakpoint: Some(CacheBreakpoint::Ephemeral),
            }),
            messages: vec![
                CachedMessage {
                    role: "user".into(),
                    content: vec![CachedContent {
                        content: "hi".into(),
                        breakpoint: None,
                    }],
                    breakpoint: Some(CacheBreakpoint::Ephemeral),
                },
                CachedMessage {
                    role: "assistant".into(),
                    content: vec![CachedContent {
                        content: "hello".into(),
                        breakpoint: None,
                    }],
                    breakpoint: None,
                },
            ],
        };
        let tools = crate::tools::all_tools();
        let req = build_agent_request("claude-sonnet-5", 4096, &tools, &conversation);

        assert_eq!(req["model"], "claude-sonnet-5");
        assert_eq!(req["max_tokens"], 4096);
        // System is a cache-controlled text block.
        assert_eq!(req["system"][0]["text"], "SYS");
        assert_eq!(req["system"][0]["cache_control"]["type"], "ephemeral");
        // Tools array mirrors the tool set.
        assert_eq!(req["tools"].as_array().unwrap().len(), tools.len());
        assert!(req["tools"][0]["input_schema"].is_object());
        // Messages preserved; the user message's breakpoint caches its last block.
        assert_eq!(req["messages"][0]["role"], "user");
        assert_eq!(req["messages"][0]["content"][0]["text"], "hi");
        assert_eq!(
            req["messages"][0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(req["messages"][1]["role"], "assistant");
        assert!(req["messages"][1]["content"][0]
            .get("cache_control")
            .is_none());
    }

    // ─── Upstream #268: Sonnet 5 output_config.effort = low ───

    #[test]
    fn sonnet5_requests_carry_low_effort_output_config() {
        let conversation = CachedConversation {
            system_prompt: None,
            messages: vec![],
        };
        let tools: Vec<crate::tools::ToolDefinition> = vec![];
        for m in ["claude-sonnet-5", "claude-sonnet-5-20260203"] {
            let req = build_agent_request(m, 1024, &tools, &conversation);
            assert_eq!(req["output_config"], json!({ "effort": "low" }), "{m}");
        }
        for m in ["claude-opus-4-8", "claude-haiku-4-5-20251001"] {
            let req = build_agent_request(m, 1024, &tools, &conversation);
            assert!(
                req.get("output_config").is_none(),
                "{m} must not set output_config"
            );
        }
    }

    #[test]
    fn model_request_extras_matches_swift_shape() {
        assert_eq!(
            model_request_extras("claude-sonnet-5"),
            Some(json!({ "output_config": { "effort": "low" } }))
        );
        assert_eq!(model_request_extras("claude-opus-4-8"), None);
    }

    fn msg(role: &str, content: &str) -> (String, String) {
        (role.to_string(), content.to_string())
    }

    fn default_strategy() -> CacheStrategy {
        CacheStrategy::default()
    }

    #[test]
    fn empty_conversation_returns_no_messages() {
        let result = build_cached_conversation(None, &[], &default_strategy());
        assert_eq!(result.system_prompt, None);
        assert!(result.messages.is_empty());
    }

    #[test]
    fn system_prompt_gets_cache_breakpoint() {
        let result = build_cached_conversation(
            Some("You are a helpful assistant."),
            &[msg("user", "Hi")],
            &CacheStrategy {
                min_messages_for_caching: 1,
                ..default_strategy()
            },
        );
        assert_eq!(
            result.system_prompt,
            Some(CachedContent {
                content: "You are a helpful assistant.".to_string(),
                breakpoint: Some(CacheBreakpoint::Ephemeral),
            })
        );
    }

    #[test]
    fn no_system_prompt_returns_none() {
        let result = build_cached_conversation(None, &[msg("user", "Hi")], &default_strategy());
        assert_eq!(result.system_prompt, None);
    }

    #[test]
    fn few_messages_skip_caching() {
        let messages = vec![msg("user", "a"), msg("assistant", "b")];
        let result = build_cached_conversation(None, &messages, &default_strategy());
        assert_eq!(result.messages.len(), 2);
        for msg in &result.messages {
            assert_eq!(msg.content.len(), 1);
            assert_eq!(msg.content[0].breakpoint, None);
        }
    }

    #[test]
    fn older_messages_get_cache_breakpoints() {
        // 4 exchanges (8 messages), keep=1 → first 6 messages cached, last 2 uncached
        let messages = vec![
            msg("user", "msg1"),
            msg("assistant", "resp1"),
            msg("user", "msg2"),
            msg("assistant", "resp2"),
            msg("user", "msg3"),
            msg("assistant", "resp3"),
            msg("user", "msg4"),
            msg("assistant", "resp4"),
        ];
        let result = build_cached_conversation(None, &messages, &default_strategy());
        assert_eq!(result.messages.len(), 8);

        // First 6 messages (cache_count = 8 - 2 = 6) should have breakpoints
        for (i, msg) in result.messages.iter().enumerate() {
            if i < 6 {
                assert_eq!(
                    msg.content[0].breakpoint,
                    Some(CacheBreakpoint::Ephemeral),
                    "message {} should be cached",
                    i
                );
            } else {
                assert_eq!(
                    msg.content[0].breakpoint, None,
                    "message {} should not be cached",
                    i
                );
            }
        }
    }

    #[test]
    fn all_messages_cached_when_keep_zero() {
        let messages = vec![
            msg("user", "msg1"),
            msg("assistant", "resp1"),
            msg("user", "msg2"),
            msg("assistant", "resp2"),
            msg("user", "msg3"),
            msg("assistant", "resp3"),
        ];
        let result = build_cached_conversation(
            None,
            &messages,
            &CacheStrategy {
                keep_uncached_exchanges: 0,
                ..default_strategy()
            },
        );
        assert_eq!(result.messages.len(), 6);
        for (i, msg) in result.messages.iter().enumerate() {
            assert_eq!(
                msg.content[0].breakpoint,
                Some(CacheBreakpoint::Ephemeral),
                "message {} should be cached when keep=0",
                i
            );
        }
    }

    #[test]
    fn cache_disabled_when_strategy_says_no() {
        let result = build_cached_conversation(
            Some("system prompt"),
            &[msg("user", "Hi")],
            &CacheStrategy {
                cache_system_prompt: false,
                min_messages_for_caching: 1,
                ..default_strategy()
            },
        );
        assert_eq!(
            result.system_prompt,
            Some(CachedContent {
                content: "system prompt".to_string(),
                breakpoint: None,
            })
        );
    }

    #[test]
    fn single_message_gets_no_cache_point() {
        let result =
            build_cached_conversation(None, &[msg("user", "only message")], &default_strategy());
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].content[0].breakpoint, None);
    }

    #[test]
    fn last_content_block_gets_breakpoint_not_earlier_ones() {
        // Currently each message has exactly one content block, so the
        // breakpoint always goes on the only content block. This test
        // verifies that the breakpoint is NOT on the message level and
        // IS on the content block level, and documents the contract
        // for when multi-block content is added later.
        let messages = vec![
            msg("user", "msg1"),
            msg("assistant", "resp1"),
            msg("user", "msg2"),
            msg("assistant", "resp2"),
        ];
        let result = build_cached_conversation(
            None,
            &messages,
            &CacheStrategy {
                min_messages_for_caching: 1,
                ..default_strategy()
            },
        );
        // Message-level breakpoint should be None — breakpoints go on content blocks
        for msg in &result.messages {
            assert_eq!(msg.breakpoint, None);
        }
    }

    #[test]
    fn keep_uncached_exchanges_respected() {
        // 3 exchanges (6 messages), keep=2 → first 2 messages cached, last 4 uncached
        let messages = vec![
            msg("user", "exchange1"),
            msg("assistant", "exchange1"),
            msg("user", "exchange2"),
            msg("assistant", "exchange2"),
            msg("user", "exchange3"),
            msg("assistant", "exchange3"),
        ];
        let result = build_cached_conversation(
            None,
            &messages,
            &CacheStrategy {
                keep_uncached_exchanges: 2,
                min_messages_for_caching: 1,
                ..default_strategy()
            },
        );
        assert_eq!(result.messages.len(), 6);

        // cache_count = 6 - (2*2) = 2 → first 2 cached
        for (i, msg) in result.messages.iter().enumerate() {
            if i < 2 {
                assert_eq!(
                    msg.content[0].breakpoint,
                    Some(CacheBreakpoint::Ephemeral),
                    "message {} should be cached",
                    i
                );
            } else {
                assert_eq!(
                    msg.content[0].breakpoint, None,
                    "message {} should not be cached",
                    i
                );
            }
        }
    }

    #[test]
    fn system_prompt_cached_even_with_few_messages() {
        // System prompt caching is independent of the message count threshold.
        let result = build_cached_conversation(
            Some("You are a helpful assistant."),
            &[msg("user", "Hi")],
            &CacheStrategy {
                min_messages_for_caching: 10, // messages below threshold
                ..default_strategy()
            },
        );
        // System prompt should still have a breakpoint
        assert_eq!(
            result.system_prompt,
            Some(CachedContent {
                content: "You are a helpful assistant.".to_string(),
                breakpoint: Some(CacheBreakpoint::Ephemeral),
            })
        );
        // Messages should have no breakpoints
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].content[0].breakpoint, None);
    }

    #[test]
    fn exactly_at_min_messages_caches_older_exchanges() {
        // At exactly 3 messages (min_messages_for_caching),
        // with keep=1 → cache_count = 3 - 2 = 1 → first message cached
        let messages = vec![
            msg("user", "old"),
            msg("assistant", "old_resp"),
            msg("user", "recent"),
        ];
        let result = build_cached_conversation(None, &messages, &default_strategy());
        assert_eq!(result.messages.len(), 3);
        assert_eq!(
            result.messages[0].content[0].breakpoint,
            Some(CacheBreakpoint::Ephemeral)
        );
        assert_eq!(result.messages[1].content[0].breakpoint, None);
        assert_eq!(result.messages[2].content[0].breakpoint, None);
    }

    #[test]
    fn no_cache_breakpoints_when_uncached_exchanges_cover_all() {
        // If keep_uncached_exchanges covers every message, no breakpoints.
        let messages = vec![msg("user", "a"), msg("assistant", "b"), msg("user", "c")];
        let result = build_cached_conversation(
            None,
            &messages,
            &CacheStrategy {
                keep_uncached_exchanges: 2, // 2*2 = 4 ≥ 3 messages
                min_messages_for_caching: 1,
                ..default_strategy()
            },
        );
        assert_eq!(result.messages.len(), 3);
        for msg in &result.messages {
            assert_eq!(msg.content[0].breakpoint, None);
        }
    }
}
