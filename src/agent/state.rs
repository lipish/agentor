use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// AgentState — Agent 的私有状态（记忆）
///
/// 包含短期记忆（最近 N 轮对话）和长期记忆（持久化 KV）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// 短期记忆：最近的消息/事件，FIFO 队列
    pub short_term: VecDeque<MemoryEntry>,
    /// 短期记忆容量上限
    pub short_term_capacity: usize,
    /// 长期记忆：持久化的 key-value 对
    pub long_term: Vec<MemoryEntry>,
    /// 当前执行阶段（状态机）
    pub phase: AgentPhase,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
    /// 累计处理的消息数
    pub message_count: u64,
    /// 累计消耗的 token 数（如果涉及 LLM 调用）
    pub token_usage: u64,
}

/// 记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub timestamp: DateTime<Utc>,
    pub role: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
}

/// Agent 执行阶段（状态机）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentPhase {
    /// 空闲，等待输入
    Idle,
    /// 正在思考/规划
    Thinking,
    /// 正在执行工具调用
    Executing,
    /// 等待人类审批（AwaitHuman）
    AwaitingHuman,
    /// 正在生成流式输出
    Streaming,
    /// 已完成当前任务
    Completed,
    /// 出错
    Failed,
}

impl AgentState {
    pub fn new(short_term_capacity: usize) -> Self {
        Self {
            short_term: VecDeque::with_capacity(short_term_capacity),
            short_term_capacity,
            long_term: Vec::new(),
            phase: AgentPhase::Idle,
            last_active: Utc::now(),
            message_count: 0,
            token_usage: 0,
        }
    }

    /// 添加短期记忆，超过容量时自动淘汰最旧的
    pub fn push_short_term(&mut self, entry: MemoryEntry) {
        if self.short_term.len() >= self.short_term_capacity {
            self.short_term.pop_front();
        }
        self.short_term.push_back(entry);
    }

    /// 添加长期记忆
    pub fn push_long_term(&mut self, entry: MemoryEntry) {
        self.long_term.push(entry);
    }

    /// 获取最近 N 条短期记忆
    pub fn recent_memories(&self, n: usize) -> Vec<&MemoryEntry> {
        self.short_term.iter().rev().take(n).collect()
    }

    /// 更新活跃时间
    pub fn touch(&mut self) {
        self.last_active = Utc::now();
    }

    /// 记录 token 消耗
    pub fn add_token_usage(&mut self, tokens: u64) {
        self.token_usage += tokens;
    }
}

impl Default for AgentState {
    fn default() -> Self {
        Self::new(50)
    }
}
