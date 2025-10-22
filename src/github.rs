use anyhow::{Result, Context};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubConfig {
    pub token: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub title: String,
    pub body: String,
    pub head: String,
    pub base: String,
}

pub async fn create_pull_request(
    config: &GitHubConfig,
    pr: &PullRequest,
) -> Result<String> {
    let octocrab = Octocrab::builder()
        .personal_token(config.token.clone())
        .build()?;
    
    let response = octocrab
        .pulls(&config.owner, &config.repo)
        .create(&pr.title, &pr.head, &pr.base)
        .body(&pr.body)
        .send()
        .await?;
    
    Ok(response.html_url.map(|url| url.to_string()).unwrap_or_default())
}

pub async fn get_repository_info(config: &GitHubConfig) -> Result<RepositoryInfo> {
    let octocrab = Octocrab::builder()
        .personal_token(config.token.clone())
        .build()?;
    
    let repo = octocrab
        .repos(&config.owner, &config.repo)
        .get()
        .await?;
    
    Ok(RepositoryInfo {
        name: repo.name,
        description: repo.description.unwrap_or_default(),
        language: repo.language.map(|l| l.to_string()).unwrap_or_default(),
        stars: repo.stargazers_count.unwrap_or(0),
        forks: repo.forks_count.unwrap_or(0),
        open_issues: repo.open_issues_count.unwrap_or(0),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub name: String,
    pub description: String,
    pub language: String,
    pub stars: u32,
    pub forks: u32,
    pub open_issues: u32,
}

pub fn load_github_config() -> Result<GitHubConfig> {
    // Try to load from environment variables
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .context("GitHub token not found. Set GITHUB_TOKEN or GH_TOKEN environment variable")?;
    
    // Try to get repo info from git remote
    let repo_info = get_git_repo_info()?;
    
    Ok(GitHubConfig {
        token,
        owner: repo_info.owner,
        repo: repo_info.name,
    })
}

#[derive(Debug)]
struct GitRepoInfo {
    owner: String,
    name: String,
}

fn get_git_repo_info() -> Result<GitRepoInfo> {
    use std::process::Command;
    
    let output = Command::new("git")
        .args(&["remote", "get-url", "origin"])
        .output()?;
    
    let url = String::from_utf8(output.stdout)?;
    
    // Parse GitHub URL (supports both HTTPS and SSH formats)
    let url = url.trim();
    let (owner, name) = if url.starts_with("https://github.com/") {
        let path = url.trim_start_matches("https://github.com/").trim_end_matches(".git");
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            anyhow::bail!("Invalid GitHub URL format");
        }
    } else if url.starts_with("git@github.com:") {
        let path = url.trim_start_matches("git@github.com:").trim_end_matches(".git");
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            anyhow::bail!("Invalid GitHub URL format");
        }
    } else {
        anyhow::bail!("Not a GitHub repository");
    };
    
    Ok(GitRepoInfo { owner, name })
}
