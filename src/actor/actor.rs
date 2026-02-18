use async_trait::async_trait;

use super::context::ActorContext;
use super::message::{ActorId, Envelope};

/// Actor 的运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorStatus {
    Starting,
    Running,
    Suspended,
    Stopping,
    Stopped,
    Failed,
    /// Actor 已休眠：状态已持久化，tokio task 已停止，发给它的消息暂存
    Hibernated,
}

/// Actor trait — 所有 Actor 必须实现此 trait
///
/// 生命周期：on_start → (handle_message 循环) → on_stop
/// 如果 handle_message 返回 Err，监督者可根据策略决定重启或停止
#[async_trait]
pub trait Actor: Send + 'static {
    /// Actor 启动时调用（初始化资源、恢复状态等）
    async fn on_start(&mut self, ctx: &mut ActorContext) -> anyhow::Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// 处理收到的消息
    async fn handle_message(
        &mut self,
        envelope: Envelope,
        ctx: &mut ActorContext,
    ) -> anyhow::Result<()>;

    /// Actor 停止前调用（清理资源、持久化状态等）
    async fn on_stop(&mut self, ctx: &mut ActorContext) -> anyhow::Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Actor 崩溃后、重启前调用（可用于记录错误、清理脏状态）
    async fn on_restart(
        &mut self,
        error: &anyhow::Error,
        ctx: &mut ActorContext,
    ) -> anyhow::Result<()> {
        let _ = (error, ctx);
        Ok(())
    }

    /// 返回 Actor 的名称（用于日志和监控）
    fn name(&self) -> &str;

    /// 返回 Actor 的 ID
    fn id(&self) -> &ActorId;
}
