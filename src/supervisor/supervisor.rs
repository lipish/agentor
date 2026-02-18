use std::collections::HashMap;

use async_trait::async_trait;
use tracing::{error, info, warn};

use super::strategy::{SupervisionDecision, SupervisionStrategy};
use crate::actor::actor::Actor;
use crate::actor::context::ActorContext;
use crate::actor::failure::FailureKind;
use crate::actor::message::{ActorId, Envelope};

/// 监督者消息
#[derive(Debug, Clone)]
pub enum SupervisorMessage {
    /// 子 Actor 报告错误（带故障分类）
    ChildFailed {
        child_id: ActorId,
        error: String,
        failure_kind: Option<FailureKind>,
        retry_count: u32,
    },
    /// 子 Actor 已停止
    ChildStopped { child_id: ActorId },
    /// 查询所有子 Actor 状态
    QueryChildren,
}

/// Supervisor — 监督者 Actor
///
/// 管理一组子 Actor 的生命周期，根据策略处理子 Actor 的失败
pub struct Supervisor {
    id: ActorId,
    name: String,
    strategy: SupervisionStrategy,
    retry_counts: HashMap<ActorId, u32>,
}

impl Supervisor {
    pub fn new(name: impl Into<String>, strategy: SupervisionStrategy) -> Self {
        let name = name.into();
        Self {
            id: ActorId::new(&name),
            name: name.clone(),
            strategy,
            retry_counts: HashMap::new(),
        }
    }

    /// 根据故障分类和策略做出监督决策
    ///
    /// 优先级：FailureKind > SupervisionStrategy
    /// - Critical 故障直接 Stop，不看策略
    /// - Transient 故障在重试上限内自动 Restart
    /// - Logic 故障尝试 Resume（让 Agent 自我反思），超限则 Stop
    /// - 无分类信息时 fallback 到原有策略逻辑
    fn decide(
        &self,
        _child_id: &ActorId,
        failure_kind: Option<&FailureKind>,
        retry_count: u32,
    ) -> SupervisionDecision {
        // 如果有故障分类，优先按分类决策
        if let Some(kind) = failure_kind {
            return match kind {
                FailureKind::Critical { .. } => SupervisionDecision::Stop,
                FailureKind::Transient { .. } => {
                    let max = self.max_retries();
                    if retry_count >= max {
                        SupervisionDecision::Escalate
                    } else {
                        SupervisionDecision::Restart
                    }
                }
                FailureKind::Logic { .. } => {
                    let max = self.max_retries();
                    if retry_count >= max {
                        SupervisionDecision::Stop
                    } else {
                        SupervisionDecision::Resume
                    }
                }
            };
        }

        // 无分类信息，fallback 到原有策略
        match &self.strategy {
            SupervisionStrategy::OneForOne {
                max_retries,
                within_secs: _,
            } => {
                if retry_count >= *max_retries {
                    SupervisionDecision::Stop
                } else {
                    SupervisionDecision::Restart
                }
            }
            SupervisionStrategy::AllForOne {
                max_retries,
                within_secs: _,
            } => {
                if retry_count >= *max_retries {
                    SupervisionDecision::Escalate
                } else {
                    SupervisionDecision::Restart
                }
            }
            SupervisionStrategy::Stop => SupervisionDecision::Stop,
            SupervisionStrategy::Escalate => SupervisionDecision::Escalate,
        }
    }

    fn max_retries(&self) -> u32 {
        match &self.strategy {
            SupervisionStrategy::OneForOne { max_retries, .. }
            | SupervisionStrategy::AllForOne { max_retries, .. } => *max_retries,
            SupervisionStrategy::Stop => 0,
            SupervisionStrategy::Escalate => 0,
        }
    }
}

#[async_trait]
impl Actor for Supervisor {
    async fn on_start(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        info!(supervisor = %self.id, "supervisor started");
        Ok(())
    }

    async fn handle_message(
        &mut self,
        envelope: Envelope,
        ctx: &mut ActorContext,
    ) -> anyhow::Result<()> {
        if let Ok(msg) = envelope.downcast::<SupervisorMessage>() {
            match msg {
                SupervisorMessage::ChildFailed {
                    child_id,
                    error,
                    failure_kind,
                    retry_count,
                } => {
                    let count = self.retry_counts.entry(child_id.clone()).or_insert(0);
                    *count = retry_count;

                    let decision = self.decide(&child_id, failure_kind.as_ref(), retry_count);
                    match decision {
                        SupervisionDecision::Restart => {
                            info!(
                                supervisor = %self.id,
                                child = %child_id,
                                retry = retry_count,
                                "restarting child actor"
                            );
                            // TODO: 通过 ActorSystem 重启子 Actor
                        }
                        SupervisionDecision::Stop => {
                            warn!(
                                supervisor = %self.id,
                                child = %child_id,
                                error = %error,
                                "stopping child actor (max retries exceeded)"
                            );
                            ctx.remove_child(&child_id);
                        }
                        SupervisionDecision::Escalate => {
                            error!(
                                supervisor = %self.id,
                                child = %child_id,
                                "escalating failure to parent"
                            );
                            // TODO: 向父监督者发送失败消息
                        }
                        SupervisionDecision::Resume => {
                            info!(
                                supervisor = %self.id,
                                child = %child_id,
                                "resuming child actor"
                            );
                        }
                    }
                }
                SupervisorMessage::ChildStopped { child_id } => {
                    info!(supervisor = %self.id, child = %child_id, "child stopped");
                    ctx.remove_child(&child_id);
                    self.retry_counts.remove(&child_id);
                }
                SupervisorMessage::QueryChildren => {
                    let children: Vec<String> =
                        ctx.children().keys().map(|id| id.to_string()).collect();
                    info!(
                        supervisor = %self.id,
                        children = ?children,
                        "current children"
                    );
                }
            }
        }
        Ok(())
    }

    async fn on_stop(&mut self, _ctx: &mut ActorContext) -> anyhow::Result<()> {
        info!(supervisor = %self.id, "supervisor stopped");
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> &ActorId {
        &self.id
    }
}
