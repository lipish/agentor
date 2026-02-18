use anyhow::{Context, Result};
use log::LevelFilter;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Initialize the logging system
pub fn init(_log_file: &str, log_level: &str) -> Result<()> {
    let level = parse_log_level(log_level);
    
    env_logger::Builder::new()
        .filter_level(level)
        .format_timestamp_secs()
        .init();
    
    Ok(())
}

/// Parse log level string to LevelFilter
fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    }
}

/// Append a log entry to the log file
pub fn append_log_entry<P: AsRef<Path>>(log_file: P, entry: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.as_ref().display()))?;
    
    writeln!(file, "{}", entry)
        .with_context(|| "Failed to write log entry")?;
    
    Ok(())
}

/// Read recent log entries from the log file
pub fn read_recent_logs<P: AsRef<Path>>(log_file: P, count: usize) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(&log_file)
        .with_context(|| format!("Failed to read log file: {}", log_file.as_ref().display()))?;
    
    let lines: Vec<String> = content.lines()
        .rev()
        .take(count)
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    
    Ok(lines)
}
