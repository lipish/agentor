# Agentor

**Agent-native Actor Runtime** — 一个专为 AI Agent 设计的 Actor 模型执行平台，用 Rust 实现。

## 核心理念

Actor 模型是 Agent 计算的天然底层。每个 Agent 就是一个 Actor，拥有私有状态（记忆）、通过异步消息通信、由监督者管理生命周期。Agentor 针对 AI Agent 的特点（高延迟 LLM 调用、不确定性崩溃、动态协作、状态持久化）做了专门设计。

## 架构

```
ActorSystem
├── Supervisor (监督树，容错策略)
│   ├── AgentActor (Planner)    ← 私有记忆 + Checkpoint
│   ├── AgentActor (Executor)   ← 工具调用 + 流式输出
│   └── AgentActor (Reviewer)   ← 人机协作 (AwaitHuman)
├── Environment (凭证 & 配置注入)
├── TokenBudget (资源预算 & 熔断)
└── TraceCollector (全链路观测 & 回放)
```

## 核心能力

**事务性消息信箱** — 两阶段 commit/nack，处理失败自动重试，超限进入 Dead Letter Queue

**故障分类监督** — Transient/Logic/Critical 三级故障分类，不同故障类型对应不同恢复策略

**流式拦截** — StreamInterceptor 实时检查流内容，检测到有害内容立即终止流

**Agent 休眠/唤醒** — 空闲 Agent 可休眠释放资源，收到消息时自动唤醒并重放暂存消息

**异步双向流通信** — Agent 间流式消息传递，支持 LLM streaming 中途中断

**状态机与 Checkpointing** — Agent 状态原子化持久化，崩溃后精准恢复

**人机协作原语** — AwaitHuman 挂起/唤醒机制，Agent 可暂停等待人类审批

**资源预算与熔断** — Token/成本预算管理，超限自动熔断

## 快速开始

```rust
use agentor::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建环境
    let env = Environment::new();
    env.set_secret("OPENAI_API_KEY", "sk-...");

    // 创建 ActorSystem
    let mut system = ActorSystem::with_environment("my-app", env);

    // 创建带 LLM 的 Agent
    let llm = LlmConnector::openai("sk-...", "gpt-4")?;
    let agent = AgentActor::new("planner")
        .with_llm(llm)
        .with_system_prompt("You are a helpful travel planner.");
    let agent_ref = system.spawn_default(Box::new(agent));

    // 发送消息（自动调用 LLM）
    agent_ref.tell(AgentMessage::UserPrompt("Plan a trip to Tokyo".into())).await?;

    // 关闭
    system.shutdown().await;
    Ok(())
}
```

## 模块结构

```
src/
├── actor/          # 核心 Actor 层 (Actor, ActorSystem, Mailbox, FailureKind, Hibernation)
├── agent/          # Agent 专用层 (AgentActor, AgentState, Checkpoint, LlmConnector)
├── supervisor/     # 监督树 (Supervisor, 故障分类策略)
├── stream/         # 流式通信 (StreamProducer/Consumer, StreamInterceptor)
├── environment/    # 环境 & 凭证 (Environment, Secret 注入)
├── budget/         # 资源预算 (TokenBudget, 熔断)
├── observe/        # 观测 & 回放 (TraceCollector → xtrace.sh)
└── prelude.rs      # 常用类型 re-export
```

## 技术栈

- **异步运行时**: tokio
- **LLM 连接**: [llm-connector](https://github.com/lipish/llm-connector) — 12+ Provider（OpenAI, Anthropic, DeepSeek, Ollama...），统一流式接口
- **可观测**: [xtrace](https://xtrace.sh) — 全链路追踪 + 指标上报，OpenTelemetry 兼容
- **追踪**: tracing + xtrace XtraceLayer 自动采集
- **序列化**: serde + bincode (状态持久化)
- **并发**: dashmap + parking_lot
- **无外部 Actor 框架依赖**，从 tokio task + mpsc channel 自建，保持对 Agent 场景的完全控制

## 构建 & 测试

```bash
cargo build
cargo test
cargo run
```

## License

MIT