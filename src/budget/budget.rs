use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tracing::warn;

/// TokenBudget — 单个 Actor / Actor Group 的 Token 预算管理
///
/// 线程安全，可在多个 Actor 间共享
#[derive(Clone)]
pub struct TokenBudget {
    inner: Arc<BudgetInner>,
}

struct BudgetInner {
    limit: AtomicU64,
    used: AtomicU64,
    tripped: AtomicBool,
    name: String,
}

impl TokenBudget {
    pub fn new(name: impl Into<String>, limit: u64) -> Self {
        Self {
            inner: Arc::new(BudgetInner {
                limit: AtomicU64::new(limit),
                used: AtomicU64::new(0),
                tripped: AtomicBool::new(false),
                name: name.into(),
            }),
        }
    }

    /// 无限预算
    pub fn unlimited(name: impl Into<String>) -> Self {
        Self::new(name, u64::MAX)
    }

    /// 尝试消费 tokens，如果超出预算返回 false
    pub fn try_consume(&self, tokens: u64) -> bool {
        if self.inner.tripped.load(Ordering::Relaxed) {
            return false;
        }

        let prev = self.inner.used.fetch_add(tokens, Ordering::Relaxed);
        let new_total = prev + tokens;
        let limit = self.inner.limit.load(Ordering::Relaxed);

        if new_total > limit {
            self.inner.tripped.store(true, Ordering::Relaxed);
            warn!(
                budget = %self.inner.name,
                used = new_total,
                limit = limit,
                "budget exceeded, circuit breaker tripped"
            );
            false
        } else {
            true
        }
    }

    /// 获取已使用量
    pub fn used(&self) -> u64 {
        self.inner.used.load(Ordering::Relaxed)
    }

    /// 获取上限
    pub fn limit(&self) -> u64 {
        self.inner.limit.load(Ordering::Relaxed)
    }

    /// 获取剩余量
    pub fn remaining(&self) -> u64 {
        let limit = self.limit();
        let used = self.used();
        limit.saturating_sub(used)
    }

    /// 是否已熔断
    pub fn is_tripped(&self) -> bool {
        self.inner.tripped.load(Ordering::Relaxed)
    }

    /// 重置预算（管理员操作）
    pub fn reset(&self) {
        self.inner.used.store(0, Ordering::Relaxed);
        self.inner.tripped.store(false, Ordering::Relaxed);
    }

    /// 修改上限
    pub fn set_limit(&self, new_limit: u64) {
        self.inner.limit.store(new_limit, Ordering::Relaxed);
        // 如果新上限大于已使用量，解除熔断
        if new_limit > self.used() {
            self.inner.tripped.store(false, Ordering::Relaxed);
        }
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }
}
