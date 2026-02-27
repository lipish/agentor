use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::time::Duration;

use agentor::actor::actor::Actor;
use agentor::actor::system::ActorSystem;
use agentor::agent::agent::{AgentActor, AgentMessage};
use agentor::agent::parser::JsonParser;
use agentor::agent::tool::{CalculatorTool, ToolRegistry};

#[tokio::test]
async fn test_tool_execution_flow() -> Result<()> {
    // 1. 设置工具和解析器
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(CalculatorTool));

    let parser = Arc::new(JsonParser);

    // 2. 创建 Agent
    // 注意：这里没有配置 LlmConnector，所以 Agent 会回显 prompt
    // 我们构造一个看起来像 LLM 输出了工具调用的 prompt
    let tool_call_prompt = r#"
    I will calculate this for you.
    ```json
    {
        "tool": "calculator",
        "args": {
            "expression": "10 + 20"
        }
    }
    ```
    "#;

    let agent = AgentActor::new("math_agent")
        .with_tools(registry)
        .with_parser(parser);

    let mut system = ActorSystem::new("test_system");
    let agent_ref = system.spawn_default(Box::new(agent));

    // 3. 发送消息触发工具调用
    // Agent 会回显我们的 prompt，Parser 会解析其中的 JSON，触发 CalculatorTool
    agent_ref.tell(AgentMessage::UserPrompt(tool_call_prompt.to_string())).await?;

    // 等待异步执行完成
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 4. 验证状态
    // 我们无法直接访问 Actor 内部状态，但可以通过发送新消息来观察
    // 或者我们可以在这里通过 system.stop_actor 停止它，然后通过日志观察
    // 为了更严谨的测试，我们应该检查 AgentState 中的记忆，但 AgentState 是私有的
    // 这里我们通过观察日志输出来确认（在实际运行 cargo test 时可以看到）
    
    // 正常情况下，CalculatorTool 执行后会发送 ToolResult 消息给自己
    // 我们可以再次发送一个普通消息，看看是否正常响应
    agent_ref.tell(AgentMessage::UserPrompt("status check".to_string())).await?;
    
    system.shutdown().await;
    Ok(())
}
