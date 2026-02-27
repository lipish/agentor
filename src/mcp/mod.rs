
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::agent::tool::{Tool, ToolOutput};

/// MCP 客户端管理器
/// 负责管理与 MCP Server 的连接，并将其暴露为 Tool
pub struct McpManager {
    clients: Arc<Mutex<HashMap<String, Arc<dyn McpClientTrait>>>>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 连接到 Stdio MCP Server
    pub async fn connect_stdio(&self, _name: &str, _command: &str, _args: &[String]) -> Result<Vec<Box<dyn Tool>>> {
        // 由于依赖库版本冲突问题，暂时禁用实际连接代码
        // 待解决 mcp-core 和 mcp-client 的类型兼容性问题后恢复
        Ok(vec![])
    }

    /// 连接到 SSE MCP Server
    pub async fn connect_sse(&self, _name: &str, _url: &str) -> Result<Vec<Box<dyn Tool>>> {
        Ok(vec![])
    }
}

/// 包装 MCP 工具为 Agentor Tool
#[derive(Clone)]
pub struct McpTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
    client: Arc<dyn McpClientTrait>,
}

#[async_trait]
trait McpClientTrait: Send + Sync {
    async fn call_tool_wrapper(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value>;
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.input_schema.clone()
    }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolOutput> {
        let result = self.client.call_tool_wrapper(&self.name, args).await
            .context(format!("Failed to call MCP tool {}", self.name))?;

        Ok(ToolOutput {
            content: result.to_string(),
            metadata: None,
        })
    }

    fn clone_box(&self) -> Box<dyn Tool> {
        Box::new(self.clone())
    }
}

