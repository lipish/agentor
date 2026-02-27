# Agentor 详细设计文档

## 1. 项目定位

Agentor 是一个 Agent-native Actor Runtime，用 Rust 从零构建。核心思想：**Actor 模型是 AI Agent 计算的天然底层**。每个 Agent 就是一个 Actor，拥有私有状态（记忆）、通过异步消息通信、由监督者管理生命周期。

与通用 Actor 框架不同，Agentor 针对 AI Agent 的特殊需求做了专门设计：高延迟 LLM 调用、不确定性崩溃、动态多 Agent 协作、状态持久化与恢复、人机协作。

## 2. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                      ActorSystem                        │
│  ┌───────────────────────────────────────────────────┐  │
│  │                 Supervisor 层                      │  │
│  │  ┌─────────────┐ ┌─────────────┐ ┌────────────┐  │  │
│  │  │ AgentActor  │ │ AgentActor  │ │ AgentActor │  │  │
│  │  │ (Planner)   │ │ (Executor)  │ │ (Reviewer) │  │  │
│  │  │             │ │             │ │            │  │  │
│  │  │ ┌─────────┐ │ │ ┌─────────┐ │ │ ┌────────┐ │  │  │
│  │  │ │AgentState│ │ │ │AgentState│ │ │ │AgentSt.│ │  │  │
│  │  │ │ 短期记忆 │ │ │ │ 短期记忆 │ │ │ │ 短期记忆│ │  │  │
│  │  │ │ 长期记忆 │ │ │ │ 长期记忆 │ │ │ │ 长期记忆│ │  │  │
│  │  │ │ 状态机   │ │ │ │ 状态机   │ │ │ │ 状态机  │ │  │  │
│  │  │ └─────────┘ │ │ └─────────┘ │ │ └────────┘ │  │  │
│  │  └──────┬──────┘ └──────┬──────┘ └─────┬──────┘  │  │
│  │         │               │              │          │  │
│  │         └───── Mailbox (mpsc) ─────────┘          │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌──────────┐ ┌──────────┐ ┌───────────┐ ┌──────────┐  │
│  │Environmt.│ │TokenBudgt│ │TraceColl. │ │  Stream  │  │
│  │Config/Sec│ │预算+熔断  │ │→ xtrace.sh│ │Producer/ │  │
│  └──────────┘ └──────────┘ └───────────┘ │Consumer  │  │
│                                          └──────────┘  │
│  ┌─────────────────────────────────────────────────┐    │
│  │              LlmConnector (llm-connector)       │    │
│  │  OpenAI │ Anthropic │ DeepSeek │ Ollama │ ...   │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

## 3. 模块设计

### 3.1 actor/ — 核心 Actor 层

这是整个运行时的基础，不依赖任何外部 Actor 框架，从 tokio task + mpsc channel 自建。

#### 3.1.1 Actor trait

```rust
#[async_trait]
pub trait Actor: Send + 'static {
    async fn on_start(&mut self, ctx: &mut ActorContext) -> Result<()>;
    async fn handle_message(&mut self, envelope: Envelope, ctx: &mut ActorContext) -> Result<()>;
    async fn on_stop(&mut self, ctx: &mut ActorContext) -> Result<()>;
    async fn on_restart(&mut self, error: &Error, ctx: &mut ActorContext) -> Result<()>;
    fn name(&self) -> &str;
    fn id(&self) -> &ActorId;
}
```

生命周期：`on_start` → `handle_message` 循环 → `on_stop`。如果 `handle_message` 返回 Err，监督者根据策略决定重启（调用 `on_restart`）或停止。

#### 3.1.2 Envelope（消息信封）

所有消息通过 `Envelope` 传递，包含：
- **id** — 消息唯一 ID (UUID)
- **trace_id** — 链路追踪 ID，贯穿整个请求链
- **timestamp** — 发送时间
- **sender** — 发送者 ActorId（可选）
- **payload** — `Box<dyn Any + Send>`，类型擦除的消息体

接收方通过 `envelope.downcast::<T>()` 恢复具体类型。这种设计让 Actor 可以接收任意类型的消息，同时保持类型安全。

#### 3.1.3 Mailbox（信箱）

基于 `tokio::sync::mpsc::channel` 的 bounded channel 实现。关键设计：
- **背压机制** — 信箱满时 `send()` 异步等待，防止消息洪泛
- **try_send** — 非阻塞发送，信箱满时立即返回错误
- **默认容量 256** — 可在 spawn 时自定义

#### 3.1.4 ActorRef（引用句柄）

Actor 的外部引用，持有 `MailboxSender` 的 clone。提供：
- `tell(payload)` — 异步发送消息（背压）
- `tell_from(payload, sender)` — 带发送者信息
- `try_tell(payload)` — 非阻塞发送
- `is_stopped()` — 检查 Actor 是否已停止

设计目标是**位置透明**：无论 Actor 在本地还是远程，使用方式一致。

#### 3.1.5 ActorContext（运行时上下文）

在 `handle_message` 中提供给 Actor 使用：
- `self_id()` / `self_ref()` — 自身标识和引用
- `children()` — 子 Actor 注册表
- `environment()` — 全局环境配置

#### 3.1.6 ActorSystem（系统管理器）

顶层管理器，职责：
- `spawn(actor, capacity)` — 创建 Actor：分配 Mailbox、注册到 DashMap、启动 tokio task 运行消息循环
- `find(id)` / `find_by_name(name)` — 查找 Actor
- `stop_actor(id)` — 发送 Stop 信号
- `shutdown()` — 向所有 Actor 发送 Stop，等待全部退出

内部使用 `DashMap<ActorId, ActorEntry>` 作为 Actor 注册表，支持并发读写。每个 Actor 运行在独立的 tokio task 中。

### 3.2 agent/ — Agent 专用层

在 Actor 基础上，为 AI Agent 场景添加专用能力。

#### 3.2.1 AgentActor

核心 Agent 实现，继承 Actor trait。额外拥有：
- **AgentState** — 私有记忆（短期 + 长期）
- **LlmConnector** — LLM 调用能力（可选）
- **CheckpointStore** — 状态持久化（可选）
- **system_prompt** — 系统提示词

消息处理流程（以 `UserPrompt` 为例）：

```
UserPrompt 到达
  → handle_user_prompt()
    → phase = Thinking
    → 写入短期记忆
    → 构造 LLM 请求（system_prompt + 历史记忆）
    → 调用 LlmConnector.chat()
    → 写入 assistant 回复到短期记忆
    → 更新 token 统计
    → phase = Idle
    → maybe_checkpoint()
```

代码实现经过重构，将不同类型消息的处理逻辑拆分为独立的私有方法（`handle_user_prompt`, `handle_tool_result` 等），提高可维护性。

如果没有配置 LlmConnector，自动 fallback 到 echo 模式（方便测试）。

#### 3.2.2 AgentMessage

Agent 专用消息类型枚举：

| 消息 | 用途 |
|------|------|
| `UserPrompt(String)` | 用户输入，触发 LLM 调用 |
| `ToolResult { tool_name, output }` | 工具执行结果回填 |
| `StreamChunk(String)` | LLM 流式响应片段 |
| `StreamEnd` | 流式响应结束 |
| `RequestApproval { description }` | 请求人类审批 |
| `ApprovalResult { approved, comment }` | 人类审批结果 |
| `SpawnSubAgent { name, config }` | 动态创建子 Agent |

#### 3.2.3 AgentState（记忆系统）

```rust
pub struct AgentState {
    pub short_term: VecDeque<MemoryEntry>,   // FIFO，默认容量 50
    pub short_term_capacity: usize,
    pub long_term: Vec<MemoryEntry>,          // 持久化 KV
    pub phase: AgentPhase,                    // 状态机
    pub last_active: DateTime<Utc>,
    pub message_count: u64,
    pub token_usage: u64,
}
```

短期记忆自动淘汰最旧条目，保证内存可控。每条 `MemoryEntry` 包含 timestamp、role、content、metadata。

#### 3.2.4 AgentPhase（状态机）

```
Idle → Thinking → Idle          (正常对话)
Idle → Thinking → Executing     (工具调用)
Idle → AwaitingHuman → Idle     (人类拒绝)
Idle → AwaitingHuman → Executing (人类批准)
Idle → Streaming → Idle          (流式输出)
任意 → Failed                    (异常)
```

`AwaitingHuman` 是人机协作的关键状态：Agent 发出 `RequestApproval` 后进入此状态，暂停处理，直到收到 `ApprovalResult`。

#### 3.2.5 Checkpoint（状态持久化）

基于文件系统的 JSON 序列化。目录结构：

```
checkpoints/
└── {actor_uuid}/
    ├── checkpoint_000001.json
    ├── checkpoint_000002.json
    └── checkpoint_000010.json
```

- `save()` — 异步写入 JSON 文件
- `load_latest()` — 扫描目录找最新版本
- `load_version()` — 加载指定版本

Agent 启动时自动从最新 checkpoint 恢复状态，停止时保存最终 checkpoint。运行中按 `checkpoint_interval`（默认每 10 条消息）自动保存。

#### 3.2.6 LlmConnector（LLM 连接）

封装 [llm-connector](https://github.com/lipish/llm-connector) crate，提供：
- `chat(messages)` — 非流式调用，返回完整内容 + token 统计
- `chat_stream(messages, callback)` — 流式调用，逐 chunk 回调
- 快捷构造：`openai()`, `anthropic()`, `deepseek()`, `ollama()`, `builder()`

支持 12+ Provider，统一的 `LlmMessage` / `LlmResponse` 接口。AgentActor 收到 `UserPrompt` 时自动将短期记忆 + system_prompt 组装为 LLM 请求上下文。

### 3.3 supervisor/ — 监督树

借鉴 Erlang/OTP 监督树模型。

#### 3.3.1 SupervisionStrategy

| 策略 | 行为 |
|------|------|
| `OneForOne { max_retries, within_secs }` | 只重启失败的子 Actor |
| `AllForOne { max_retries, within_secs }` | 一个失败，全部重启 |
| `Stop` | 直接停止失败的子 Actor |
| `Escalate` | 上报给父监督者 |

#### 3.3.2 Supervisor

Supervisor 本身也是一个 Actor，通过 `SupervisorMessage` 接收子 Actor 的故障报告。根据策略和重试次数做出决策（Restart / Stop / Escalate / Resume）。

### 3.4 stream/ — 流式通信

#### 3.4.1 设计动机

LLM 的流式输出需要 Actor 之间建立持续的数据流，而不是单条消息。同时监督者需要能够中途中断流（比如检测到有害内容）。

#### 3.4.2 双向流

```rust
let (producer, consumer) = create_stream::<String>(buffer_size);
```

- **StreamProducer** — 生产端，`send(data)` / `finish()` / `error(msg)` / `is_cancelled()`
- **StreamConsumer** — 消费端，`next()` / `cancel()`

取消机制通过独立的 mpsc channel 实现，消费者调用 `cancel()` 后，生产者通过 `is_cancelled()` 检测到并停止发送。

#### 3.4.3 StreamEvent

```rust
enum StreamEvent<T> {
    Data(T),    // 数据片段
    End,        // 正常结束
    Error(String), // 错误
    Cancel,     // 取消
}
```

### 3.5 environment/ — 环境与凭证

`Environment` 是一个线程安全的 KV 容器（`parking_lot::RwLock<HashMap>`），分为两个命名空间：
- **config** — 普通配置项（model name、temperature 等）
- **secrets** — 敏感凭证（API keys），生产环境应从 Vault / 环境变量加载

通过 `ActorContext.environment()` 在任何 Actor 中访问。支持 `load_from_env(prefix)` 批量加载环境变量。

### 3.6 budget/ — 资源预算与熔断

#### 3.6.1 TokenBudget

基于 `AtomicU64` + `AtomicBool` 的无锁实现，线程安全，可在多个 Actor 间共享（`Arc`）。

核心逻辑：
```
try_consume(tokens):
  if tripped → return false
  used += tokens (atomic)
  if used > limit → tripped = true, return false
  return true
```

熔断后所有 `try_consume` 立即返回 false，防止 Agent 死循环刷爆 API。管理员可通过 `reset()` 解除熔断，或 `set_limit()` 调整上限。

### 3.7 observe/ — 可观测与链路追踪

#### 3.7.1 本地追踪

`TraceCollector` 内部维护一个环形缓冲区（`VecDeque`，默认容量 10000），记录所有 `TraceEvent`。支持：
- `query_by_trace(trace_id)` — 按链路 ID 查询（回放一次完整请求）
- `query_by_actor(actor_id, limit)` — 按 Actor 查询最近事件

#### 3.7.2 xtrace 远程上报

集成 [xtrace](https://xtrace.sh) 的 Rust SDK（`xtrace-client` crate）。

- `with_xtrace(capacity, endpoint, token)` — 创建时连接 xtrace 服务
- `record()` 时自动异步上报（`tokio::spawn`，不阻塞消息循环）
- `report_token_usage()` — 推送 prompt_tokens / completion_tokens / total_tokens 指标

上报格式遵循 xtrace 的 `BatchIngestRequest`，包含 `TraceIngest`（链路）和 `ObservationIngest`（观测点）。LLM 调用映射为 `GENERATION` 类型，工具调用映射为 `SPAN` 类型。

## 4. 数据流

### 4.1 用户消息处理流程

```
用户 → ActorRef.tell(UserPrompt("..."))
     → Envelope 入 Mailbox
     → ActorSystem 消息循环取出
     → AgentActor.handle_message()
       → downcast 为 AgentMessage::UserPrompt
       → 写入短期记忆
       → 组装 LLM 上下文 (system_prompt + 历史记忆)
       → LlmConnector.chat() → llm-connector → LLM API
       → 写入 assistant 回复
       → 更新 token_usage
       → maybe_checkpoint()
       → TraceCollector.record() → xtrace (async)
```

### 4.2 监督故障处理流程

```
AgentActor.handle_message() 返回 Err
  → ActorSystem 记录错误日志
  → (未来) 通知 Supervisor
  → Supervisor.decide(child_id, retry_count)
    → OneForOne: retry_count < max → Restart
    → OneForOne: retry_count >= max → Stop
    → AllForOne: 重启所有子 Actor
    → Escalate: 上报父监督者
```

## 5. 并发模型

每个 Actor 运行在独立的 tokio task 中，通过 bounded mpsc channel 通信。这意味着：

- **无共享可变状态** — Actor 的状态完全私有，只通过消息修改
- **背压传播** — 信箱满时发送方自动等待，防止内存溢出
- **故障隔离** — 一个 Actor panic 不影响其他 Actor（tokio task 级别隔离）
- **顺序处理** — 单个 Actor 内消息严格顺序处理，无需加锁

全局共享的数据结构使用并发安全的容器：
- `DashMap` — Actor 注册表
- `parking_lot::RwLock` — Environment、TraceBuffer
- `AtomicU64` / `AtomicBool` — TokenBudget

## 6. 依赖关系

```
agentor
├── tokio (异步运行时)
├── tracing + tracing-subscriber (结构化日志)
├── serde + serde_json + bincode (序列化)
├── uuid + chrono (标识 + 时间)
├── async-trait (异步 trait)
├── dashmap + parking_lot (并发容器)
├── anyhow + thiserror (错误处理)
├── llm-connector 0.6.0 (LLM 协议抽象)
├── xtrace-client 0.0.12 (可观测上报)
└── futures-util (流式处理)
```

## 7. 演进路线

### v0.1（已完成）— 基础框架

Actor 运行时、AgentActor + LLM 调用、监督树骨架、流式通信、Checkpoint、环境凭证、资源熔断、xtrace 可观测集成。9 个测试通过。

### v0.2（已完成）— 可靠性增强

从 AAS 设计方案（`docs/agentor/`）中吸收四个特性，18 个测试全部通过。

#### 7.1 Transactional Mailbox（事务性信箱）

**问题**：当前 Mailbox 基于标准 mpsc channel，消息取出即消费。如果 Agent 在处理消息过程中崩溃（LLM 超时、工具调用失败），消息丢失，无法重试。

**方案**：引入两阶段消息处理机制。

```
消息从 channel 取出 → 标记为 in-flight（暂存）
  → handle_message 成功 → commit（确认消费）
  → handle_message 失败 → nack（消息回到队列头部，等待重试）
```

实现要点：
- `Mailbox` 内部增加 `pending: Option<Envelope>` 字段，保存当前正在处理的消息
- 消息循环改为：`recv()` → `handle_message()` → 成功则 `commit()`，失败则 `nack()` 将消息放回
- `nack` 时附带重试计数，超过阈值则转入 Dead Letter Queue（DLQ）
- DLQ 是一个独立的 `VecDeque<(Envelope, Error)>`，可供人工检查

这个改动对 Actor trait 接口透明，只影响 Mailbox 和 ActorSystem 的消息循环。

#### 7.2 Failure Classification（故障分类）

**问题**：当前监督策略只看重试次数，不区分故障类型。但 Agent 场景中，网络超时和 Agent 产生幻觉是完全不同的故障，需要不同的恢复策略。

**方案**：引入三级故障分类。

| 级别 | 类型 | 典型场景 | 默认策略 |
|------|------|----------|----------|
| `Transient` | 临时性故障 | LLM API 超时、网络抖动、Rate Limit | 指数退避重试（backoff） |
| `Logic` | 逻辑错误 | 工具输入格式错误、JSON 解析失败 | 修正 prompt 后重试（reflect-and-retry） |
| `Critical` | 严重故障 | 预算耗尽、安全违规、持续幻觉 | 立即停止 + 告警 |

实现要点：
- 新增 `FailureKind` 枚举（Transient / Logic / Critical）
- `handle_message` 返回的 `anyhow::Error` 可通过 downcast 提取 `FailureKind`
- Supervisor 的 `decide()` 方法签名扩展为 `decide(child_id, failure_kind, retry_count)`
- 不同 FailureKind 对应不同的重试策略和上限
- Transient 故障自动 backoff 重试，Logic 故障可触发 Agent 自我反思（在 prompt 中注入错误信息重新调用 LLM），Critical 故障直接 shutdown

#### 7.3 Stream Interception（流拦截）

**问题**：当前 StreamProducer/Consumer 是点对点的，监督者无法实时检查流内容。如果 Agent 在流式输出中产生有害内容，只能等流结束后才能发现。

**方案**：在 Producer 和 Consumer 之间插入 Interceptor 层。

```
Producer → [Interceptor] → Consumer
              ↓
         检测到问题 → 发送 Cancel 给 Producer
                     → 发送 Error 给 Consumer
```

实现要点：
- 新增 `StreamInterceptor` trait：

```rust
#[async_trait]
pub trait StreamInterceptor<T>: Send + 'static {
    /// 检查每个 chunk，返回 Pass（放行）或 Block（拦截）
    async fn inspect(&mut self, chunk: &T) -> InterceptResult;
}

pub enum InterceptResult {
    Pass,
    Block { reason: String },
}
```

- `create_stream` 扩展为 `create_intercepted_stream(buffer, interceptor)`，内部启动一个 tokio task 做转发 + 检查
- 内置 `SafetyInterceptor`（关键词/正则匹配）作为示例实现
- Interceptor 可链式组合（多个 Interceptor 串联）

#### 7.4 Hibernation（休眠）

**问题**：处于 `AwaitingHuman` 状态的 Agent 可能等待数小时甚至数天，但始终占用一个 tokio task 和内存。

**方案**：空闲 Agent 可序列化到磁盘，释放所有运行时资源。

```
Agent 进入 AwaitingHuman
  → 超过 idle_timeout（可配置）
  → 自动 Hibernate：序列化完整状态到 CheckpointStore，停止 tokio task
  → 收到 ApprovalResult 消息时
  → 自动 Thaw：从 Checkpoint 恢复状态，重新 spawn tokio task
```

实现要点：
- `AgentActor` 新增 `idle_timeout: Duration` 配置
- `ActorSystem` 新增 `hibernate(id)` / `thaw(id)` 方法
- Hibernate 时保存完整 Checkpoint + 将待处理消息持久化
- 优化：Hibernate 过程使用轮询 + 超时机制等待 Actor 停止，避免竞态条件
- Thaw 时从 Checkpoint 恢复 + 重放持久化的消息
- 对外部调用者透明：`ActorRef.tell()` 在 Agent 休眠时自动触发 Thaw

### v0.3（中期）— 编排与易用性

#### 7.5 DSL 编排层

YAML 声明式定义 Agent 拓扑，降低使用门槛。核心能力：
- 声明 Actor 模板（model、role、tools）
- 定义数据流拓扑（线性管道、扇出并行、流拦截）
- 配置预算策略和人机协作规则
- 启动时静态校验（检测环路、不可达 Agent）

```yaml
version: "1.0"
name: "code-reviewer"
actors:
  - id: coder
    model: gpt-4o
    role: "Expert Rust Developer"
  - id: critic
    model: claude-3-5-sonnet
    role: "Security Auditor"
topology:
  - from: coder
    to: critic
    type: stream
    interceptors: [safety_scanner]
policies:
  budget:
    max_tokens: 100000
  approval:
    - actor: critic
      action: require_human
```

DSL 解析后生成 ActorSystem 的 spawn 调用序列，不引入新的运行时概念。

#### 7.6 AwaitHuman 超时策略

扩展当前的 `AwaitingHuman` 状态，支持超时后的自动处理：

| 策略 | 行为 |
|------|------|
| `DefaultApprove` | 超时后自动批准 |
| `DefaultReject` | 超时后自动拒绝 |
| `Escalate` | 超时后上报给父监督者 |
| `Hibernate` | 超时后休眠，等待人类唤醒 |

### v0.4（远期）— 生产化

- **Supervisor 实际重启** — 与 ActorSystem 联动实现真正的 Actor 重启
- **持久化后端扩展** — Checkpoint 支持 SQLite / Redis / S3
- **XtraceLayer 自动采集** — 通过 tracing Layer 自动上报 span 和 metric
- **Pub/Sub 主题订阅** — Actor 之间的广播通信
- **WASM 沙箱工具执行** — 工具调用在 WASM 沙箱中运行
- **分布式 Actor** — ActorRef 支持远程寻址，跨节点消息路由
- **Python/JS SDK** — 通过 gRPC/HTTP API 暴露，供其他语言调用

## 8. 设计决策记录

### 8.1 为什么不用 Event Sourcing

AAS 方案提出存储所有消息作为不可变序列，支持 Deterministic Replay。我们选择不采用，原因：LLM 调用本身是不确定的（同样的 prompt 不同时间返回不同结果），真正的确定性回放意义有限。Checkpoint + Trace 日志已经能满足状态恢复和问题排查的需求，复杂度远低于 Event Sourcing。

### 8.2 为什么用 bounded channel 而非 unbounded

AAS 的 streaming_design.md 建议用 `unbounded_channel` 做流式传输以降低延迟。我们坚持使用 bounded channel，原因：一个失控的 LLM 流可以无限产生 token，unbounded channel 会导致内存无限增长。bounded channel 的背压机制是生产环境的安全底线。对于延迟敏感场景，可以适当增大 buffer_size。

### 8.3 为什么记忆系统不内置向量存储

AAS 方案提出每个 Actor 配独立向量索引做 RAG。我们认为向量检索应该作为外部工具（通过 ToolResult 消息回填），而不是运行时内置能力。原因：向量存储的选型和配置差异极大（Qdrant / Milvus / pgvector），内置会增加运行时的复杂度和依赖。Agent 通过工具调用访问向量存储，保持运行时的精简。
