use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// Deploy using the configured deployment method
pub fn deploy(command: &str, target_dir: &str, artifacts: &[String]) -> Result<()> {
    log::info!("Starting deployment...");
    
    // File deployment: copy artifacts to target directory
    if !target_dir.is_empty() && !artifacts.is_empty() {
        deploy_files(target_dir, artifacts)?;
    }
    
    // Process deployment: execute deployment command
    if !command.is_empty() {
        deploy_process(command)?;
    }
    
    if command.is_empty() && (target_dir.is_empty() || artifacts.is_empty()) {
        log::warn!("No deployment method configured (neither command nor file deployment)");
    }
    
    log::info!("Deployment completed successfully");
    Ok(())
}

/// Deploy by copying files to target directory
fn deploy_files(target_dir: &str, artifacts: &[String]) -> Result<()> {
    log::info!("Deploying files to: {}", target_dir);
    
    // Create target directory if it doesn't exist
    let target_path = Path::new(target_dir);
    if !target_path.exists() {
        fs::create_dir_all(target_path)
            .with_context(|| format!("Failed to create target directory: {}", target_dir))?;
    }
    
    // Copy each artifact
    for artifact in artifacts {
        let source = Path::new(artifact);
        if !source.exists() {
            anyhow::bail!("Artifact not found: {}", artifact);
        }
        
        let file_name = source.file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid artifact path: {}", artifact))?;
        let dest = target_path.join(file_name);
        
        log::info!("Copying {} to {}", artifact, dest.display());
        fs::copy(source, &dest)
            .with_context(|| format!("Failed to copy {} to {}", artifact, dest.display()))?;
    }
    
    Ok(())
}

/// Deploy by executing a command
fn deploy_process(command: &str) -> Result<()> {
    log::info!("Executing deployment command: {}", command);
    
    // Parse command and arguments
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Deployment command is empty");
    }
    
    let (cmd, args) = parts.split_at(1);
    
    // Execute deployment command
    let output = Command::new(cmd[0])
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute deployment command: {}", command))?;
    
    if output.status.success() {
        log::info!("Deployment command executed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("Deployment command failed: {}", stderr);
        anyhow::bail!("Deployment command failed with exit code: {:?}", output.status.code());
    }
}
