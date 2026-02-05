use crate::types::{McpServerConfig, McpServersConfig};
use crate::wasi::config::store::get_all;

const WASI_CONFIG_KEY: &str = "mcp_servers";

pub fn load_server_config(server_id: &str) -> Result<McpServerConfig, String> {
    let servers_config = load_all_servers_config()?;

    servers_config
        .mcp_servers
        .into_iter()
        .find(|server| server.id == server_id)
        .ok_or_else(|| format!("MCP server '{}' not found in configuration", server_id))
}

fn load_all_servers_config() -> Result<McpServersConfig, String> {
    let config = get_all().map_err(|e| format!("Failed to get wasi config: {:?}", e))?;

    config
        .iter()
        .find(|(key, _)| key == WASI_CONFIG_KEY)
        .map(|(_, value)| {
            serde_json::from_str(value)
                .map_err(|e| format!("Failed to parse mcp_servers config: {}", e))
        })
        .unwrap_or_else(|| Err("mcp_servers key not found in runtime configuration".to_string()))
}

// Requires wasm emv for testing, keeping it on-hold for now

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_load_default_config() -> Result<(), String> {
//         let config = load_all_servers_config()?;
//         assert_eq!(config.mcp_servers.len(), 2);
//         assert_eq!(config.mcp_servers[0].id, "weather-server-001");
//         assert_eq!(config.mcp_servers[1].id, "calculator-server-001");
//         Ok(())
//     }

//     #[test]
//     fn test_load_server_config() {
//         let config = load_server_config("weather-server-001").unwrap();
//         assert_eq!(config.id, "weather-server-001");
//         assert_eq!(config.tools.len(), 1);
//         assert_eq!(config.tools[0].tool.name, "get_weather");
//     }

//     #[test]
//     fn test_load_nonexistent_server() {
//         let result = load_server_config("nonexistent-server");
//         assert!(result.is_err());
//     }
// }
