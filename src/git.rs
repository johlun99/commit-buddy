use anyhow::{Context, Result};
use git2::{Repository, Diff, DiffFormat};
use serde::{Deserialize, Serialize};
use crate::ai;
use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub files_changed: Vec<String>,
    pub diff: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffInfo {
    pub commits: Vec<CommitInfo>,
    pub total_files_changed: usize,
    pub total_additions: i32,
    pub total_deletions: i32,
}

pub async fn generate_pr_description(base: &str, format: &str, config: &Config) -> Result<()> {
    println!("🔍 Analyzing commits since {}...", base);
    
    let diff_info = get_diff_info(base)?;
    
    if diff_info.commits.is_empty() {
        println!("No commits found to analyze.");
        return Ok(());
    }

    println!("📝 Generating AI-powered PR description...");
    let description = ai::generate_pr_description(&diff_info, config).await?;
    
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&description)?;
            println!("{}", json);
        }
        "markdown" | _ => {
            println!("\n{}", description);
        }
    }
    
    Ok(())
}

pub async fn generate_tests(base: &str, framework: &str, config: &Config) -> Result<()> {
    println!("🔍 Analyzing code changes since {}...", base);
    
    let diff_info = get_diff_info(base)?;
    
    if diff_info.commits.is_empty() {
        println!("No commits found to analyze.");
        return Ok(());
    }

    println!("🧪 Generating unit tests...");
    let tests = ai::generate_tests(&diff_info, framework, config).await?;
    
    println!("\n{}", tests);
    Ok(())
}

pub async fn improve_commit_message(commit_hash: Option<&str>, config: &Config) -> Result<()> {
    let repo = Repository::open(".")?;
    let commit_hash = commit_hash.unwrap_or("HEAD");
    
    let commit_obj = repo.revparse_single(commit_hash)?;
    let commit = commit_obj.as_commit()
        .context("Could not find commit")?;
    
    let message = commit.message().unwrap_or("No message").to_string();
    let author = commit.author().name().unwrap_or("Unknown").to_string();
    
    println!("📝 Analyzing commit: {}", commit_hash);
    println!("Current message: {}", message);
    println!("Author: {}", author);
    
    let improved_message = ai::improve_commit_message(&message, config).await?;
    
    println!("\n💡 Suggested improved message:");
    println!("{}", improved_message);
    
    Ok(())
}

pub async fn interactive_commit(all: bool, config: &Config) -> Result<()> {
    let repo = Repository::open(".")?;
    
    if all {
        println!("📁 Staging all changes...");
        // Stage all changes
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;
    }
    
    // Get staged changes
    let diff_info = get_staged_changes()?;
    
    if diff_info.commits.is_empty() {
        println!("No staged changes found.");
        return Ok(());
    }
    
    println!("🤖 Generating commit message suggestions...");
    let suggestions = ai::generate_commit_suggestions(&diff_info, config).await?;
    
    println!("\n💡 Suggested commit messages:");
    for (i, suggestion) in suggestions.iter().enumerate() {
        println!("{}. {}", i + 1, suggestion);
    }
    
    // In a real implementation, you'd use a library like dialoguer for interactive selection
    println!("\n✨ Use one of these suggestions with: git commit -m \"your message\"");
    
    Ok(())
}

pub async fn generate_changelog(base: &str, output: Option<&str>, config: &Config) -> Result<()> {
    println!("📋 Generating changelog since {}...", base);
    
    let diff_info = get_diff_info(base)?;
    
    if diff_info.commits.is_empty() {
        println!("No commits found to analyze.");
        return Ok(());
    }

    let changelog = ai::generate_changelog(&diff_info, config).await?;
    
    match output {
        Some(file_path) => {
            std::fs::write(file_path, &changelog)?;
            println!("✅ Changelog written to {}", file_path);
        }
        None => {
            println!("\n{}", changelog);
        }
    }
    
    Ok(())
}

pub async fn code_review(base: &str, config: &Config) -> Result<()> {
    println!("🔍 Performing AI code review since {}...", base);
    
    let diff_info = get_diff_info(base)?;
    
    if diff_info.commits.is_empty() {
        println!("No commits found to review.");
        return Ok(());
    }

    let review = ai::code_review(&diff_info, config).await?;
    
    println!("\n{}", review);
    Ok(())
}

fn get_diff_info(base: &str) -> Result<DiffInfo> {
    let repo = Repository::open(".")?;
    let head = repo.head()?.peel_to_commit()?;
    let base_obj = repo.revparse_single(base)?;
    let base_commit = base_obj.as_commit()
        .context("Could not find base commit")?;
    
    let mut commits = Vec::new();
    let mut walk = repo.revwalk()?;
    walk.push(head.id())?;
    walk.hide(base_commit.id())?;
    
    for commit_id in walk {
        let commit_id = commit_id?;
        let commit = repo.find_commit(commit_id)?;
        
        let message = commit.message().unwrap_or("No message").to_string();
        let author = commit.author().name().unwrap_or("Unknown").to_string();
        let date = commit.time().seconds().to_string();
        
        // Get diff for this commit
        let diff = get_commit_diff(&repo, &commit)?;
        let files_changed = get_files_changed(&diff);
        
        commits.push(CommitInfo {
            hash: commit_id.to_string(),
            message,
            author,
            date,
            files_changed,
            diff,
        });
    }
    
    // Calculate totals before moving commits
    let total_files_changed = commits.iter()
        .flat_map(|c| &c.files_changed)
        .collect::<std::collections::HashSet<_>>()
        .len();
    
    Ok(DiffInfo {
        commits,
        total_files_changed,
        total_additions: 0, // Would need more complex diff analysis
        total_deletions: 0, // Would need more complex diff analysis
    })
}

fn get_staged_changes() -> Result<DiffInfo> {
    let repo = Repository::open(".")?;
    
    let mut commits = Vec::new();
    
    // Get staged changes
    let diff = repo.diff_index_to_workdir(None, None)?;
    let diff_str = format_diff(&diff)?;
    let files_changed = get_files_changed(&diff_str);
    
    if !files_changed.is_empty() {
        commits.push(CommitInfo {
            hash: "STAGED".to_string(),
            message: "Staged changes".to_string(),
            author: "Current user".to_string(),
            date: chrono::Utc::now().to_rfc3339(),
            files_changed,
            diff: diff_str,
        });
    }
    
    // Calculate totals before moving commits
    let total_files_changed = commits.iter()
        .flat_map(|c| &c.files_changed)
        .collect::<std::collections::HashSet<_>>()
        .len();
    
    Ok(DiffInfo {
        commits,
        total_files_changed,
        total_additions: 0,
        total_deletions: 0,
    })
}

fn get_commit_diff(repo: &Repository, commit: &git2::Commit) -> Result<String> {
    let tree = commit.tree()?;
    let parent = if commit.parent_count() > 0 {
        Some(commit.parent(0)?.tree()?)
    } else {
        None
    };
    
    let diff = repo.diff_tree_to_tree(parent.as_ref(), Some(&tree), None)?;
    format_diff(&diff)
}

fn format_diff(diff: &Diff) -> Result<String> {
    let mut output = Vec::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let content = std::str::from_utf8(line.content()).unwrap_or("");
        output.push(content.to_string());
        true
    })?;
    
    Ok(output.join(""))
}

fn get_files_changed(diff: &str) -> Vec<String> {
    diff.lines()
        .filter(|line| line.starts_with("diff --git") || line.starts_with("+++") || line.starts_with("---"))
        .filter_map(|line| {
            if line.starts_with("diff --git") {
                line.split_whitespace().nth(2).map(|s| s.trim_start_matches("a/").to_string())
            } else if line.starts_with("+++") || line.starts_with("---") {
                let path = line.trim_start_matches("+++ ").trim_start_matches("--- ");
                if !path.starts_with("/dev/null") {
                    Some(path.trim_start_matches("a/").trim_start_matches("b/").to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}
