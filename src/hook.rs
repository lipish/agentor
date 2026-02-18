use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Install post-commit hook in the given Git repository
pub fn install_hook<P: AsRef<Path>>(repo_path: P) -> Result<()> {
    let hooks_dir = repo_path.as_ref().join(".git").join("hooks");
    
    if !hooks_dir.exists() {
        anyhow::bail!("Not a valid Git repository: .git/hooks directory not found");
    }
    
    let hook_path = hooks_dir.join("post-commit");
    
    // Generate hook script content (cross-platform compatible)
    let hook_content = generate_hook_script();
    
    // Write hook script
    fs::write(&hook_path, hook_content)
        .with_context(|| format!("Failed to write hook file: {}", hook_path.display()))?;
    
    // Set executable permission on Unix-like systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }
    
    log::info!("Post-commit hook installed successfully at: {}", hook_path.display());
    
    Ok(())
}

/// Generate hook script content
fn generate_hook_script() -> String {
    // Use sh for Unix, but check for Windows
    #[cfg(windows)]
    {
        r#"#!/bin/sh
# Agentor post-commit hook
agentor run
"#.to_string()
    }
    
    #[cfg(not(windows))]
    {
        r#"#!/bin/sh
# Agentor post-commit hook
agentor run
"#.to_string()
    }
}

/// Check if hook is already installed
pub fn is_hook_installed<P: AsRef<Path>>(repo_path: P) -> bool {
    let hook_path = repo_path.as_ref().join(".git").join("hooks").join("post-commit");
    hook_path.exists()
}

/// Uninstall post-commit hook
pub fn uninstall_hook<P: AsRef<Path>>(repo_path: P) -> Result<()> {
    let hook_path = repo_path.as_ref().join(".git").join("hooks").join("post-commit");
    
    if hook_path.exists() {
        fs::remove_file(&hook_path)
            .with_context(|| format!("Failed to remove hook file: {}", hook_path.display()))?;
        log::info!("Post-commit hook uninstalled successfully");
    } else {
        log::warn!("Hook file does not exist, nothing to uninstall");
    }
    
    Ok(())
}
