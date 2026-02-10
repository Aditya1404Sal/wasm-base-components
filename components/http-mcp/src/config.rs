use crate::types::McpServerConfig;
use crate::wasi::config::store::get;
use rust_mcp_schema::InitializeResult;
use serde_json::Value;

const WASI_CONFIG_KEY: &str = "mcp_servers";
const MCP_INITIALIZE_KEY: &str = "meta_info";

pub fn load_server_config(server_id: &str) -> Result<McpServerConfig, String> {
    let raw = get(WASI_CONFIG_KEY)
        .map_err(|e| format!("Failed to get wasi config: {:?}", e))?
        .ok_or_else(|| "mcp_servers key not found in runtime configuration".to_string())?;

    parse_server_config(&raw, server_id)
}

pub fn load_initialize_result() -> Result<InitializeResult, String> {
    let value = get(MCP_INITIALIZE_KEY)
        .map_err(|e| format!("Failed to get wasi config: {:?}", e))?
        .ok_or_else(|| "meta_info key not found in runtime configuration".to_string())?;

    parse_meta_config(&value)
}

fn parse_server_config(raw: &str, server_id: &str) -> Result<McpServerConfig, String> {
    let parsed: Value = serde_json::from_str(raw)
        .map_err(|e| format!("Failed to parse mcp_servers config: {}", e))?;

    let servers = parsed
        .get("mcp-servers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "mcp-servers key missing or not an array".to_string())?;

    for entry in servers {
        if entry.get("id").and_then(|v| v.as_str()) == Some(server_id) {
            return serde_json::from_value(entry.clone())
                .map_err(|e| format!("Failed to deserialize server '{}': {}", server_id, e));
        }
    }

    Err(format!(
        "MCP server '{}' not found in configuration",
        server_id
    ))
}

fn parse_meta_config(raw: &str) -> Result<InitializeResult, String> {
    serde_json::from_str(raw).map_err(|e| format!("Failed to parse meta_info config: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_MCP_CONFIG: &str = r#"{
        "mcp-servers": [
            {
                "id": "weather-server-001",
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
                        "action-id": "action-weather-001"
                    }
                ]
            },
            {
                "id": "calculator-server-001",
                "tools": [
                    {
                        "name": "add",
                        "description": "Add two numbers",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "a": { "type": "number" },
                                "b": { "type": "number" }
                            },
                            "required": ["a", "b"]
                        },
                        "action-id": "action-calc-001"
                    }
                ]
            }
        ]
    }"#;

    const TEST_META_CONFIG: &str = r#"{
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "test-server",
            "version": "1.0.0"
        }
    }"#;

    #[test]
    fn test_parse_server_config_finds_existing_server() {
        let config = parse_server_config(TEST_MCP_CONFIG, "weather-server-001").unwrap();
        assert_eq!(config.id, "weather-server-001");
        assert_eq!(config.tools.len(), 1);
        assert_eq!(config.tools[0].tool.name, "get_weather");
        assert_eq!(config.tools[0].action_id, "action-weather-001");
    }

    #[test]
    fn test_parse_server_config_finds_second_server() {
        let config = parse_server_config(TEST_MCP_CONFIG, "calculator-server-001").unwrap();
        assert_eq!(config.id, "calculator-server-001");
        assert_eq!(config.tools[0].tool.name, "add");
        assert_eq!(config.tools[0].action_id, "action-calc-001");
    }

    #[test]
    fn test_parse_server_config_not_found() {
        let result = parse_server_config(TEST_MCP_CONFIG, "nonexistent-server");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in configuration"));
    }

    #[test]
    fn test_parse_server_config_invalid_json() {
        let result = parse_server_config("not json", "any-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_server_config_missing_mcp_servers_key() {
        let result = parse_server_config(r#"{"other": "data"}"#, "any-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mcp-servers key missing"));
    }

    #[test]
    fn test_parse_server_config_empty_servers_array() {
        let result = parse_server_config(r#"{"mcp-servers": []}"#, "any-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in configuration"));
    }

    #[test]
    fn test_parse_meta_config_valid() {
        let meta = parse_meta_config(TEST_META_CONFIG).unwrap();
        assert_eq!(meta.protocol_version, "2024-11-05");
        assert_eq!(meta.server_info.name, "test-server");
        assert_eq!(meta.server_info.version, "1.0.0");
    }

    #[test]
    fn test_parse_meta_config_invalid_json() {
        let result = parse_meta_config("not json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[test]
    fn test_parse_meta_config_missing_required_fields() {
        let result = parse_meta_config(r#"{"protocolVersion": "2024-11-05"}"#);
        assert!(result.is_err());
    }
}
