use clap::{Parser, Subcommand};
use anyhow::Result;

mod git;
mod ai;
mod github;
mod utils;

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
        /// Base branch to compare against (default: main)
        #[arg(short, long, default_value = "main")]
        base: String,
        /// Output format (markdown, json)
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Generate unit tests for changed code
    GenerateTests {
        /// Base branch to compare against (default: main)
        #[arg(short, long, default_value = "main")]
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
        /// Base branch to compare against (default: main)
        #[arg(short, long, default_value = "main")]
        base: String,
        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Code review assistance
    Review {
        /// Base branch to compare against (default: main)
        #[arg(short, long, default_value = "main")]
        base: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::PrDescription { base, format } => {
            git::generate_pr_description(&base, &format).await?;
        }
        Commands::GenerateTests { base, framework } => {
            git::generate_tests(&base, &framework).await?;
        }
        Commands::ImproveCommit { commit } => {
            git::improve_commit_message(commit.as_deref()).await?;
        }
        Commands::Commit { all } => {
            git::interactive_commit(all).await?;
        }
        Commands::Changelog { base, output } => {
            git::generate_changelog(&base, output.as_deref()).await?;
        }
        Commands::Review { base } => {
            git::code_review(&base).await?;
        }
    }

    Ok(())
}
