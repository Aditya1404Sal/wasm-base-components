use crate::actions;
use crate::config;
use crate::types::*;
use rust_mcp_schema::{
    CallToolRequestParams, CallToolResult, ContentBlock, JsonrpcErrorResponse, JsonrpcRequest,
    JsonrpcResponse, ListToolsResult, Tool, JSONRPC_VERSION, LATEST_PROTOCOL_VERSION,
};
use serde_json::{json, Value};

pub fn process_rpc(server_id: &str, body: &str) -> Result<JsonrpcResponse, JsonrpcErrorResponse> {
    let raw: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return Err(make_error_response_or_fallback(
                None,
                -32700,
                &format!("Invalid JSON-RPC request: {}", e),
            ));
        }
    };

    let id = raw.get("id").cloned();

    let request: JsonrpcRequest = match serde_json::from_value(raw.clone()) {
        Ok(r) => r,
        Err(e) => {
            return Err(make_error_response_or_fallback(
                id,
                -32600,
                &format!("Invalid JSON-RPC request: {}", e),
            ));
        }
    };

    if request.jsonrpc() != JSONRPC_VERSION {
        return Err(make_error_response_or_fallback(
            id,
            -32600,
            "Invalid Request: jsonrpc must be '2.0'",
        ));
    }

    let params = raw.get("params").cloned().unwrap_or(json!({}));

    let server_config = match config::load_server_config(server_id) {
        Ok(cfg) => cfg,
        Err(e) => {
            return Err(make_error_response_or_fallback(
                id,
                -32000,
                &format!("Failed to load server config: {}", e),
            ));
        }
    };

    let result = match request.method.as_str() {
        "initialize" => handle_initialize(),
        "tools/list" => handle_list_tools(&server_config),
        "tools/call" => handle_call_tool(&params, &server_config),
        _ => {
            return Err(make_error_response_or_fallback(
                id,
                -32601,
                &format!("Method not found: {}", request.method),
            ));
        }
    };

    match result {
        Ok(result_value) => match create_success_response(id.clone(), result_value) {
            Ok(resp) => Ok(resp),
            Err(e) => Err(make_error_response_or_fallback(
                id,
                -32603,
                &format!("Failed to build response: {}", e),
            )),
        },
        Err(e) => Err(make_error_response_or_fallback(id, -32000, &e)),
    }
}

fn make_error_response_or_fallback(
    id: Option<Value>,
    code: i32,
    message: &str,
) -> JsonrpcErrorResponse {
    match create_error_response(id.clone(), code, message) {
        Ok(resp) => resp,
        Err(_) => serde_json::from_value(json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": id.unwrap_or(json!(null)),
            "error": {
                "code": code,
                "message": message
            }
        }))
        .expect("fallback error response must deserialize"),
    }
}

fn handle_initialize() -> Result<Value, String> {
    let mut init_result = crate::config::load_initialize_result()?;

    if init_result.protocol_version.is_empty() {
        init_result.protocol_version = LATEST_PROTOCOL_VERSION.to_string();
    }

    serde_json::to_value(init_result)
        .map_err(|e| format!("Failed to serialize initialize result: {}", e))
}

fn handle_list_tools(server_config: &McpServerConfig) -> Result<Value, String> {
    let tools: Vec<Tool> = server_config.tools.iter().map(|t| t.tool.clone()).collect();

    let result = ListToolsResult {
        tools,
        next_cursor: None,
        meta: None,
    };

    serde_json::to_value(result).map_err(|e| format!("Failed to serialize tools list: {}", e))
}

//Note(Aditya): I need to know what the RunPayload.input and configurations correspond to
// http-wrapper also had RunPayload but that had generic objects in test cases :(
fn handle_call_tool(params: &Value, server_config: &McpServerConfig) -> Result<Value, String> {
    // Parse tool call parameters
    let call_params: CallToolRequestParams = serde_json::from_value(params.clone())
        .map_err(|e| format!("Invalid tool call parameters: {}", e))?;

    // Find the tool in the configuration
    let tool_with_action = server_config
        .tools
        .iter()
        .find(|t| t.tool.name == call_params.name)
        .ok_or_else(|| format!("Tool '{}' not found", call_params.name))?;

    // Validate arguments against input schema
    if let Some(args) = &call_params.arguments {
        crate::validation::validate_arguments(args, &tool_with_action.tool.input_schema)?;
    }

    // Execute the mapped action
    let args_value = call_params
        .arguments
        .as_ref()
        .map(|m| Value::Object(m.clone()))
        .unwrap_or(Value::Null);
    let action_result: ActionResponse =
        actions::execute_mapped_action(&tool_with_action.action_id, &args_value)?;

    let content: Vec<ContentBlock> = actions::parse_action_output(&action_result)?;

    let result = CallToolResult {
        content,
        structured_content: None,
        is_error: Some(!action_result.success),
        meta: None,
    };

    serde_json::to_value(result).map_err(|e| format!("Failed to serialize tool result: {}", e))
}

pub fn create_success_response(
    id: Option<Value>,
    result: Value,
) -> Result<JsonrpcResponse, String> {
    let response_value = json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id.unwrap_or(json!(null)),
        "result": result
    });

    let parsed: JsonrpcResponse = serde_json::from_value(response_value)
        .map_err(|e| format!("Response does not match JsonrpcResponse schema: {}", e))?;

    Ok(parsed)
}

pub fn create_error_response(
    id: Option<Value>,
    code: i32,
    message: &str,
) -> Result<JsonrpcErrorResponse, String> {
    let error_obj = json!({
        "code": code,
        "message": message
    });

    let response_value = json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id.unwrap_or(json!(null)),
        "error": error_obj
    });

    let parsed: JsonrpcErrorResponse = serde_json::from_value(response_value).map_err(|e| {
        format!(
            "Error response does not match JsonrpcErrorResponse schema: {}",
            e
        )
    })?;

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_test_server_config() -> McpServerConfig {
        serde_json::from_value(json!({
            "id": "test-server",
            "tools": [
                {
                    "name": "get_weather",
                    "description": "Get weather for a location",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "location": { "type": "string" }
                        },
                        "required": ["location"]
                    },
                    "action-id": "action-001"
                },
                {
                    "name": "add_numbers",
                    "description": "Add two numbers",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "a": { "type": "number" },
                            "b": { "type": "number" }
                        },
                        "required": ["a", "b"]
                    },
                    "action-id": "action-002"
                }
            ]
        }))
        .expect("test server config must parse")
    }

    // --- create_success_response tests ---

    #[test]
    fn test_create_success_response_with_numeric_id() {
        let result = create_success_response(Some(json!(1)), json!({"status": "ok"}));
        assert!(result.is_ok());

        let serialized = serde_json::to_value(result.unwrap()).unwrap();
        assert_eq!(serialized["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(serialized["id"], json!(1));
        assert_eq!(serialized["result"]["status"], "ok");
    }

    #[test]
    fn test_create_success_response_with_string_id() {
        let result = create_success_response(Some(json!("req-42")), json!({"data": [1, 2, 3]}));
        assert!(result.is_ok());

        let serialized = serde_json::to_value(result.unwrap()).unwrap();
        assert_eq!(serialized["id"], "req-42");
    }

    #[test]
    fn test_create_success_response_with_null_id_fails() {
        // JsonrpcResponse requires a valid RequestId, so None (null) is rejected
        let result = create_success_response(None, json!({}));
        assert!(result.is_err());
    }

    // --- create_error_response tests ---

    #[test]
    fn test_create_error_response_method_not_found() {
        let result = create_error_response(Some(json!(1)), -32601, "Method not found");
        assert!(result.is_ok());

        let serialized = serde_json::to_value(result.unwrap()).unwrap();
        assert_eq!(serialized["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(serialized["id"], 1);
        assert_eq!(serialized["error"]["code"], -32601);
        assert_eq!(serialized["error"]["message"], "Method not found");
    }

    #[test]
    fn test_create_error_response_parse_error() {
        let result = create_error_response(None, -32700, "Parse error");
        assert!(result.is_ok());

        let serialized = serde_json::to_value(result.unwrap()).unwrap();
        assert!(serialized["id"].is_null());
        assert_eq!(serialized["error"]["code"], -32700);
    }

    #[test]
    fn test_create_error_response_invalid_request() {
        let result = create_error_response(Some(json!("abc")), -32600, "Invalid Request");
        assert!(result.is_ok());

        let serialized = serde_json::to_value(result.unwrap()).unwrap();
        assert_eq!(serialized["error"]["code"], -32600);
        assert_eq!(serialized["error"]["message"], "Invalid Request");
    }

    #[test]
    fn test_create_error_response_internal_error() {
        let result = create_error_response(Some(json!(99)), -32603, "Internal error");
        assert!(result.is_ok());
    }

    // --- make_error_response_or_fallback tests ---

    #[test]
    fn test_make_error_response_with_id() {
        let resp = make_error_response_or_fallback(Some(json!(5)), -32600, "Bad request");
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["id"], 5);
        assert_eq!(serialized["error"]["code"], -32600);
        assert_eq!(serialized["error"]["message"], "Bad request");
    }

    #[test]
    fn test_make_error_response_without_id() {
        let resp = make_error_response_or_fallback(None, -32700, "Parse error");
        let serialized = serde_json::to_value(&resp).unwrap();
        assert!(serialized["id"].is_null());
        assert_eq!(serialized["error"]["code"], -32700);
    }

    // --- handle_list_tools tests ---

    #[test]
    fn test_handle_list_tools_returns_all_tools() {
        let config = make_test_server_config();
        let result = handle_list_tools(&config).unwrap();

        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "get_weather");
        assert_eq!(tools[1]["name"], "add_numbers");
    }

    #[test]
    fn test_handle_list_tools_includes_descriptions() {
        let config = make_test_server_config();
        let result = handle_list_tools(&config).unwrap();

        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools[0]["description"], "Get weather for a location");
        assert_eq!(tools[1]["description"], "Add two numbers");
    }

    #[test]
    fn test_handle_list_tools_includes_input_schema() {
        let config = make_test_server_config();
        let result = handle_list_tools(&config).unwrap();

        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
        let required = tools[0]["inputSchema"]["required"].as_array().unwrap();
        assert_eq!(required, &[json!("location")]);
    }

    #[test]
    fn test_handle_list_tools_empty_server() {
        let config: McpServerConfig = serde_json::from_value(json!({
            "id": "empty-server",
            "tools": []
        }))
        .unwrap();

        let result = handle_list_tools(&config).unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_handle_list_tools_no_next_cursor() {
        let config = make_test_server_config();
        let result = handle_list_tools(&config).unwrap();
        assert!(result.get("nextCursor").is_none() || result["nextCursor"].is_null());
    }

    // --- handle_call_tool param parsing tests ---
    // (action execution requires WASI runtime, so we test the validation/parsing path)

    #[test]
    fn test_handle_call_tool_unknown_tool() {
        let config = make_test_server_config();
        let params = json!({ "name": "nonexistent_tool" });
        let result = handle_call_tool(&params, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_handle_call_tool_invalid_params() {
        let config = make_test_server_config();
        let params = json!({ "invalid": true });
        let result = handle_call_tool(&params, &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid tool call parameters"));
    }

    #[test]
    fn test_handle_call_tool_invalid_arguments_against_schema() {
        let config = make_test_server_config();
        // "location" is required as a string, passing a number should fail validation
        let params = json!({
            "name": "get_weather",
            "arguments": { "location": 123 }
        });
        let result = handle_call_tool(&params, &config);
        assert!(result.is_err());
    }

    // --- round-trip serialization tests ---

    #[test]
    fn test_success_response_round_trip() {
        let original_result = json!({
            "tools": [{"name": "test", "inputSchema": {"type": "object"}}]
        });
        let resp = create_success_response(Some(json!(1)), original_result.clone()).unwrap();
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(deserialized["id"], 1);
        assert_eq!(deserialized["result"], original_result);
    }

    #[test]
    fn test_error_response_round_trip() {
        let resp = create_error_response(Some(json!(42)), -32601, "Method not found").unwrap();
        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized["jsonrpc"], JSONRPC_VERSION);
        assert_eq!(deserialized["id"], 42);
        assert_eq!(deserialized["error"]["code"], -32601);
        assert_eq!(deserialized["error"]["message"], "Method not found");
    }
}
