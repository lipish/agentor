use anyhow::Result;
use crate::config::Config;
use crate::{builder, deployer, rollback, syncer};

/// Run the complete deployment workflow: build -> deploy -> sync
pub fn run(config: &Config) -> Result<()> {
    log::info!("=== Starting Agentor deployment workflow ===");
    
    let start_time = std::time::Instant::now();
    
    // Get current commit for logging
    let commit = syncer::get_short_commit().unwrap_or_else(|_| "unknown".to_string());
    log::info!("Current commit: {}", commit);
    
    // Step 1: Build
    if let Err(e) = builder::build(&config.build.command) {
        log::error!("Build failed: {}", e);
        
        // Attempt rollback if enabled
        if config.rollback.enabled && !config.deploy.target_dir.is_empty() {
            log::warn!("Attempting to rollback due to build failure...");
            if let Err(rollback_err) = rollback::rollback(".", &config.deploy.target_dir) {
                log::error!("Rollback failed: {}", rollback_err);
            }
        }
        
        return Err(e);
    }
    
    // Verify artifacts exist
    if !config.deploy.artifacts.is_empty() {
        if let Err(e) = builder::verify_artifacts(&config.deploy.artifacts) {
            log::error!("Artifact verification failed: {}", e);
            return Err(e);
        }
    }
    
    // Step 2: Backup current version (before deployment)
    if config.rollback.enabled && !config.deploy.artifacts.is_empty() {
        let full_commit = syncer::get_current_commit().unwrap_or_else(|_| commit.clone());
        if let Err(e) = rollback::backup_version(
            ".",
            &full_commit,
            &config.deploy.artifacts,
            config.rollback.keep_versions,
        ) {
            log::warn!("Failed to backup version: {}", e);
        }
    }
    
    // Step 3: Deploy
    if let Err(e) = deployer::deploy(
        &config.deploy.command,
        &config.deploy.target_dir,
        &config.deploy.artifacts,
    ) {
        log::error!("Deployment failed: {}", e);
        
        // Attempt rollback if enabled
        if config.rollback.enabled && !config.deploy.target_dir.is_empty() {
            log::warn!("Attempting to rollback due to deployment failure...");
            if let Err(rollback_err) = rollback::rollback(".", &config.deploy.target_dir) {
                log::error!("Rollback failed: {}", rollback_err);
            }
        }
        
        return Err(e);
    }
    
    // Step 4: Sync to remote (if enabled)
    if config.sync.enabled {
        if let Err(e) = syncer::sync(&config.sync.remote, &config.sync.branch) {
            log::error!("Sync to remote failed: {}", e);
            log::warn!("Local deployment succeeded but sync failed");
            // Don't return error - local deployment is more important
        }
    }
    
    let duration = start_time.elapsed();
    log::info!("=== Deployment workflow completed successfully in {:.2}s ===", duration.as_secs_f64());
    
    Ok(())
}
