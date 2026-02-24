use serde_json::json;
use wasi::http::types::{
    Fields, IncomingRequest, Method, OutgoingBody, OutgoingResponse, ResponseOutparam,
};

wit_bindgen::generate!({
    world: "mcp",
    generate_all,
});

mod actions;
mod config;
mod mcp;
mod types;
mod validation;

use exports::wasi::http::incoming_handler::Guest as McpHandler;

pub(crate) const MAX_REQUEST_BODY_SIZE: usize = 10 * 1024 * 1024; // 10Mb

struct Component;

impl McpHandler for Component {
    fn handle(request: IncomingRequest, response_out: ResponseOutparam) {
        inner_handle(request, response_out);
    }
}

fn inner_handle(request: IncomingRequest, response_out: ResponseOutparam) {
    match (request.method(), request.path_with_query().as_deref()) {
        (Method::Post, Some(path)) if path.starts_with("/mcp/") => {
            handle_mcp_request(request, response_out, path);
        }
        _ => {
            send_response(
                response_out,
                405,
                json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32000,
                        "message": "Method Not Allowed. Expected POST to /mcp/{server-id}"
                    },
                    "id": null
                })
                .to_string(),
            );
        }
    }
}

fn handle_mcp_request(request: IncomingRequest, response_out: ResponseOutparam, path: &str) {
    if let Err(e) = validate_content_type(&request) {
        send_json_rpc_error(response_out, 400, -32600, &e);
        return;
    }

    let server_id = match extract_server_id_from_path(path) {
        Ok(id) => id,
        Err(e) => {
            send_json_rpc_error(response_out, 400, -32600, &e);
            return;
        }
    };

    let headers = request.headers().entries();

    let body = match read_request_body(&request) {
        Ok(b) => b,
        Err(e) => {
            send_json_rpc_error(response_out, 400, -32700, &e);
            return;
        }
    };

    match crate::mcp::process_rpc(&server_id, &body, &headers) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(body_str) => send_response(response_out, 200, body_str),
            Err(_) => {
                send_json_rpc_error(response_out, 500, -32603, "Internal server error");
            }
        },
        Err(error_response) => match serde_json::to_string(&error_response) {
            Ok(body_str) => send_response(response_out, 200, body_str),
            Err(_) => {
                send_json_rpc_error(response_out, 500, -32603, "Internal server error");
            }
        },
    }
}

fn validate_content_type(request: &IncomingRequest) -> Result<(), String> {
    request
        .headers()
        .entries()
        .iter()
        .find(|(key, value)| {
            key.eq_ignore_ascii_case("content-type")
                && String::from_utf8_lossy(value).contains("application/json")
        })
        .map(|_| ())
        .ok_or_else(|| "Content-Type must be application/json".to_string())
}

fn extract_server_id_from_path(path: &str) -> Result<String, String> {
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() >= 3 {
        let server_id = parts[2].split('?').next().unwrap_or(parts[2]);
        if server_id.is_empty() {
            Err("Server ID cannot be empty. Expected /mcp/{server-id}".to_string())
        } else {
            Ok(server_id.to_string())
        }
    } else {
        Err("Invalid path format. Expected /mcp/{server-id}".to_string())
    }
}

pub(crate) fn read_request_body(request: &IncomingRequest) -> Result<String, String> {
    let body_stream = request.consume().map_err(|_| "Failed to get body stream")?;
    let input_stream = body_stream
        .stream()
        .map_err(|_| "Failed to get input stream")?;

    let mut buf = Vec::new();
    while let Ok(chunk) = input_stream.blocking_read(64 * 1024) {
        if chunk.is_empty() {
            break;
        }
        if buf.len() + chunk.len() > MAX_REQUEST_BODY_SIZE {
            return Err(format!(
                "Request body exceeds maximum size of {} bytes",
                MAX_REQUEST_BODY_SIZE
            ));
        }
        buf.extend_from_slice(&chunk);
    }

    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8 in body: {}", e))
}

fn send_json_rpc_error(response_out: ResponseOutparam, status: u16, code: i32, message: &str) {
    let error_body = json!({
        "jsonrpc": "2.0",
        "error": {
            "code": code,
            "message": message
        },
        "id": null
    });
    send_response(response_out, status, error_body.to_string());
}

fn send_response(response_out: ResponseOutparam, status: u16, body: String) {
    let headers = Fields::new();
    if let Err(e) = headers.set("content-type", &[b"application/json".to_vec()]) {
        eprintln!("Failed to set content-type header: {:?}", e);
    }

    let response = OutgoingResponse::new(headers);
    if let Err(e) = response.set_status_code(status) {
        eprintln!("Failed to set status code: {:?}", e);
        return;
    }

    let response_body = match response.body() {
        Ok(rb) => rb,
        Err(e) => {
            eprintln!("Failed to get response body: {:?}", e);
            return;
        }
    };
    ResponseOutparam::set(response_out, Ok(response));

    let output_stream = match response_body.write() {
        Ok(os) => os,
        Err(e) => {
            eprintln!("Failed to get output stream: {:?}", e);
            return;
        }
    };
    if let Err(e) = output_stream.blocking_write_and_flush(body.as_bytes()) {
        eprintln!("Failed to write response: {:?}", e);
        return;
    }

    drop(output_stream);
    if let Err(e) = OutgoingBody::finish(response_body, None) {
        eprintln!("Failed to finish body: {:?}", e);
    }
}

export!(Component);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_server_id_valid() {
        let result = extract_server_id_from_path("/mcp/weather-server-001");
        assert_eq!(result.unwrap(), "weather-server-001");
    }

    #[test]
    fn test_extract_server_id_with_query_params() {
        let result = extract_server_id_from_path("/mcp/server-123?key=value");
        assert_eq!(result.unwrap(), "server-123");
    }

    #[test]
    fn test_extract_server_id_with_trailing_path() {
        let result = extract_server_id_from_path("/mcp/server-123/extra/path");
        assert_eq!(result.unwrap(), "server-123");
    }

    #[test]
    fn test_extract_server_id_empty() {
        let result = extract_server_id_from_path("/mcp/");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Server ID cannot be empty"));
    }

    #[test]
    fn test_extract_server_id_too_short() {
        let result = extract_server_id_from_path("/mcp");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_server_id_root_path() {
        let result = extract_server_id_from_path("/");
        assert!(result.is_err());
    }
}
