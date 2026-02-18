use agentor::prelude::*;
use tracing_subscriber::EnvFilter;

/// Agentor — Agent-native Actor Runtime
///
/// 演示：创建 ActorSystem，spawn 一个 AgentActor，发送消息，然后关闭
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    println!("=== Agentor: Agent-native Actor Runtime ===\n");

    // 1. 创建环境
    let env = Environment::new();
    env.set_config("model", "gpt-4");
    env.set_secret("OPENAI_API_KEY", "sk-demo-key");

    // 2. 创建 ActorSystem
    let mut system = ActorSystem::with_environment("agentor", env);
    println!("[system] ActorSystem '{}' created", system.name());

    // 3. 创建并 spawn AgentActor
    let agent = AgentActor::new("planner-agent");
    let agent_ref = system.spawn_default(Box::new(agent));
    println!("[system] Agent spawned: {}", agent_ref.id());

    // 4. 发送消息
    agent_ref
        .tell(AgentMessage::UserPrompt(
            "Help me plan a trip to Tokyo".to_string(),
        ))
        .await?;
    println!("[main] sent UserPrompt to agent");

    agent_ref
        .tell(AgentMessage::ToolResult {
            tool_name: "weather_api".to_string(),
            output: "Tokyo: 22°C, sunny".to_string(),
        })
        .await?;
    println!("[main] sent ToolResult to agent");

    // 5. 等待消息处理完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 6. 关闭系统
    system.shutdown().await;
    println!("\n[system] shutdown complete");

    Ok(())
}
