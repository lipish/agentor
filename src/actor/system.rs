use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::Mutex;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use super::actor::{Actor, ActorStatus};
use super::address::ActorRef;
use super::context::ActorContext;
use super::mailbox::Mailbox;
use super::message::{ActorId, Envelope, SystemMessage};
use crate::environment::Environment;

/// ActorSystem — Actor 运行时的核心管理器
///
/// 负责：
/// - spawn / stop Actor
/// - 维护 Actor 注册表
/// - 提供全局服务（Environment、观测等）
pub struct ActorSystem {
    name: String,
    actors: Arc<DashMap<ActorId, ActorEntry>>,
    environment: Arc<Environment>,
    handles: Vec<JoinHandle<()>>,
    /// 休眠 Actor 的暂存消息队列
    pending_messages: Arc<DashMap<ActorId, Mutex<VecDeque<Envelope>>>>,
}

struct ActorEntry {
    actor_ref: ActorRef,
    status: ActorStatus,
}

impl ActorSystem {
    /// 创建新的 ActorSystem
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actors: Arc::new(DashMap::new()),
            environment: Arc::new(Environment::new()),
            handles: Vec::new(),
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    /// 使用自定义 Environment 创建
    pub fn with_environment(name: impl Into<String>, env: Environment) -> Self {
        Self {
            name: name.into(),
            actors: Arc::new(DashMap::new()),
            environment: Arc::new(env),
            handles: Vec::new(),
            pending_messages: Arc::new(DashMap::new()),
        }
    }

    /// 获取系统名称
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取 Environment
    pub fn environment(&self) -> &Arc<Environment> {
        &self.environment
    }

    /// Spawn 一个新的 Actor，返回其 ActorRef
    pub fn spawn(&mut self, mut actor: Box<dyn Actor>, mailbox_capacity: usize) -> ActorRef {
        let actor_id = actor.id().clone();
        let actor_name = actor.name().to_string();
        let (mut mailbox, sender) = Mailbox::new(mailbox_capacity);
        let actor_ref = ActorRef::new(actor_id.clone(), sender.clone());

        let mut ctx = ActorContext {
            self_id: actor_id.clone(),
            self_ref: actor_ref.clone(),
            children: HashMap::new(),
            environment: self.environment.clone(),
        };

        let actors = self.actors.clone();
        let id_for_task = actor_id.clone();

        // 注册到 actor 表
        actors.insert(
            actor_id.clone(),
            ActorEntry {
                actor_ref: actor_ref.clone(),
                status: ActorStatus::Starting,
            },
        );

        let handle = tokio::spawn(async move {
            // on_start
            if let Err(e) = actor.on_start(&mut ctx).await {
                error!(actor = %id_for_task, "on_start failed: {e}");
                actors.entry(id_for_task.clone()).and_modify(|entry| {
                    entry.status = ActorStatus::Failed;
                });
                return;
            }

            actors.entry(id_for_task.clone()).and_modify(|entry| {
                entry.status = ActorStatus::Running;
            });
            info!(actor = %id_for_task, name = %actor_name, "actor started");

            // 消息循环（事务性）
            loop {
                match mailbox.recv().await {
                    Some(envelope) => {
                        // 检查是否是系统消息
                        if let Some(sys_msg) = envelope.downcast_ref::<SystemMessage>() {
                            match sys_msg {
                                SystemMessage::Stop => {
                                    info!(actor = %id_for_task, "received Stop signal");
                                    mailbox.commit();
                                    break;
                                }
                                SystemMessage::Ping => {
                                    mailbox.commit();
                                    continue;
                                }
                                _ => {}
                            }
                        }

                        // 记录消息元信息（handle_message 会 move envelope）
                        let msg_id = envelope.id;
                        let trace_id = envelope.trace_id;

                        match actor.handle_message(envelope, &mut ctx).await {
                            Ok(()) => {
                                mailbox.commit();
                            }
                            Err(e) => {
                                error!(actor = %id_for_task, msg_id = %msg_id, "handle_message error: {e}");
                                // envelope 已被 handle_message 消费，只能记录错误
                                mailbox.record_failure(msg_id, trace_id, &e.to_string());
                            }
                        }
                    }
                    None => {
                        info!(actor = %id_for_task, "mailbox closed, stopping");
                        break;
                    }
                }
            }

            // on_stop
            actors.entry(id_for_task.clone()).and_modify(|entry| {
                entry.status = ActorStatus::Stopping;
            });

            if let Err(e) = actor.on_stop(&mut ctx).await {
                warn!(actor = %id_for_task, "on_stop error: {e}");
            }

            actors.entry(id_for_task.clone()).and_modify(|entry| {
                entry.status = ActorStatus::Stopped;
            });
            info!(actor = %id_for_task, "actor stopped");
        });

        self.handles.push(handle);
        actor_ref
    }

    /// 使用默认信箱容量(256) spawn Actor
    pub fn spawn_default(&mut self, actor: Box<dyn Actor>) -> ActorRef {
        self.spawn(actor, 256)
    }

    /// 通过 ID 查找 ActorRef
    pub fn find(&self, id: &ActorId) -> Option<ActorRef> {
        self.actors.get(id).map(|entry| entry.actor_ref.clone())
    }

    /// 通过名称查找（返回第一个匹配的）
    pub fn find_by_name(&self, name: &str) -> Option<ActorRef> {
        self.actors
            .iter()
            .find(|entry| entry.key().name == name)
            .map(|entry| entry.value().actor_ref.clone())
    }

    /// 获取 Actor 状态
    pub fn status(&self, id: &ActorId) -> Option<ActorStatus> {
        self.actors.get(id).map(|entry| entry.status)
    }

    /// 向指定 Actor 发送停止信号
    pub async fn stop_actor(&self, id: &ActorId) -> anyhow::Result<()> {
        if let Some(entry) = self.actors.get(id) {
            entry
                .actor_ref
                .tell(SystemMessage::Stop)
                .await
                .map_err(|e| anyhow::anyhow!("failed to send stop: {e}"))?;
        }
        Ok(())
    }

    /// 关闭整个系统：向所有 Actor 发送 Stop，等待全部退出
    pub async fn shutdown(mut self) {
        info!(system = %self.name, "shutting down actor system");

        // 向所有 actor 发送 Stop
        for entry in self.actors.iter() {
            let _ = entry.value().actor_ref.tell(SystemMessage::Stop).await;
        }

        // 等待所有 task 完成
        for handle in self.handles.drain(..) {
            let _ = handle.await;
        }

        info!(system = %self.name, "actor system shut down");
    }

    /// 当前活跃 Actor 数量
    pub fn actor_count(&self) -> usize {
        self.actors
            .iter()
            .filter(|e| e.status == ActorStatus::Running)
            .count()
    }

    /// 休眠指定 Actor
    ///
    /// 发送 Stop 信号让 Actor 正常停止（触发 on_stop 保存 checkpoint），
    /// 然后标记为 Hibernated。后续发给该 Actor 的消息会暂存到 pending_messages。
    pub async fn hibernate(&self, id: &ActorId) -> anyhow::Result<()> {
        let status = self.status(id);
        match status {
            Some(ActorStatus::Running) => {
                // 初始化暂存队列
                self.pending_messages
                    .entry(id.clone())
                    .or_insert_with(|| Mutex::new(VecDeque::new()));

                // 发送 Stop 信号
                self.stop_actor(id).await?;

                // 等待状态变为 Stopped，然后标记为 Hibernated
                // 简化实现：直接标记，实际状态会由 task 异步更新
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                self.actors.entry(id.clone()).and_modify(|entry| {
                    if entry.status == ActorStatus::Stopped {
                        entry.status = ActorStatus::Hibernated;
                    }
                });

                info!(actor = %id, "actor hibernated");
                Ok(())
            }
            Some(ActorStatus::Hibernated) => {
                // 已经休眠
                Ok(())
            }
            _ => Err(anyhow::anyhow!(
                "cannot hibernate actor in state {:?}",
                status
            )),
        }
    }

    /// 唤醒休眠的 Actor
    ///
    /// 调用方需提供一个新的 Actor 实例（已从 checkpoint 恢复状态）。
    /// ActorSystem 会重新 spawn 并重放暂存的消息。
    pub async fn thaw(
        &mut self,
        id: &ActorId,
        actor: Box<dyn Actor>,
        mailbox_capacity: usize,
    ) -> anyhow::Result<ActorRef> {
        let status = self.status(id);
        if status != Some(ActorStatus::Hibernated) {
            return Err(anyhow::anyhow!(
                "cannot thaw actor in state {:?}, expected Hibernated",
                status
            ));
        }

        // 移除旧的注册表条目
        self.actors.remove(id);

        // 重新 spawn
        let actor_ref = self.spawn(actor, mailbox_capacity);

        // 重放暂存的消息
        if let Some((_, pending)) = self.pending_messages.remove(id) {
            let messages: VecDeque<Envelope> = pending.into_inner();
            let count = messages.len();
            for envelope in messages {
                if let Err(e) = actor_ref.send_envelope(envelope).await {
                    warn!(actor = %id, error = %e, "failed to replay pending message");
                }
            }
            if count > 0 {
                info!(actor = %id, replayed = count, "replayed pending messages after thaw");
            }
        }

        info!(actor = %id, "actor thawed");
        Ok(actor_ref)
    }

    /// 向休眠的 Actor 暂存消息（内部使用）
    pub fn buffer_message(&self, id: &ActorId, envelope: Envelope) -> bool {
        if let Some(entry) = self.pending_messages.get(id) {
            entry.value().lock().push_back(envelope);
            true
        } else {
            false
        }
    }

    /// 查看指定 Actor 的暂存消息数量
    pub fn pending_message_count(&self, id: &ActorId) -> usize {
        self.pending_messages
            .get(id)
            .map(|entry| entry.value().lock().len())
            .unwrap_or(0)
    }
}
