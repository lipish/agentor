use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Main configuration structure for Agentor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub watch: WatchConfig,
    pub build: BuildConfig,
    pub deploy: DeployConfig,
    pub sync: SyncConfig,
    pub rollback: RollbackConfig,
    pub log: LogConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchConfig {
    pub repo_path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub target_dir: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub enabled: bool,
    pub remote: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    pub enabled: bool,
    pub keep_versions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub file: String,
    pub level: String,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        
        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;
        
        Ok(config)
    }
    
    /// Create a default configuration
    pub fn default() -> Self {
        Config {
            watch: WatchConfig {
                repo_path: ".".to_string(),
                branch: "main".to_string(),
            },
            build: BuildConfig {
                command: "cargo build --release".to_string(),
            },
            deploy: DeployConfig {
                command: String::new(),
                target_dir: String::new(),
                artifacts: Vec::new(),
            },
            sync: SyncConfig {
                enabled: true,
                remote: "origin".to_string(),
                branch: "main".to_string(),
            },
            rollback: RollbackConfig {
                enabled: true,
                keep_versions: 3,
            },
            log: LogConfig {
                file: "agentor.log".to_string(),
                level: "info".to_string(),
            },
        }
    }
    
    /// Save configuration to a TOML file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;
        
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;
        
        Ok(())
    }
}
