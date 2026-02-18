use anyhow::{Context, Result};
use std::process::Command;

/// Sync code to remote repository using git push
pub fn sync(remote: &str, branch: &str) -> Result<()> {
    log::info!("Syncing to remote repository...");
    log::info!("Remote: {}, Branch: {}", remote, branch);
    
    let output = Command::new("git")
        .args(&["push", remote, branch])
        .output()
        .with_context(|| "Failed to execute git push")?;
    
    if output.status.success() {
        log::info!("Successfully synced to remote repository");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("Git push failed: {}", stderr);
        anyhow::bail!("Git push failed with exit code: {:?}", output.status.code());
    }
}

/// Get current commit hash
pub fn get_current_commit() -> Result<String> {
    let output = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .with_context(|| "Failed to get current commit hash")?;
    
    if output.status.success() {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(commit)
    } else {
        anyhow::bail!("Failed to get current commit hash");
    }
}

/// Get short commit hash (first 7 characters)
pub fn get_short_commit() -> Result<String> {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .with_context(|| "Failed to get short commit hash")?;
    
    if output.status.success() {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(commit)
    } else {
        anyhow::bail!("Failed to get short commit hash");
    }
}
