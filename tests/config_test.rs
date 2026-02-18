#[cfg(test)]
mod tests {
    use agentor::config::Config;
    use std::fs;
    
    #[test]
    fn test_load_config() {
        let config_content = r#"
[watch]
repo_path = "."
branch = "main"

[build]
command = "cargo build --release"

[deploy]
command = "systemctl restart app.service"
target_dir = "/opt/deploy"
artifacts = ["target/release/my-app"]

[sync]
enabled = true
remote = "origin"
branch = "main"

[rollback]
enabled = true
keep_versions = 3

[log]
file = "agentor.log"
level = "info"
"#;
        
        let test_file = "/tmp/test_config.toml";
        fs::write(test_file, config_content).unwrap();
        
        let config = Config::load(test_file).unwrap();
        
        assert_eq!(config.watch.repo_path, ".");
        assert_eq!(config.watch.branch, "main");
        assert_eq!(config.build.command, "cargo build --release");
        assert_eq!(config.deploy.command, "systemctl restart app.service");
        assert_eq!(config.deploy.target_dir, "/opt/deploy");
        assert_eq!(config.deploy.artifacts.len(), 1);
        assert!(config.sync.enabled);
        assert_eq!(config.sync.remote, "origin");
        assert_eq!(config.rollback.keep_versions, 3);
        
        fs::remove_file(test_file).unwrap();
    }
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        
        assert_eq!(config.watch.repo_path, ".");
        assert_eq!(config.watch.branch, "main");
        assert!(!config.build.command.is_empty());
        assert!(config.sync.enabled);
        assert_eq!(config.sync.remote, "origin");
        assert_eq!(config.rollback.keep_versions, 3);
    }
    
    #[test]
    fn test_save_and_load_config() {
        let test_file = "/tmp/test_save_config.toml";
        
        let config = Config::default();
        config.save(test_file).unwrap();
        
        let loaded = Config::load(test_file).unwrap();
        
        assert_eq!(config.watch.repo_path, loaded.watch.repo_path);
        assert_eq!(config.build.command, loaded.build.command);
        
        fs::remove_file(test_file).unwrap();
    }
}
