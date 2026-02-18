use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use super::state::AgentState;
use crate::actor::message::ActorId;

/// Checkpoint — Agent 状态的持久化快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub actor_id: ActorId,
    pub timestamp: DateTime<Utc>,
    pub state: AgentState,
    pub version: u64,
}

/// CheckpointStore — 管理 Checkpoint 的存储和恢复
///
/// 当前实现基于文件系统（JSON），未来可扩展为数据库/对象存储
pub struct CheckpointStore {
    base_dir: PathBuf,
}

impl CheckpointStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// 保存 checkpoint
    pub async fn save(&self, checkpoint: &Checkpoint) -> anyhow::Result<()> {
        let dir = self.actor_dir(&checkpoint.actor_id);
        tokio::fs::create_dir_all(&dir).await?;

        let filename = format!("checkpoint_{:06}.json", checkpoint.version);
        let path = dir.join(filename);
        let data = serde_json::to_string_pretty(checkpoint)?;
        tokio::fs::write(&path, data).await?;

        info!(
            actor = %checkpoint.actor_id,
            version = checkpoint.version,
            "checkpoint saved"
        );
        Ok(())
    }

    /// 加载最新的 checkpoint
    pub async fn load_latest(&self, actor_id: &ActorId) -> anyhow::Result<Option<Checkpoint>> {
        let dir = self.actor_dir(actor_id);
        if !dir.exists() {
            return Ok(None);
        }

        let mut entries = tokio::fs::read_dir(&dir).await?;
        let mut latest: Option<(u64, PathBuf)> = None;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(version_str) = name
                .strip_prefix("checkpoint_")
                .and_then(|s| s.strip_suffix(".json"))
            {
                if let Ok(version) = version_str.parse::<u64>() {
                    match &latest {
                        Some((v, _)) if version > *v => {
                            latest = Some((version, entry.path()));
                        }
                        None => {
                            latest = Some((version, entry.path()));
                        }
                        _ => {}
                    }
                }
            }
        }

        match latest {
            Some((version, path)) => {
                let data = tokio::fs::read_to_string(&path).await?;
                let checkpoint: Checkpoint = serde_json::from_str(&data)?;
                info!(actor = %actor_id, version = version, "checkpoint loaded");
                Ok(Some(checkpoint))
            }
            None => {
                warn!(actor = %actor_id, "no checkpoint found");
                Ok(None)
            }
        }
    }

    /// 加载指定版本的 checkpoint
    pub async fn load_version(
        &self,
        actor_id: &ActorId,
        version: u64,
    ) -> anyhow::Result<Option<Checkpoint>> {
        let path = self
            .actor_dir(actor_id)
            .join(format!("checkpoint_{:06}.json", version));

        if !path.exists() {
            return Ok(None);
        }

        let data = tokio::fs::read_to_string(&path).await?;
        let checkpoint: Checkpoint = serde_json::from_str(&data)?;
        Ok(Some(checkpoint))
    }

    fn actor_dir(&self, actor_id: &ActorId) -> PathBuf {
        self.base_dir.join(actor_id.id.to_string())
    }
}
