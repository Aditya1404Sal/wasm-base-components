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
        "initialize" => handle_initialize(&params),
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

fn handle_initialize(params: &Value) -> Result<Value, String> {
    let protocol_version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(LATEST_PROTOCOL_VERSION);

    let capabilities = params.get("capabilities").cloned().unwrap_or(json!({}));

    Ok(json!({
        "protocolVersion": protocol_version,
        "capabilities": capabilities,
        "serverInfo": {
            "name": "betty-mcp-server",
            "version": "0.1.0"
        }
    }))
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
