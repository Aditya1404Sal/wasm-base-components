use serde_json::json;

wit_bindgen::generate!({
    world: "mcp",
    generate_all,
});

mod actions;
mod config;
mod mcp;
mod types;
mod validation;

use crate::betty_blocks::auth::jwt::{validate_token, AuthError};
use exports::wasi::http::incoming_handler::Guest as McpHandler;

const MAX_REQUEST_BODY_SIZE: usize = 10 * 1024 * 1024; // 10Mb : a mcp request is typically in Kbs (Safe limit I'd say??)

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
    let token = match extract_bearer_token(&auth_headers) {
        Ok(t) => t,
        Err(e) => {
            send_json_rpc_error(response_out, 401, -32000, &e);
            return;
        }
    };
    // Claims to be returned here, would be used by authorization component
    if let Err(auth_err) = validate_token(&token) {
        let msg = format_auth_error(&auth_err);
        send_json_rpc_error(response_out, 401, -32000, &msg);
        return;
    }

    let body = match read_request_body(&request) {
        Ok(b) => b,
        Err(e) => {
            send_json_rpc_error(response_out, 400, -32700, &e);
            return;
        }
    };

    match crate::mcp::process_rpc(&server_id, &body) {
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

/// Extracts the Bearer token from the Authorization header
fn extract_bearer_token(headers: &[(String, String)]) -> Result<String, String> {
    let auth_header = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
        .ok_or_else(|| "Missing Authorization header".to_string())?;
    let value = auth_header.1.trim();
    let token = value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != "null")
        .ok_or_else(|| "Invalid Authorization header format".to_string())?;
    Ok(token.to_string())
}

fn format_auth_error(err: &AuthError) -> String {
    match err {
        AuthError::MalformedToken => "Malformed JWT token".to_string(),
        AuthError::UnsupportedAlgorithm(detail) => {
            format!("Unsupported algorithm: {}", detail)
        }
        AuthError::MissingConfig(key) => {
            format!("Missing server configuration: {}", key)
        }
        AuthError::InvalidPublicKey(detail) => {
            format!("Invalid public key: {}", detail)
        }
        AuthError::ValidationFailed(detail) => {
            format!("Token validation failed: {}", detail)
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
    while let Ok(chunk) = input_stream.blocking_read(64 * 1024) {
        if chunk.is_empty() {
            break;
        }
        if buf.len() + chunk.len() > MAX_REQUEST_BODY_SIZE {
            return Err(format!(
                "Request body is too large. Maximum size is {} bytes. Possibility of malicious payload",
                MAX_REQUEST_BODY_SIZE
            ));
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_bearer_token tests ---

    #[test]
    fn test_extract_bearer_token_valid() {
        let headers = vec![("Authorization".to_string(), "Bearer abc123".to_string())];
        let result = extract_bearer_token(&headers);
        assert_eq!(result.unwrap(), "abc123");
    }

    #[test]
    fn test_extract_bearer_token_with_extra_whitespace() {
        let headers = vec![(
            "Authorization".to_string(),
            "Bearer   token_value  ".to_string(),
        )];
        let result = extract_bearer_token(&headers);
        assert_eq!(result.unwrap(), "token_value");
    }

    #[test]
    fn test_extract_bearer_token_case_insensitive_header() {
        let headers = vec![("authorization".to_string(), "Bearer mytoken".to_string())];
        assert_eq!(extract_bearer_token(&headers).unwrap(), "mytoken");

        let headers = vec![("AUTHORIZATION".to_string(), "Bearer mytoken".to_string())];
        assert_eq!(extract_bearer_token(&headers).unwrap(), "mytoken");
    }

    #[test]
    fn test_extract_bearer_token_missing_header() {
        let headers: Vec<(String, String)> = vec![];
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing Authorization header"));
    }

    #[test]
    fn test_extract_bearer_token_wrong_scheme() {
        let headers = vec![("Authorization".to_string(), "Basic abc123".to_string())];
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Invalid Authorization header format"));
    }

    #[test]
    fn test_extract_bearer_token_empty_token() {
        let headers = vec![("Authorization".to_string(), "Bearer ".to_string())];
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token_null_token() {
        let headers = vec![("Authorization".to_string(), "Bearer null".to_string())];
        let result = extract_bearer_token(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_bearer_token_among_other_headers() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("Authorization".to_string(), "Bearer found_it".to_string()),
            ("Accept".to_string(), "*/*".to_string()),
        ];
        assert_eq!(extract_bearer_token(&headers).unwrap(), "found_it");
    }

    // --- extract_server_id_from_path tests ---

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
    fn test_extract_server_id_invalid_path_no_mcp() {
        let result = extract_server_id_from_path("/api/server-123");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid path format"));
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

    // --- format_auth_error tests ---

    #[test]
    fn test_format_auth_error_malformed_token() {
        let msg = format_auth_error(&AuthError::MalformedToken);
        assert_eq!(msg, "Malformed JWT token");
    }

    #[test]
    fn test_format_auth_error_unsupported_algorithm() {
        let msg = format_auth_error(&AuthError::UnsupportedAlgorithm("HS256".to_string()));
        assert!(msg.contains("Unsupported algorithm"));
        assert!(msg.contains("HS256"));
    }

    #[test]
    fn test_format_auth_error_missing_config() {
        let msg = format_auth_error(&AuthError::MissingConfig("JWT_PUBLIC_KEY".to_string()));
        assert!(msg.contains("Missing server configuration"));
        assert!(msg.contains("JWT_PUBLIC_KEY"));
    }

    #[test]
    fn test_format_auth_error_invalid_public_key() {
        let msg = format_auth_error(&AuthError::InvalidPublicKey("bad PEM".to_string()));
        assert!(msg.contains("Invalid public key"));
        assert!(msg.contains("bad PEM"));
    }

    #[test]
    fn test_format_auth_error_validation_failed() {
        let msg = format_auth_error(&AuthError::ValidationFailed("token expired".to_string()));
        assert!(msg.contains("Token validation failed"));
        assert!(msg.contains("token expired"));
    }
}
