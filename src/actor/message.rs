use std::any::Any;
use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 消息信封 — 包装实际消息体，附加元数据（trace_id、时间戳、发送者）
#[derive(Debug)]
pub struct Envelope {
    pub id: Uuid,
    pub trace_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub sender: Option<ActorId>,
    pub payload: Box<dyn Any + Send>,
}

impl Envelope {
    pub fn new(payload: Box<dyn Any + Send>, sender: Option<ActorId>) -> Self {
        let trace_id = Uuid::new_v4();
        Self::with_trace(payload, sender, trace_id)
    }

    pub fn with_trace(
        payload: Box<dyn Any + Send>,
        sender: Option<ActorId>,
        trace_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            trace_id,
            timestamp: Utc::now(),
            sender,
            payload,
        }
    }

    /// 尝试将 payload 向下转型为具体类型
    pub fn downcast<T: 'static>(self) -> Result<T, Self> {
        match self.payload.downcast::<T>() {
            Ok(val) => Ok(*val),
            Err(payload) => Err(Self { payload, ..self }),
        }
    }

    /// 尝试获取 payload 的引用
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

/// Actor 的唯一标识
#[derive(Debug, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActorId {
    pub id: Uuid,
    pub name: String,
}

impl ActorId {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
        }
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, &self.id.to_string()[..8])
    }
}

/// 系统级控制消息
#[derive(Debug, Clone)]
pub enum SystemMessage {
    /// 正常停止
    Stop,
    /// 重启（由监督者发出）
    Restart,
    /// 检查存活
    Ping,
    /// 存活回复
    Pong,
}
