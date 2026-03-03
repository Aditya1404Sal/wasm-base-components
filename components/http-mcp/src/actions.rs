use crate::wasi::http::outgoing_handler::handle as http_handle;
use crate::wasi::http::types::{
    Fields, Method, OutgoingBody, OutgoingRequest, Scheme,
};
use crate::wasi::io::poll::poll;
use rust_mcp_schema::ContentBlock;
use serde_json::{json, Value};

pub fn execute_mapped_action(
    action_id: &str,
    arguments: &Value,
    configurations: &str,
    wasmcloud_host: &str,
    application_id: &str,
) -> Result<(bool, Vec<ContentBlock>), String> {
    let input_json = serde_json::to_string(arguments)
        .map_err(|e| format!("Failed to serialize arguments: {}", e))?;

    let request_body = json!({
        "action_id": action_id,
        "payload": {
            "input": input_json,
            "configurations": serde_json::from_str::<Value>(configurations).unwrap_or(json!([]))
        },
        "jwt": ""
    });

    let body_bytes = serde_json::to_vec(&request_body)
        .map_err(|e| format!("Failed to serialize request body: {}", e))?;

    let response_body = send_http_request(wasmcloud_host, application_id, &body_bytes)?;

    let data: Value = match serde_json::from_str(&response_body) {
        Ok(v) => v,
        Err(_) => {
            return Ok((
                true,
                vec![ContentBlock::text_content(format!(
                    "Failed to parse action response: {}",
                    response_body
                ))],
            ))
        }
    };
    let output_value = data.get("output").cloned().unwrap_or(Value::Null);
    Ok((false, parse_action_output(&output_value)))
}

fn parse_host_url(wasmcloud_host: &str) -> (Scheme, &str) {
    if let Some(authority) = wasmcloud_host.strip_prefix("https://") {
        (Scheme::Https, authority)
    } else if let Some(authority) = wasmcloud_host.strip_prefix("http://") {
        (Scheme::Http, authority)
    } else {
        (Scheme::Https, wasmcloud_host)
    }
}

fn send_http_request(
    wasmcloud_host: &str,
    application_id: &str,
    body_bytes: &[u8],
) -> Result<String, String> {
    let (scheme, authority) = parse_host_url(wasmcloud_host);

    let headers = Fields::new();
    headers
        .set("content-type", &[b"application/json".to_vec()])
        .map_err(|e| format!("Failed to set content-type header: {:?}", e))?;
    headers
        .set("host", &[application_id.as_bytes().to_vec()])
        .map_err(|e| format!("Failed to set host header: {:?}", e))?;

    let request = OutgoingRequest::new(headers);
    request
        .set_method(&Method::Post)
        .map_err(|_| "Failed to set method")?;
    request
        .set_scheme(Some(&scheme))
        .map_err(|_| "Failed to set scheme")?;
    request
        .set_authority(Some(authority))
        .map_err(|_| "Failed to set authority")?;

    let outgoing_body = request
        .body()
        .map_err(|_| "Failed to get outgoing body")?;
    let output_stream = outgoing_body
        .write()
        .map_err(|_| "Failed to get output stream")?;
    output_stream
        .blocking_write_and_flush(body_bytes)
        .map_err(|e| format!("Failed to write request body: {:?}", e))?;
    drop(output_stream);
    OutgoingBody::finish(outgoing_body, None)
        .map_err(|e| format!("Failed to finish outgoing body: {:?}", e))?;

    let future_response =
        http_handle(request, None).map_err(|e| format!("HTTP request failed: {:?}", e))?;

    let pollable = future_response.subscribe();
    poll(&[&pollable]);

    let response = future_response
        .get()
        .ok_or("Response not ready")?
        .map_err(|_| "Failed to get response")?
        .map_err(|e| format!("HTTP error: {:?}", e))?;

    let status = response.status();
    let incoming_body = response
        .consume()
        .map_err(|_| "Failed to consume response body")?;
    let input_stream = incoming_body
        .stream()
        .map_err(|_| "Failed to get response stream")?;

    let mut buf = Vec::new();
    loop {
        match input_stream.blocking_read(64 * 1024) {
            Ok(chunk) => {
                if chunk.is_empty() {
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Err(_) => break,
        }
    }
    drop(input_stream);

    let response_text =
        String::from_utf8(buf).map_err(|e| format!("Invalid UTF-8 in response: {}", e))?;

    if status < 200 || status >= 300 {
        return Err(format!(
            "Action HTTP request failed with status {}: {}",
            status, response_text
        ));
    }

    Ok(response_text)
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
