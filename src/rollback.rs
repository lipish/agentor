use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const AGENTOR_DIR: &str = ".agentor";
const VERSIONS_DIR: &str = "versions";
const STATE_FILE: &str = "state.json";

/// Version metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub commit: String,
    pub timestamp: DateTime<Utc>,
    pub artifacts: Vec<String>,
}

/// State structure for tracking deployment history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub versions: Vec<Version>,
    pub current_version: Option<String>,
}

impl State {
    fn new() -> Self {
        State {
            versions: Vec::new(),
            current_version: None,
        }
    }
}

/// Initialize agentor directory structure
pub fn init_agentor_dir<P: AsRef<Path>>(repo_path: P) -> Result<()> {
    let agentor_path = repo_path.as_ref().join(AGENTOR_DIR);
    let versions_path = agentor_path.join(VERSIONS_DIR);
    
    fs::create_dir_all(&versions_path)
        .with_context(|| format!("Failed to create agentor directory: {}", agentor_path.display()))?;
    
    Ok(())
}

/// Get the state file path
fn get_state_path<P: AsRef<Path>>(repo_path: P) -> PathBuf {
    repo_path.as_ref().join(AGENTOR_DIR).join(STATE_FILE)
}

/// Load state from file
pub fn load_state<P: AsRef<Path>>(repo_path: P) -> Result<State> {
    let state_path = get_state_path(repo_path);
    
    if !state_path.exists() {
        return Ok(State::new());
    }
    
    let content = fs::read_to_string(&state_path)
        .with_context(|| format!("Failed to read state file: {}", state_path.display()))?;
    
    let state: State = serde_json::from_str(&content)
        .with_context(|| "Failed to parse state file")?;
    
    Ok(state)
}

/// Save state to file
pub fn save_state<P: AsRef<Path>>(repo_path: P, state: &State) -> Result<()> {
    let state_path = get_state_path(repo_path);
    
    let content = serde_json::to_string_pretty(state)
        .with_context(|| "Failed to serialize state")?;
    
    fs::write(&state_path, content)
        .with_context(|| format!("Failed to write state file: {}", state_path.display()))?;
    
    Ok(())
}

/// Backup current version
pub fn backup_version<P: AsRef<Path>>(
    repo_path: P,
    commit: &str,
    artifacts: &[String],
    keep_versions: usize,
) -> Result<()> {
    log::info!("Backing up version: {}", commit);
    
    let agentor_path = repo_path.as_ref().join(AGENTOR_DIR);
    let versions_path = agentor_path.join(VERSIONS_DIR);
    let version_path = versions_path.join(commit);
    
    // Create version directory
    fs::create_dir_all(&version_path)
        .with_context(|| format!("Failed to create version directory: {}", version_path.display()))?;
    
    // Copy artifacts to version directory
    for artifact in artifacts {
        let source = Path::new(artifact);
        if source.exists() {
            let file_name = source.file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid artifact path: {}", artifact))?;
            let dest = version_path.join(file_name);
            
            fs::copy(source, &dest)
                .with_context(|| format!("Failed to backup artifact: {}", artifact))?;
        }
    }
    
    // Update state
    let mut state = load_state(&repo_path)?;
    
    let version = Version {
        commit: commit.to_string(),
        timestamp: Utc::now(),
        artifacts: artifacts.to_vec(),
    };
    
    state.versions.push(version);
    state.current_version = Some(commit.to_string());
    
    // Keep only the most recent versions
    if state.versions.len() > keep_versions {
        let to_remove = state.versions.len() - keep_versions;
        for version in state.versions.drain(..to_remove) {
            let old_version_path = versions_path.join(&version.commit);
            if old_version_path.exists() {
                fs::remove_dir_all(&old_version_path).ok();
            }
        }
    }
    
    save_state(&repo_path, &state)?;
    
    log::info!("Version backed up successfully");
    Ok(())
}

/// Rollback to previous version
pub fn rollback<P: AsRef<Path>>(repo_path: P, target_dir: &str) -> Result<()> {
    log::info!("Starting rollback...");
    
    let mut state = load_state(&repo_path)?;
    
    if state.versions.len() < 2 {
        anyhow::bail!("No previous version available for rollback");
    }
    
    // Get previous version (second to last)
    let prev_version = &state.versions[state.versions.len() - 2];
    let commit = &prev_version.commit;
    
    log::info!("Rolling back to version: {}", commit);
    
    let versions_path = repo_path.as_ref().join(AGENTOR_DIR).join(VERSIONS_DIR);
    let version_path = versions_path.join(commit);
    
    if !version_path.exists() {
        anyhow::bail!("Version directory not found: {}", version_path.display());
    }
    
    // Copy artifacts from version directory to target directory
    if !target_dir.is_empty() {
        let target_path = Path::new(target_dir);
        for artifact in &prev_version.artifacts {
            let file_name = Path::new(artifact).file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid artifact path: {}", artifact))?;
            
            let source = version_path.join(file_name);
            let dest = target_path.join(file_name);
            
            if source.exists() {
                log::info!("Restoring {} to {}", source.display(), dest.display());
                fs::copy(&source, &dest)
                    .with_context(|| format!("Failed to restore artifact: {}", artifact))?;
            }
        }
    }
    
    // Update state to point to previous version
    state.current_version = Some(commit.to_string());
    save_state(&repo_path, &state)?;
    
    log::info!("Rollback completed successfully");
    Ok(())
}
