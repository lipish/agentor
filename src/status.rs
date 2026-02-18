use anyhow::Result;
use crate::rollback::load_state;

/// Display deployment status and history
pub fn show_status() -> Result<()> {
    let state = load_state(".")?;
    
    println!("=== Agentor Deployment Status ===\n");
    
    if let Some(current) = &state.current_version {
        println!("Current Version: {}", current);
    } else {
        println!("Current Version: None");
    }
    
    println!("\nDeployment History ({} versions):", state.versions.len());
    
    if state.versions.is_empty() {
        println!("  No deployment history available");
    } else {
        for (i, version) in state.versions.iter().rev().enumerate() {
            let is_current = state.current_version.as_ref().map_or(false, |c| c == &version.commit);
            let marker = if is_current { " (current)" } else { "" };
            
            println!("\n{}. Commit: {}{}", i + 1, version.commit, marker);
            println!("   Time: {}", version.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("   Artifacts: {}", version.artifacts.join(", "));
        }
    }
    
    println!();
    Ok(())
}
