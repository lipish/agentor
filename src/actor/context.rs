use std::collections::HashMap;
use std::sync::Arc;

use super::address::ActorRef;
use super::message::ActorId;

/// ActorContext — Actor 运行时上下文
///
/// 提供给 Actor 在 handle_message 中使用的能力：
/// - 获取自身 ID / Ref
/// - 访问子 Actor
/// - 访问系统级服务（环境变量、预算等）
pub struct ActorContext {
    /// 自身 ID
    pub(crate) self_id: ActorId,
    /// 自身的 ActorRef（可用于告诉别人"回复我"）
    pub(crate) self_ref: ActorRef,
    /// 子 Actor 列表
    pub(crate) children: HashMap<ActorId, ActorRef>,
    /// 共享的系统环境（只读）
    pub(crate) environment: Arc<crate::environment::Environment>,
}

impl ActorContext {
    /// 获取自身 ID
    pub fn self_id(&self) -> &ActorId {
        &self.self_id
    }

    /// 获取自身的 ActorRef
    pub fn self_ref(&self) -> &ActorRef {
        &self.self_ref
    }

    /// 获取子 Actor 的引用
    pub fn child(&self, id: &ActorId) -> Option<&ActorRef> {
        self.children.get(id)
    }

    /// 获取所有子 Actor
    pub fn children(&self) -> &HashMap<ActorId, ActorRef> {
        &self.children
    }

    /// 注册子 Actor
    pub fn register_child(&mut self, id: ActorId, actor_ref: ActorRef) {
        self.children.insert(id, actor_ref);
    }

    /// 移除子 Actor
    pub fn remove_child(&mut self, id: &ActorId) -> Option<ActorRef> {
        self.children.remove(id)
    }

    /// 访问环境配置
    pub fn environment(&self) -> &crate::environment::Environment {
        &self.environment
    }
}
