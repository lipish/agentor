use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

/// 工具执行结果
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub metadata: Option<Value>,
}

/// 工具定义 Trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;
    
    /// 工具描述
    fn description(&self) -> &str;
    
    /// 执行工具
    async fn execute(&self, args: Value) -> Result<ToolOutput>;

    /// 参数 schema (JSON Schema)
    fn parameters(&self) -> Value;

    /// 克隆自身
    fn clone_box(&self) -> Box<dyn Tool>;
}

impl Clone for Box<dyn Tool> {
    fn clone(&self) -> Box<dyn Tool> {
        self.clone_box()
    }
}

/// 工具注册表
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(HashMap::new()),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        // 由于 Arc 是不可变的，我们需要重新构建 HashMap
        // 注意：这在运行时动态添加工具时会有性能开销，但通常工具是在初始化时注册的
        let mut tools = (*self.tools).clone();
        tools.insert(tool.name().to_string(), tool);
        self.tools = Arc::new(tools);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn list_tools(&self) -> Vec<&Box<dyn Tool>> {
        self.tools.values().collect()
    }
}

// 示例工具实现

#[derive(Clone)]
pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic operations"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "The mathematical expression to evaluate (e.g., '2 + 2')"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolOutput> {
        let expression = args["expression"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'expression' argument"))?;
        
        // 简单模拟计算，实际应用可以使用更安全的表达式求值库
        // 这里仅作演示
        let result = if expression.contains('+') {
             let parts: Vec<&str> = expression.split('+').collect();
             let a: f64 = parts[0].trim().parse()?;
             let b: f64 = parts[1].trim().parse()?;
             a + b
        } else {
             0.0
        };

        Ok(ToolOutput {
            content: result.to_string(),
            metadata: None,
        })
    }

    fn clone_box(&self) -> Box<dyn Tool> {
        Box::new(self.clone())
    }
}
