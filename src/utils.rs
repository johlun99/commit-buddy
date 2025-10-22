use anyhow::Result;
use std::process::Command;

pub fn run_git_command(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", error);
    }
    
    Ok(String::from_utf8(output.stdout)?)
}

pub fn get_current_branch() -> Result<String> {
    let output = run_git_command(&["branch", "--show-current"])?;
    Ok(output.trim().to_string())
}

pub fn get_git_status() -> Result<String> {
    run_git_command(&["status", "--porcelain"])
}

pub fn is_git_repository() -> bool {
    run_git_command(&["rev-parse", "--git-dir"]).is_ok()
}

pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.1} {}", size, UNITS[unit_index])
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

pub fn extract_commit_type(message: &str) -> Option<&str> {
    let message = message.trim();
    if let Some(colon_pos) = message.find(':') {
        let prefix = &message[..colon_pos];
        if prefix.len() <= 20 && prefix.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            return Some(prefix);
        }
    }
    None
}

pub fn is_conventional_commit(message: &str) -> bool {
    extract_commit_type(message).is_some()
}

pub fn get_commit_emoji(commit_type: &str) -> &str {
    match commit_type {
        "feat" => "✨",
        "fix" => "🐛",
        "docs" => "📚",
        "style" => "💄",
        "refactor" => "♻️",
        "test" => "🧪",
        "chore" => "🔧",
        "perf" => "⚡",
        "ci" => "👷",
        "build" => "📦",
        "revert" => "⏪",
        _ => "📝",
    }
}
