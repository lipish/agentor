use std::fmt;

use super::mailbox::{MailboxSendError, MailboxSender};
use super::message::{ActorId, Envelope};

/// ActorRef — Actor 的外部引用句柄，用于向 Actor 发送消息
///
/// 位置透明：无论 Actor 在本地还是远程，使用方式一致
#[derive(Clone)]
pub struct ActorRef {
    pub(crate) id: ActorId,
    pub(crate) sender: MailboxSender,
}

impl ActorRef {
    pub fn new(id: ActorId, sender: MailboxSender) -> Self {
        Self { id, sender }
    }

    /// 获取目标 Actor 的 ID
    pub fn id(&self) -> &ActorId {
        &self.id
    }

    /// 发送消息（异步，背压）
    pub async fn tell(&self, payload: impl std::any::Any + Send) -> Result<(), MailboxSendError> {
        let envelope = Envelope::new(Box::new(payload), None);
        self.sender.send(envelope).await
    }

    /// 带发送者信息的消息发送
    pub async fn tell_from(
        &self,
        payload: impl std::any::Any + Send,
        sender: ActorId,
    ) -> Result<(), MailboxSendError> {
        let envelope = Envelope::new(Box::new(payload), Some(sender));
        self.sender.send(envelope).await
    }

    /// 发送原始 Envelope
    pub async fn send_envelope(&self, envelope: Envelope) -> Result<(), MailboxSendError> {
        self.sender.send(envelope).await
    }

    /// 尝试发送（不等待，信箱满时失败）
    pub fn try_tell(&self, payload: impl std::any::Any + Send) -> Result<(), MailboxSendError> {
        let envelope = Envelope::new(Box::new(payload), None);
        self.sender.try_send(envelope)
    }

    /// Actor 是否已停止
    pub fn is_stopped(&self) -> bool {
        self.sender.is_closed()
    }
}

impl fmt::Debug for ActorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorRef({})", self.id)
    }
}

impl fmt::Display for ActorRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorRef({})", self.id)
    }
}
