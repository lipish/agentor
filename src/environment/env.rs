use std::collections::HashMap;

use parking_lot::RwLock;

/// Environment — 统一的环境上下文与凭证容器
///
/// 所有 Actor 共享（只读访问），支持：
/// - 配置项（key-value）
/// - 加密凭证（API Keys 等）
/// - 运行时参数
pub struct Environment {
    config: RwLock<HashMap<String, String>>,
    secrets: RwLock<HashMap<String, String>>,
}

impl Environment {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(HashMap::new()),
            secrets: RwLock::new(HashMap::new()),
        }
    }

    /// 设置配置项
    pub fn set_config(&self, key: impl Into<String>, value: impl Into<String>) {
        self.config.write().insert(key.into(), value.into());
    }

    /// 获取配置项
    pub fn get_config(&self, key: &str) -> Option<String> {
        self.config.read().get(key).cloned()
    }

    /// 注入凭证（生产环境应从 Vault / 环境变量加载）
    pub fn set_secret(&self, key: impl Into<String>, value: impl Into<String>) {
        self.secrets.write().insert(key.into(), value.into());
    }

    /// 获取凭证
    pub fn get_secret(&self, key: &str) -> Option<String> {
        self.secrets.read().get(key).cloned()
    }

    /// 检查凭证是否存在
    pub fn has_secret(&self, key: &str) -> bool {
        self.secrets.read().contains_key(key)
    }

    /// 从环境变量批量加载配置
    pub fn load_from_env(&self, prefix: &str) {
        for (key, value) in std::env::vars() {
            if key.starts_with(prefix) {
                let config_key = key.strip_prefix(prefix).unwrap_or(&key).to_lowercase();
                self.set_config(config_key, value);
            }
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}
