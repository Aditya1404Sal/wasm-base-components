use serde_json::json;

wit_bindgen::generate!({
    world: "mcp",
    generate_all,
});

use exports::wasi::http::incoming_handler::Guest as McpHandler;

mod actions;
mod config;
mod mcp;
mod types;

use crate::betty_blocks::auth::jwt::validate_token;

struct Component;

impl McpHandler for Component {
    fn handle(
        request: crate::wasi::http::types::IncomingRequest,
        response_out: crate::wasi::http::types::ResponseOutparam,
    ) {
        inner_handle(request, response_out);
    }
}

fn inner_handle(
    request: crate::wasi::http::types::IncomingRequest,
    response_out: crate::wasi::http::types::ResponseOutparam,
) {
    use crate::wasi::http::types::Method;

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

fn handle_mcp_request(
    request: crate::wasi::http::types::IncomingRequest,
    response_out: crate::wasi::http::types::ResponseOutparam,
    path: &str,
) {
    let server_id = match extract_server_id_from_path(path) {
        Ok(id) => id,
        Err(e) => {
            send_json_rpc_error(response_out, 400, -32600, &e);
            return;
        }
    };

    if let Err(e) = validate_content_type(&request) {
        send_json_rpc_error(response_out, 400, -32600, &e);
        return;
    }

    // Authenticate request JWT
    let auth_headers: Vec<(String, String)> = request
        .headers()
        .entries()
        .into_iter()
        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
        .collect();
    if validate_token(&auth_headers).is_err() {
        send_json_rpc_error(response_out, 401, -32000, "Unauthorized");
        return;
    }

    let body = match read_request_body(&request) {
        Ok(b) => b,
        Err(e) => {
            send_json_rpc_error(response_out, 400, -32700, &e);
            return;
        }
    };

    match mcp::process_rpc(&server_id, &body) {
        Ok(result) => match serde_json::to_string(&result) {
            Ok(body_str) => send_response(response_out, 200, body_str),
            Err(e) => {
                eprintln!("Failed to serialize JsonrpcResponse: {}", e);
                send_json_rpc_error(response_out, 500, -32603, "Internal server error");
            }
        },
        Err(error_response) => {
            // Return the structured JsonrpcErrorResponse directly
            match serde_json::to_string(&error_response) {
                Ok(body_str) => send_response(response_out, 200, body_str),
                Err(e) => {
                    eprintln!("Failed to serialize JsonrpcErrorResponse: {}", e);
                    send_json_rpc_error(response_out, 500, -32603, "Internal server error");
                }
            }
        }
    }
}

fn validate_content_type(
    request: &crate::wasi::http::types::IncomingRequest,
) -> Result<(), String> {
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

    if parts.len() >= 3 && parts[1] == "mcp" {
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

fn read_request_body(
    request: &crate::wasi::http::types::IncomingRequest,
) -> Result<String, String> {
    let body_stream = request.consume().map_err(|_| "Failed to get body stream")?;
    let input_stream = body_stream
        .stream()
        .map_err(|_| "Failed to get input stream")?;

    let mut buf = Vec::new();
    while let Ok(chunk) = input_stream.blocking_read(1024 * 1024) {
        if chunk.is_empty() {
            break;
        }
        buf.extend_from_slice(&chunk);
    }

    String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8 in body: {}", e))
}

fn send_json_rpc_error(
    response_out: crate::wasi::http::types::ResponseOutparam,
    status: u16,
    code: i32,
    message: &str,
) {
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

fn send_response(
    response_out: crate::wasi::http::types::ResponseOutparam,
    status: u16,
    body: String,
) {
    use crate::wasi::http::types::{Fields, OutgoingBody, OutgoingResponse};

    let headers = Fields::new();
    let _ = headers.set("content-type", &[b"application/json".to_vec()]);

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
    crate::wasi::http::types::ResponseOutparam::set(response_out, Ok(response));

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
