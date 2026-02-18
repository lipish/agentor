use anyhow::{Context, Result};
use std::process::Command;

/// Execute build command
pub fn build(command: &str) -> Result<()> {
    log::info!("Starting build process...");
    log::info!("Build command: {}", command);
    
    let start = std::time::Instant::now();
    
    // Parse command and arguments
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Build command is empty");
    }
    
    let (cmd, args) = parts.split_at(1);
    
    // Execute build command
    let output = Command::new(cmd[0])
        .args(args)
        .output()
        .with_context(|| format!("Failed to execute build command: {}", command))?;
    
    let duration = start.elapsed();
    
    if output.status.success() {
        log::info!("Build completed successfully in {:.2}s", duration.as_secs_f64());
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("Build failed: {}", stderr);
        anyhow::bail!("Build command failed with exit code: {:?}", output.status.code());
    }
}

/// Verify that build artifacts exist
pub fn verify_artifacts(artifacts: &[String]) -> Result<()> {
    for artifact in artifacts {
        if !std::path::Path::new(artifact).exists() {
            anyhow::bail!("Build artifact not found: {}", artifact);
        }
    }
    Ok(())
}
