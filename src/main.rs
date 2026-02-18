use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::Path;

use agentor::{config, hook, logger, rollback, runner, status};

#[derive(Parser)]
#[command(name = "agentor")]
#[command(about = "Local Git auto-deploy tool - 本地 Git 自动部署工具", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize agentor in the current Git repository
    Init {
        /// Path to the Git repository (defaults to current directory)
        #[arg(short, long, default_value = ".")]
        path: String,
    },
    /// Run the complete deployment workflow manually
    Run {
        /// Path to config file
        #[arg(short, long, default_value = "deploy.toml")]
        config: String,
    },
    /// Rollback to the previous version
    Rollback {
        /// Path to config file
        #[arg(short, long, default_value = "deploy.toml")]
        config: String,
    },
    /// Show deployment status and history
    Status,
    /// Show recent deployment logs
    Log {
        /// Number of log entries to display
        #[arg(short, long, default_value = "10")]
        count: usize,
        
        /// Path to log file
        #[arg(short = 'f', long, default_value = "agentor.log")]
        file: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Init { path } => {
            init_command(&path)?;
        }
        Commands::Run { config } => {
            run_command(&config)?;
        }
        Commands::Rollback { config } => {
            rollback_command(&config)?;
        }
        Commands::Status => {
            status_command()?;
        }
        Commands::Log { count, file } => {
            log_command(count, &file)?;
        }
    }
    
    Ok(())
}

/// Initialize agentor in a Git repository
fn init_command(repo_path: &str) -> Result<()> {
    println!("Initializing agentor in: {}", repo_path);
    
    // Check if it's a Git repository
    let git_dir = Path::new(repo_path).join(".git");
    if !git_dir.exists() {
        anyhow::bail!("Not a Git repository: {}", repo_path);
    }
    
    // Initialize agentor directory structure
    rollback::init_agentor_dir(repo_path)
        .with_context(|| "Failed to create agentor directory")?;
    
    // Install post-commit hook
    hook::install_hook(repo_path)
        .with_context(|| "Failed to install post-commit hook")?;
    
    // Create default config file if it doesn't exist
    let config_path = Path::new(repo_path).join("deploy.toml");
    if !config_path.exists() {
        let default_config = config::Config::default();
        default_config.save(&config_path)
            .with_context(|| "Failed to create config file")?;
        println!("Created default config file: {}", config_path.display());
    } else {
        println!("Config file already exists: {}", config_path.display());
    }
    
    println!("\n✅ Agentor initialized successfully!");
    println!("\nNext steps:");
    println!("  1. Edit deploy.toml to configure your deployment");
    println!("  2. Make a git commit to trigger automatic deployment");
    println!("  3. Or run 'agentor run' to deploy manually");
    
    Ok(())
}

/// Run the deployment workflow
fn run_command(config_path: &str) -> Result<()> {
    // Load configuration
    let cfg = config::Config::load(config_path)
        .with_context(|| format!("Failed to load config from: {}", config_path))?;
    
    // Initialize logger
    logger::init(&cfg.log.file, &cfg.log.level)
        .with_context(|| "Failed to initialize logger")?;
    
    // Run deployment workflow
    runner::run(&cfg)?;
    
    Ok(())
}

/// Rollback to previous version
fn rollback_command(config_path: &str) -> Result<()> {
    // Load configuration
    let cfg = config::Config::load(config_path)
        .with_context(|| format!("Failed to load config from: {}", config_path))?;
    
    // Initialize logger
    logger::init(&cfg.log.file, &cfg.log.level)
        .with_context(|| "Failed to initialize logger")?;
    
    if !cfg.rollback.enabled {
        anyhow::bail!("Rollback is not enabled in configuration");
    }
    
    // Perform rollback
    rollback::rollback(".", &cfg.deploy.target_dir)?;
    
    println!("✅ Rollback completed successfully");
    
    Ok(())
}

/// Show deployment status
fn status_command() -> Result<()> {
    status::show_status()?;
    Ok(())
}

/// Show recent logs
fn log_command(count: usize, log_file: &str) -> Result<()> {
    println!("=== Recent Deployment Logs ===\n");
    
    if !Path::new(log_file).exists() {
        println!("Log file not found: {}", log_file);
        return Ok(());
    }
    
    let logs = logger::read_recent_logs(log_file, count)
        .with_context(|| "Failed to read log file")?;
    
    if logs.is_empty() {
        println!("No logs available");
    } else {
        for log in logs {
            println!("{}", log);
        }
    }
    
    Ok(())
}
