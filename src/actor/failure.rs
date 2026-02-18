use std::fmt;
use std::time::Duration;

/// FailureKind — Agent 场景的三级故障分类
///
/// 不同类型的故障对应不同的恢复策略：
/// - Transient: 自动指数退避重试
/// - Logic: 反思后重试（修正 prompt）
/// - Critical: 立即停止 + 告警
#[derive(Debug, Clone)]
pub enum FailureKind {
    /// 临时性故障：网络超时、Rate Limit、Provider 不可用
    Transient {
        /// 建议的重试延迟（指数退避基数）
        backoff_base: Duration,
    },
    /// 逻辑错误：工具输入格式错误、JSON 解析失败、参数校验不通过
    Logic {
        /// 错误上下文，可注入到下次 LLM 调用的 prompt 中做自我反思
        context: String,
    },
    /// 严重故障：预算耗尽、安全违规、持续幻觉
    Critical {
        /// 是否需要告警（通知人类）
        alert: bool,
    },
}

impl FailureKind {
    /// 快捷构造：网络超时类临时故障
    pub fn transient() -> Self {
        Self::Transient {
            backoff_base: Duration::from_millis(500),
        }
    }

    /// 快捷构造：自定义退避基数的临时故障
    pub fn transient_with_backoff(base_ms: u64) -> Self {
        Self::Transient {
            backoff_base: Duration::from_millis(base_ms),
        }
    }

    /// 快捷构造：逻辑错误
    pub fn logic(context: impl Into<String>) -> Self {
        Self::Logic {
            context: context.into(),
        }
    }

    /// 快捷构造：严重故障（需要告警）
    pub fn critical() -> Self {
        Self::Critical { alert: true }
    }

    /// 快捷构造：严重故障（静默）
    pub fn critical_silent() -> Self {
        Self::Critical { alert: false }
    }

    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Transient { .. } | Self::Logic { .. })
    }
}

impl fmt::Display for FailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transient { backoff_base } => {
                write!(f, "Transient(backoff={}ms)", backoff_base.as_millis())
            }
            Self::Logic { context } => write!(f, "Logic({})", context),
            Self::Critical { alert } => write!(f, "Critical(alert={})", alert),
        }
    }
}

/// AgentError — 携带 FailureKind 的错误类型
///
/// Actor 的 handle_message 可以返回 `anyhow::Error`，
/// 通过 downcast 提取 AgentError 获得故障分类信息。
#[derive(Debug)]
pub struct AgentError {
    pub kind: FailureKind,
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AgentError {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(
        kind: FailureKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 从 anyhow::Error 中尝试提取 AgentError
    pub fn extract(err: &anyhow::Error) -> Option<&AgentError> {
        err.downcast_ref::<AgentError>()
    }
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|s| s.as_ref() as &(dyn std::error::Error + 'static))
    }
}
