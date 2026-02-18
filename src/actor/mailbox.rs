use std::collections::VecDeque;

use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use super::message::Envelope;

/// Mailbox — Actor 的事务性消息信箱
///
/// 支持两阶段消息处理：
/// - `recv()` 取出消息，内部记录 in-flight 状态
/// - 处理成功 → `commit()` 清除 in-flight
/// - 处理失败 → `nack(envelope, error)` 退回重试队列
///
/// 被 nack 退回的消息优先于新消息被消费。
/// 超过最大重试次数的消息进入 Dead Letter Queue (DLQ)。
///
/// 注意：由于 Envelope 包含 `Box<dyn Any + Send>` 不可 Clone，
/// nack 时需要调用方将 envelope 传回。如果 envelope 已被 handle_message
/// 消费（move），则使用 `record_failure()` 仅记录错误到 DLQ。
pub struct Mailbox {
    receiver: mpsc::Receiver<Envelope>,
    capacity: usize,
    /// 被 nack 退回的消息，优先于 channel 消费
    retry_queue: VecDeque<RetryEntry>,
    /// 死信队列
    dead_letters: VecDeque<DeadLetter>,
    /// 当前 in-flight 消息的重试计数
    inflight_retry_count: Option<u32>,
    /// 最大重试次数（默认 3）
    max_retries: u32,
    /// DLQ 容量上限（默认 100）
    dlq_capacity: usize,
}

struct RetryEntry {
    envelope: Envelope,
    retry_count: u32,
}

/// 死信条目 — 处理失败且超过重试上限的消息
pub struct DeadLetter {
    pub msg_id: Uuid,
    pub trace_id: Uuid,
    pub retry_count: u32,
    pub last_error: String,
    /// 如果 envelope 可以被传回，则保留原始消息；否则为 None
    pub envelope: Option<Envelope>,
}

impl std::fmt::Debug for DeadLetter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeadLetter")
            .field("msg_id", &self.msg_id)
            .field("retry_count", &self.retry_count)
            .field("last_error", &self.last_error)
            .finish()
    }
}

/// MailboxSender — 用于向 Mailbox 发送消息的句柄，可 Clone
#[derive(Clone)]
pub struct MailboxSender {
    sender: mpsc::Sender<Envelope>,
}

impl Mailbox {
    pub fn new(capacity: usize) -> (Self, MailboxSender) {
        let (sender, receiver) = mpsc::channel(capacity);
        (
            Self {
                receiver,
                capacity,
                retry_queue: VecDeque::new(),
                dead_letters: VecDeque::new(),
                inflight_retry_count: None,
                max_retries: 3,
                dlq_capacity: 100,
            },
            MailboxSender { sender },
        )
    }

    /// 默认容量 256
    pub fn default_capacity() -> (Self, MailboxSender) {
        Self::new(256)
    }

    /// 配置最大重试次数
    pub fn set_max_retries(&mut self, max: u32) {
        self.max_retries = max;
    }

    /// 从信箱接收下一条消息
    ///
    /// 优先从 retry_queue 取（被 nack 退回的消息），否则从 channel 取新消息。
    pub async fn recv(&mut self) -> Option<Envelope> {
        if let Some(entry) = self.retry_queue.pop_front() {
            self.inflight_retry_count = Some(entry.retry_count);
            Some(entry.envelope)
        } else {
            let envelope = self.receiver.recv().await?;
            self.inflight_retry_count = Some(0);
            Some(envelope)
        }
    }

    /// 确认消息已成功处理
    pub fn commit(&mut self) {
        self.inflight_retry_count = None;
    }

    /// 消息处理失败，将 envelope 退回重试队列或进入 DLQ
    ///
    /// 适用于 envelope 未被消费的场景（如系统消息检查阶段失败）。
    pub fn nack(&mut self, envelope: Envelope, error: &str) {
        let prev_count = self.inflight_retry_count.take().unwrap_or(0);
        let retry_count = prev_count + 1;

        if retry_count >= self.max_retries {
            warn!(
                msg_id = %envelope.id,
                retries = retry_count,
                error = %error,
                "message exceeded max retries, moving to DLQ"
            );
            let msg_id = envelope.id;
            let trace_id = envelope.trace_id;
            self.push_dead_letter(DeadLetter {
                msg_id,
                trace_id,
                retry_count,
                last_error: error.to_string(),
                envelope: Some(envelope),
            });
        } else {
            warn!(
                msg_id = %envelope.id,
                retry = retry_count,
                error = %error,
                "nack: message will be retried"
            );
            self.retry_queue.push_back(RetryEntry {
                envelope,
                retry_count,
            });
        }
    }

    /// 记录处理失败（envelope 已被 handle_message 消费，无法退回）
    ///
    /// 仅记录错误信息到 DLQ，不保留原始消息。
    pub fn record_failure(&mut self, msg_id: Uuid, trace_id: Uuid, error: &str) {
        let retry_count = self.inflight_retry_count.take().unwrap_or(0) + 1;
        warn!(
            msg_id = %msg_id,
            retries = retry_count,
            error = %error,
            "message processing failed, recorded to DLQ"
        );
        self.push_dead_letter(DeadLetter {
            msg_id,
            trace_id,
            retry_count,
            last_error: error.to_string(),
            envelope: None,
        });
    }

    fn push_dead_letter(&mut self, dl: DeadLetter) {
        if self.dead_letters.len() >= self.dlq_capacity {
            self.dead_letters.pop_front();
        }
        self.dead_letters.push_back(dl);
    }

    /// DLQ 中的死信数量
    pub fn dead_letter_count(&self) -> usize {
        self.dead_letters.len()
    }

    /// retry_queue 中的待重试消息数量
    pub fn retry_queue_len(&self) -> usize {
        self.retry_queue.len()
    }

    /// 排空 DLQ，返回所有死信
    pub fn drain_dead_letters(&mut self) -> Vec<DeadLetter> {
        self.dead_letters.drain(..).collect()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

impl MailboxSender {
    /// 发送消息，如果信箱满则等待（背压）
    pub async fn send(&self, envelope: Envelope) -> Result<(), MailboxSendError> {
        self.sender
            .send(envelope)
            .await
            .map_err(|_| MailboxSendError::ActorStopped)
    }

    /// 尝试发送，不等待。信箱满时返回错误
    pub fn try_send(&self, envelope: Envelope) -> Result<(), MailboxSendError> {
        self.sender.try_send(envelope).map_err(|e| match e {
            mpsc::error::TrySendError::Full(_) => {
                warn!("mailbox full, message dropped");
                MailboxSendError::MailboxFull
            }
            mpsc::error::TrySendError::Closed(_) => MailboxSendError::ActorStopped,
        })
    }

    /// 信箱是否已关闭
    pub fn is_closed(&self) -> bool {
        self.sender.is_closed()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MailboxSendError {
    #[error("actor has stopped, mailbox closed")]
    ActorStopped,
    #[error("mailbox is full")]
    MailboxFull,
}
