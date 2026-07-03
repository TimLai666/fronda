//! Pure adapters between the agent loop (`agent_contract`) and the chat model
//! (`app_contract`). Kept out of the gpui view so it is unit-testable.

use agent_contract::ToolCallRecord;
use app_contract::chat_model::{ToolCall, ToolCallStatus};

/// Map one executed tool record to a chat `ToolCall` for display: pretty-printed
/// input arguments, a Done/Failed status, and the result (or error) text.
pub fn tool_call_from_record(record: &ToolCallRecord) -> ToolCall {
    let (status, result_text) = match &record.result {
        Ok(value) => (ToolCallStatus::Done, Some(value_to_text(value))),
        Err(err) => (ToolCallStatus::Failed, Some(err.clone())),
    };
    ToolCall {
        name: record.name.clone(),
        status,
        input_json: serde_json::to_string_pretty(&record.input).ok(),
        result_text,
    }
}

/// Map a full turn's tool records to chat tool calls, in order.
pub fn tool_calls_from_records(records: &[ToolCallRecord]) -> Vec<ToolCall> {
    records.iter().map(tool_call_from_record).collect()
}

/// A JSON string result reads best unquoted; everything else is compact JSON.
fn value_to_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_success_record() {
        let record = ToolCallRecord {
            name: "get_timeline".into(),
            input: json!({ "verbose": true }),
            result: Ok(json!({ "clips": 3 })),
        };
        let call = tool_call_from_record(&record);
        assert_eq!(call.name, "get_timeline");
        assert_eq!(call.status, ToolCallStatus::Done);
        assert!(call.input_json.unwrap().contains("verbose"));
        assert_eq!(call.result_text.as_deref(), Some("{\"clips\":3}"));
    }

    #[test]
    fn maps_failure_record() {
        let record = ToolCallRecord {
            name: "bad_tool".into(),
            input: json!({}),
            result: Err("unknown tool".into()),
        };
        let call = tool_call_from_record(&record);
        assert_eq!(call.status, ToolCallStatus::Failed);
        assert_eq!(call.result_text.as_deref(), Some("unknown tool"));
    }

    #[test]
    fn string_result_is_unquoted() {
        let record = ToolCallRecord {
            name: "note".into(),
            input: json!({}),
            result: Ok(json!("done")),
        };
        assert_eq!(
            tool_call_from_record(&record).result_text.as_deref(),
            Some("done")
        );
    }

    #[test]
    fn maps_a_full_turn_in_order() {
        let records = vec![
            ToolCallRecord {
                name: "a".into(),
                input: json!({}),
                result: Ok(json!(1)),
            },
            ToolCallRecord {
                name: "b".into(),
                input: json!({}),
                result: Err("x".into()),
            },
        ];
        let calls = tool_calls_from_records(&records);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[1].status, ToolCallStatus::Failed);
    }
}
