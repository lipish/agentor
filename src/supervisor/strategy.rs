/// 监督策略 — 定义子 Actor 失败时的处理方式
#[derive(Debug, Clone)]
pub enum SupervisionStrategy {
    /// 只重启失败的那个子 Actor
    OneForOne { max_retries: u32, within_secs: u64 },
    /// 一个失败，全部重启
    AllForOne { max_retries: u32, within_secs: u64 },
    /// 停止失败的子 Actor，不重启
    Stop,
    /// 将失败上报给父监督者
    Escalate,
}

impl Default for SupervisionStrategy {
    fn default() -> Self {
        Self::OneForOne {
            max_retries: 3,
            within_secs: 60,
        }
    }
}

/// 监督决策
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionDecision {
    /// 重启 Actor
    Restart,
    /// 停止 Actor
    Stop,
    /// 上报给上级监督者
    Escalate,
    /// 忽略错误，继续运行
    Resume,
}
