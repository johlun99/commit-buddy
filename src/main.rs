use clap::{Parser, Subcommand};
use anyhow::Result;

mod git;
mod ai;
mod github;
mod utils;
mod config;

#[derive(Parser)]
#[command(name = "commit-buddy")]
#[command(about = "AI-powered git companion for enhanced development workflow")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate AI-powered PR description from commits
    PrDescription {
        /// Base branch to compare against (default: master)
        #[arg(short, long, default_value = "master")]
        base: String,
        /// Output format (markdown, json)
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Generate unit tests for changed code
    GenerateTests {
        /// Base branch to compare against (default: master)
        #[arg(short, long, default_value = "master")]
        base: String,
        /// Test framework to use (jest, pytest, etc.)
        #[arg(short, long, default_value = "auto")]
        framework: String,
    },
    /// Improve commit messages with AI suggestions
    ImproveCommit {
        /// Commit hash to improve (default: HEAD)
        #[arg(short, long)]
        commit: Option<String>,
    },
    /// Interactive commit message assistant
    Commit {
        /// Stage all changes before committing
        #[arg(short, long)]
        all: bool,
    },
    /// Generate changelog from commits
    Changelog {
        /// Base branch to compare against (default: master)
        #[arg(short, long, default_value = "master")]
        base: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Code review assistance
    Review {
        /// Base branch to compare against (default: master)
        #[arg(short, long, default_value = "master")]
        base: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::load()?;

    match cli.command {
        Commands::PrDescription { base, format } => {
            let effective_base = if base == "master" { 
                config.get_default_branch() 
            } else { 
                &base 
            };
            git::generate_pr_description(effective_base, &format).await?;
        }
        Commands::GenerateTests { base, framework } => {
            let effective_base = if base == "master" { 
                config.get_default_branch() 
            } else { 
                &base 
            };
            git::generate_tests(effective_base, &framework).await?;
        }
        Commands::ImproveCommit { commit } => {
            git::improve_commit_message(commit.as_deref()).await?;
        }
        Commands::Commit { all } => {
            git::interactive_commit(all).await?;
        }
        Commands::Changelog { base, output } => {
            let effective_base = if base == "master" { 
                config.get_default_branch() 
            } else { 
                &base 
            };
            git::generate_changelog(effective_base, output.as_deref()).await?;
        }
        Commands::Review { base } => {
            let effective_base = if base == "master" { 
                config.get_default_branch() 
            } else { 
                &base 
            };
            git::code_review(effective_base).await?;
        }
    }

    Ok(())
}
