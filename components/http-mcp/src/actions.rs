use crate::betty_blocks::actions::actions::{call, Error as ActionError, RunInput, RunPayload};
use rust_mcp_schema::ContentBlock;
use serde_json::Value;

pub fn execute_mapped_action(
    action_id: &str,
    arguments: &Value,
    configurations: &str,
) -> Result<(bool, Vec<ContentBlock>), String> {
    let input_json = serde_json::to_string(arguments)
        .map_err(|e| format!("Failed to serialize arguments: {}", e))?;

    let run_input = RunInput {
        action_id: action_id.to_string(),
        payload: RunPayload {
            input: input_json,
            configurations: configurations.to_string(),
        },
    };

    match call(&run_input) {
        Ok(output) => {
            // Response body is {"output": <any JSON value>}. If "output" key is missing -> null.
            let data: Value = match serde_json::from_str(&output.result) {
                Ok(v) => v,
                Err(_) => {
                    return Ok((
                        true,
                        vec![ContentBlock::text_content(format!(
                            "Failed to parse action response: {}",
                            output.result
                        ))],
                    ))
                }
            };
            let output_value = data.get("output").cloned().unwrap_or(Value::Null);
            Ok((false, parse_action_output(&output_value)))
        }
        Err(ActionError::RunFailed(msg)) => Ok((
            true,
            vec![ContentBlock::text_content(format!(
                "Action failed: {}",
                msg
            ))],
        )),
        Err(ActionError::Forbidden) => Ok((
            true,
            vec![ContentBlock::text_content(
                "Action forbidden: insufficient permissions".to_string(),
            )],
        )),
    }
}

pub fn parse_action_output(output: &Value) -> Vec<ContentBlock> {
    match output {
        Value::Null => vec![],
        Value::String(s) => vec![ContentBlock::text_content(s.clone())],
        other => vec![ContentBlock::text_content(
            serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_action_output_null() {
        let content = parse_action_output(&Value::Null);
        assert!(
            content.is_empty(),
            "null output should produce no content blocks"
        );
    }

    #[test]
    fn test_parse_action_output_string() {
        let content = parse_action_output(&json!("Hello, world!"));
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::TextContent(t) => assert_eq!(t.text, "Hello, world!"),
            _ => panic!("expected TextContent"),
        }
    }

    #[test]
    fn test_parse_action_output_object() {
        let content = parse_action_output(&json!({"key": "value", "num": 42}));
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::TextContent(t) => {
                // Should be pretty-printed JSON
                assert!(t.text.contains("key"));
                assert!(t.text.contains("value"));
            }
            _ => panic!("expected TextContent"),
        }
    }

    #[test]
    fn test_parse_action_output_number() {
        let content = parse_action_output(&json!(42));
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::TextContent(t) => assert_eq!(t.text, "42"),
            _ => panic!("expected TextContent"),
        }
    }

    #[test]
    fn test_parse_action_output_array() {
        let content = parse_action_output(&json!([1, 2, 3]));
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::TextContent(t) => {
                assert!(t.text.contains('1'));
                assert!(t.text.contains('3'));
            }
            _ => panic!("expected TextContent"),
        }
    }
}
