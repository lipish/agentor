#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    
    #[test]
    fn test_basic_integration() {
        // This is a placeholder for integration tests
        // Real integration tests would require setting up a git repository
        // and testing the full workflow
        
        // For now, just verify that basic components can be imported
        use agentor::config::Config;
        use agentor::rollback;
        
        let config = Config::default();
        assert!(!config.build.command.is_empty());
        
        // Test creating agentor directory
        let test_dir = "/tmp/agentor_test";
        if Path::new(test_dir).exists() {
            fs::remove_dir_all(test_dir).ok();
        }
        fs::create_dir(test_dir).unwrap();
        
        rollback::init_agentor_dir(test_dir).unwrap();
        
        let agentor_path = Path::new(test_dir).join(".agentor");
        assert!(agentor_path.exists());
        
        // Cleanup
        fs::remove_dir_all(test_dir).unwrap();
    }
}
