use async_trait::async_trait;
use tracing::{debug, info, warn};

use super::checkpoint::{Checkpoint, CheckpointStore};
use super::llm::{LlmConnector, LlmMessage, LlmRole};
use super::state::{AgentPhase, AgentState, MemoryEntry};
use crate::actor::actor::Actor;
use crate::actor::context::ActorContext;
use crate::actor::message::{ActorId, Envelope};

/// Agent 专用消息类型
#[derive(Debug, Clone)]
pub enum AgentMessage {
    /// 用户输入
    UserPrompt(String),
    /// 工具调用结果
    ToolResult { tool_name: String, output: String },
    /// LLM 流式响应片段
    StreamChunk(String),
    /// LLM 流式响应结束
    StreamEnd,
    /// 请求人类审批
    RequestApproval { description: String },
    /// 人类审批结果
    ApprovalResult {
        approved: bool,
        comment: Option<String>,
    },
    /// 创建子 Agent
    SpawnSubAgent {
        name: String,
        config: serde_json::Value,
    },
}

/// AgentActor — 基于 Actor 模型的 AI Agent 实现
///
/// 每个 AgentActor 是一个独立的 Actor，拥有：
/// - 私有状态（记忆）
/// - 消息处理循环
/// - 状态持久化（Checkpoint）
/// - 人机协作能力（AwaitHuman）
pub struct AgentActor {
    id: ActorId,
    state: AgentState,
    checkpoint_store: Option<CheckpointStore>,
    checkpoint_version: u64,
    checkpoint_interval: u64,
    llm: Option<LlmConnector>,
    system_prompt: Option<String>,
}

impl AgentActor {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: ActorId::new(&name),
            state: AgentState::default(),
            checkpoint_store: None,
            checkpoint_version: 0,
            checkpoint_interval: 10,
            llm: None,
            system_prompt: None,
        }
    }

    pub fn with_state(mut self, state: AgentState) -> Self {
        self.state = state;
        self
    }

    pub fn with_checkpoint_store(mut self, store: CheckpointStore) -> Self {
        self.checkpoint_store = Some(store);
        self
    }

    pub fn with_checkpoint_interval(mut self, interval: u64) -> Self {
        self.checkpoint_interval = interval;
        self
    }

    /// 配置 LLM 连接器
    pub fn with_llm(mut self, llm: LlmConnector) -> Self {
        self.llm = Some(llm);
        self
    }

    /// 配置系统提示词
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// 保存 checkpoint（如果配置了 store）
    async fn maybe_checkpoint(&mut self) -> anyhow::Result<()> {
        if self.state.message_count.is_multiple_of(self.checkpoint_interval) {
            if let Some(store) = &self.checkpoint_store {
                self.checkpoint_version += 1;
                let checkpoint = Checkpoint {
                    actor_id: self.id.clone(),
                    timestamp: chrono::Utc::now(),
                    state: self.state.clone(),
                    version: self.checkpoint_version,
                };
                store.save(&checkpoint).await?;
            }
        }
        Ok(())
    }

    /// 处理 AgentMessage
    async fn handle_agent_message(
        &mut self,
        msg: AgentMessage,
        _ctx: &mut ActorContext,
    ) -> anyhow::Result<()> {
        match msg {
            AgentMessage::UserPrompt(prompt) => {
                self.state.phase = AgentPhase::Thinking;
                self.state.touch();
                self.state.message_count += 1;

                self.state.push_short_term(MemoryEntry {
                    timestamp: chrono::Utc::now(),
                    role: "user".to_string(),
                    content: prompt.clone(),
                    metadata: None,
                });

                info!(agent = %self.id, "received user prompt, thinking...");

                // 调用 LLM（如果配置了 LlmConnector）
                let reply = if let Some(llm) = &self.llm {
                    let mut msgs: Vec<LlmMessage> = Vec::new();

                    // 系统提示词
                    if let Some(sp) = &self.system_prompt {
                        msgs.push(LlmMessage::system(sp.clone()));
                    }

                    // 历史记忆作为上下文
                    for mem in self.state.short_term.iter() {
                        let role = match mem.role.as_str() {
                            "user" => LlmRole::User,
                            "assistant" => LlmRole::Assistant,
                            "system" => LlmRole::System,
                            _ => LlmRole::User,
                        };
                        msgs.push(LlmMessage {
                            role,
                            content: mem.content.clone(),
                        });
                    }

                    match llm.chat(&msgs).await {
                        Ok(resp) => {
                            // 更新 token 统计
                            if let Some(t) = resp.total_tokens {
                                self.state.add_token_usage(t);
                            }
                            info!(
                                agent = %self.id,
                                tokens = ?resp.total_tokens,
                                "LLM response received"
                            );
                            resp.content
                        }
                        Err(e) => {
                            warn!(agent = %self.id, error = %e, "LLM call failed, falling back to echo");
                            format!("[echo] {}", prompt)
                        }
                    }
                } else {
                    // 无 LLM 配置，回显模式
                    format!("[echo] {}", prompt)
                };

                self.state.push_short_term(MemoryEntry {
                    timestamp: chrono::Utc::now(),
                    role: "assistant".to_string(),
                    content: reply,
                    metadata: None,
                });

                self.state.phase = AgentPhase::Idle;
                self.maybe_checkpoint().await?;
            }

            AgentMessage::ToolResult { tool_name, output } => {
                self.state.phase = AgentPhase::Thinking;
                self.state.touch();

                self.state.push_short_term(MemoryEntry {
                    timestamp: chrono::Utc::now(),
                    role: "tool".to_string(),
                    content: output,
                    metadata: Some(serde_json::json!({ "tool": tool_name })),
                });

                self.state.phase = AgentPhase::Idle;
            }

            AgentMessage::StreamChunk(chunk) => {
                self.state.phase = AgentPhase::Streaming;
                debug!(agent = %self.id, chunk_len = chunk.len(), "stream chunk");
            }

            AgentMessage::StreamEnd => {
                self.state.phase = AgentPhase::Idle;
                info!(agent = %self.id, "stream ended");
            }

            AgentMessage::RequestApproval { description } => {
                self.state.phase = AgentPhase::AwaitingHuman;
                info!(agent = %self.id, desc = %description, "awaiting human approval");
                // Actor 进入 AwaitingHuman 状态，后续消息中等待 ApprovalResult
            }

            AgentMessage::ApprovalResult { approved, comment } => {
                if self.state.phase == AgentPhase::AwaitingHuman {
                    info!(
                        agent = %self.id,
                        approved = approved,
                        comment = ?comment,
                        "human approval received"
                    );
                    self.state.phase = if approved {
                        AgentPhase::Executing
                    } else {
                        AgentPhase::Idle
                    };
                }
            }

            AgentMessage::SpawnSubAgent { name, config } => {
                info!(agent = %self.id, sub_agent = %name, "spawn sub-agent requested");
                // TODO: 通过 ActorSystem 动态 spawn 子 Agent
                // 目前记录请求
                self.state.push_short_term(MemoryEntry {
                    timestamp: chrono::Utc::now(),
                    role: "system".to_string(),
                    content: format!("spawn sub-agent: {}", name),
                    metadata: Some(config),
                });
            }
        }
        Ok(())
    }

    /// 获取当前状态的只读引用
    pub fn state(&self) -> &AgentState {
        &self.state
    }
}

#[async_trait]
impl Actor for AgentActor {
    async fn on_start(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        // 尝试从 checkpoint 恢复状态
        if let Some(store) = &self.checkpoint_store {
            if let Some(checkpoint) = store.load_latest(&self.id).await? {
                info!(
                    agent = %self.id,
                    version = checkpoint.version,
                    "restored from checkpoint"
                );
                self.state = checkpoint.state;
                self.checkpoint_version = checkpoint.version;
            }
        }
        info!(agent = %self.id, "agent actor started");
        Ok(())
    }

    async fn handle_message(
        &mut self,
        envelope: Envelope,
        ctx: &mut ActorContext,
    ) -> anyhow::Result<()> {
        // 尝试将 payload 转为 AgentMessage
        match envelope.downcast::<AgentMessage>() {
            Ok(msg) => self.handle_agent_message(msg, ctx).await,
            Err(envelope) => {
                // 尝试转为 String（简单文本消息）
                match envelope.downcast::<String>() {
                    Ok(text) => {
                        self.handle_agent_message(AgentMessage::UserPrompt(text), ctx)
                            .await
                    }
                    Err(_) => {
                        debug!(agent = %self.id, "received unknown message type");
                        Ok(())
                    }
                }
            }
        }
    }

    async fn on_stop(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        // 停止前保存最终 checkpoint
        if let Some(store) = &self.checkpoint_store {
            self.checkpoint_version += 1;
            let checkpoint = Checkpoint {
                actor_id: self.id.clone(),
                timestamp: chrono::Utc::now(),
                state: self.state.clone(),
                version: self.checkpoint_version,
            };
            store.save(&checkpoint).await?;
        }
        info!(agent = %self.id, "agent actor stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        &self.id.name
    }

    fn id(&self) -> &ActorId {
        &self.id
    }
}
