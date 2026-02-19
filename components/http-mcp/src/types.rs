use rust_mcp_schema::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct McpServersConfig {
    #[serde(rename = "mcp-servers")]
    pub mcp_servers: Vec<McpServerConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolWithAction {
    #[serde(flatten)]
    pub tool: Tool,
    #[serde(rename = "action-id")]
    pub action_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub tools: Vec<ToolWithAction>,
}
