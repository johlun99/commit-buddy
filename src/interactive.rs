use anyhow::Result;
use crate::config::Config;
use crate::git;
use crate::ai;
use std::io::{self, Write};
use std::process::Command;

pub struct InteractiveCli {
    pub config: Config,
    pub current_branch: String,
    pub status: String,
}

impl InteractiveCli {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            current_branch: "unknown".to_string(),
            status: "unknown".to_string(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.update_status().await?;
        
        loop {
            self.clear_screen();
            self.display_header();
            self.display_status();
            self.display_menu();
            
            match self.get_user_input() {
                Ok(choice) => {
                    if let Err(e) = self.handle_choice(choice).await {
                        self.show_error(&e.to_string());
                    }
                }
                Err(_) => {
                    println!("Invalid input. Please try again.");
                    self.pause();
                }
            }
        }
    }

    fn clear_screen(&self) {
        print!("\x1B[2J\x1B[1;1H");
    }

    fn display_header(&self) {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                    🤖 COMMIT BUDDY 🤖                        ║");
        println!("║              AI-Powered Git Companion                        ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();
    }

    fn display_status(&self) {
        println!("📊 Repository Status:");
        println!("   Branch: {}", self.current_branch);
        println!("   Status: {}", self.status);
        println!("   AI Features: {}", if self.config.has_openai_key() { "✅ Enabled" } else { "❌ Disabled" });
        println!();
    }

    fn display_menu(&self) {
        println!("🎯 Git Operations:");
        println!("   1. 📝 Add files to staging");
        println!("   2. 💾 Commit changes");
        println!("   3. 🚀 Push to remote");
        println!("   4. 📥 Pull from remote");
        println!("   5. 🌿 Switch branch");
        println!("   6. 🔀 Merge branch");
        println!("   7. 📋 View status");
        println!();
        println!("🤖 AI Features:");
        println!("   8. ✨ Generate PR description");
        println!("   9. 🧪 Generate unit tests");
        println!("  10. 💬 Improve commit message");
        println!("  11. 📝 Interactive commit");
        println!("  12. 📋 Generate changelog");
        println!("  13. 🔍 Code review");
        println!();
        println!("⚙️  Utilities:");
        println!("  14. 🔄 Refresh status");
        println!("  15. ⚙️  Configuration");
        println!("  16. ❌ Exit");
        println!();
        print!("Enter your choice (1-16): ");
        io::stdout().flush().unwrap();
    }

    fn get_user_input(&self) -> Result<u32> {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().parse::<u32>().map_err(|_| anyhow::anyhow!("Invalid input"))
    }

    async fn handle_choice(&mut self, choice: u32) -> Result<()> {
        match choice {
            1 => self.add_files().await,
            2 => self.commit_changes().await,
            3 => self.push_changes().await,
            4 => self.pull_changes().await,
            5 => self.switch_branch().await,
            6 => self.merge_branch().await,
            7 => self.view_status().await,
            8 => self.generate_pr_description().await,
            9 => self.generate_tests().await,
            10 => self.improve_commit_message().await,
            11 => self.interactive_commit().await,
            12 => self.generate_changelog().await,
            13 => self.code_review().await,
            14 => self.refresh_status().await,
            15 => self.show_configuration(),
            16 => self.exit(),
            _ => {
                println!("Invalid choice. Please select 1-16.");
                self.pause();
                Ok(())
            }
        }
    }

    async fn add_files(&mut self) -> Result<()> {
        println!("📝 Add files to staging:");
        println!("   1. Add all files");
        println!("   2. Add specific file");
        println!("   3. Add with pattern");
        print!("Choose option (1-3): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => {
                self.run_git_command(&["add", "."])?;
                println!("✅ All files added to staging");
            }
            "2" => {
                print!("Enter file path: ");
                io::stdout().flush().unwrap();
                let mut file_path = String::new();
                io::stdin().read_line(&mut file_path)?;
                self.run_git_command(&["add", file_path.trim()])?;
                println!("✅ File added to staging");
            }
            "3" => {
                print!("Enter pattern (e.g., *.rs): ");
                io::stdout().flush().unwrap();
                let mut pattern = String::new();
                io::stdin().read_line(&mut pattern)?;
                self.run_git_command(&["add", pattern.trim()])?;
                println!("✅ Files matching pattern added to staging");
            }
            _ => println!("❌ Invalid option"),
        }

        self.pause();
        Ok(())
    }

    async fn commit_changes(&mut self) -> Result<()> {
        println!("💾 Commit changes:");
        print!("Enter commit message: ");
        io::stdout().flush().unwrap();

        let mut message = String::new();
        io::stdin().read_line(&mut message)?;
        let message = message.trim();

        if message.is_empty() {
            println!("❌ Commit message cannot be empty");
            self.pause();
            return Ok(());
        }

        self.run_git_command(&["commit", "-m", message])?;
        println!("✅ Changes committed successfully");
        self.pause();
        Ok(())
    }

    async fn push_changes(&mut self) -> Result<()> {
        println!("🚀 Push changes to remote:");
        println!("   1. Push current branch");
        println!("   2. Push all branches");
        println!("   3. Push with upstream");
        print!("Choose option (1-3): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => {
                self.run_git_command(&["push"])?;
                println!("✅ Current branch pushed successfully");
            }
            "2" => {
                self.run_git_command(&["push", "--all"])?;
                println!("✅ All branches pushed successfully");
            }
            "3" => {
                self.run_git_command(&["push", "-u", "origin", &self.current_branch])?;
                println!("✅ Branch pushed with upstream set");
            }
            _ => println!("❌ Invalid option"),
        }

        self.pause();
        Ok(())
    }

    async fn pull_changes(&mut self) -> Result<()> {
        println!("📥 Pull changes from remote:");
        self.run_git_command(&["pull"])?;
        println!("✅ Changes pulled successfully");
        self.pause();
        Ok(())
    }

    async fn switch_branch(&mut self) -> Result<()> {
        println!("🌿 Switch branch:");
        println!("   1. Switch to existing branch");
        println!("   2. Create and switch to new branch");
        print!("Choose option (1-2): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let choice = input.trim();

        match choice {
            "1" => {
                print!("Enter branch name: ");
                io::stdout().flush().unwrap();
                let mut branch = String::new();
                io::stdin().read_line(&mut branch)?;
                self.run_git_command(&["checkout", branch.trim()])?;
                println!("✅ Switched to branch: {}", branch.trim());
            }
            "2" => {
                print!("Enter new branch name: ");
                io::stdout().flush().unwrap();
                let mut branch = String::new();
                io::stdin().read_line(&mut branch)?;
                self.run_git_command(&["checkout", "-b", branch.trim()])?;
                println!("✅ Created and switched to branch: {}", branch.trim());
            }
            _ => println!("❌ Invalid option"),
        }

        self.pause();
        Ok(())
    }

    async fn merge_branch(&mut self) -> Result<()> {
        println!("🔀 Merge branch:");
        print!("Enter branch name to merge: ");
        io::stdout().flush().unwrap();

        let mut branch = String::new();
        io::stdin().read_line(&mut branch)?;
        let branch = branch.trim();

        self.run_git_command(&["merge", branch])?;
        println!("✅ Branch '{}' merged successfully", branch);
        self.pause();
        Ok(())
    }

    async fn view_status(&mut self) -> Result<()> {
        println!("📋 Git Status:");
        let output = self.run_git_command_output(&["status"])?;
        println!("{}", output);
        self.pause();
        Ok(())
    }

    async fn generate_pr_description(&mut self) -> Result<()> {
        println!("✨ Generating AI-powered PR description...");
        let base = self.config.get_default_branch();
        git::generate_pr_description(base, "markdown", &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn generate_tests(&mut self) -> Result<()> {
        println!("🧪 Generating AI-powered unit tests...");
        let base = self.config.get_default_branch();
        git::generate_tests(base, "auto", &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn improve_commit_message(&mut self) -> Result<()> {
        println!("💬 Improve commit message:");
        print!("Enter commit hash (or press Enter for HEAD): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let commit_hash = if input.trim().is_empty() {
            None
        } else {
            Some(input.trim().to_string())
        };

        git::improve_commit_message(commit_hash.as_deref(), &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn interactive_commit(&mut self) -> Result<()> {
        println!("📝 Interactive commit:");
        println!("   1. Stage all changes");
        println!("   2. Stage specific files");
        print!("Choose option (1-2): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let all = input.trim() == "1";

        git::interactive_commit(all, &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn generate_changelog(&mut self) -> Result<()> {
        println!("📋 Generating changelog...");
        let base = self.config.get_default_branch();
        git::generate_changelog(base, None, &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn code_review(&mut self) -> Result<()> {
        println!("🔍 Performing AI code review...");
        let base = self.config.get_default_branch();
        git::code_review(base, &self.config).await?;
        self.pause();
        Ok(())
    }

    async fn refresh_status(&mut self) -> Result<()> {
        println!("🔄 Refreshing status...");
        self.update_status().await?;
        println!("✅ Status refreshed");
        self.pause();
        Ok(())
    }

    fn show_configuration(&mut self) -> Result<()> {
        println!("⚙️  Configuration:");
        println!("   Default Branch: {}", self.config.get_default_branch());
        println!("   OpenAI API Key: {}", if self.config.has_openai_key() { "✅ Set" } else { "❌ Not set" });
        println!("   GitHub Token: {}", if self.config.has_github_token() { "✅ Set" } else { "❌ Not set" });
        println!();
        println!("To configure, edit your .env file with:");
        println!("   COMMIT_BUDDY_DEFAULT_BRANCH=master");
        println!("   OPENAI_API_KEY=your_key_here");
        println!("   GITHUB_TOKEN=your_token_here");
        self.pause();
        Ok(())
    }

    fn exit(&self) -> Result<()> {
        println!("👋 Goodbye! Thanks for using Commit Buddy!");
        std::process::exit(0);
    }

    async fn update_status(&mut self) -> Result<()> {
        // Get current branch
        let branch_output = self.run_git_command_output(&["branch", "--show-current"])?;
        self.current_branch = branch_output.trim().to_string();

        // Get status summary
        let status_output = self.run_git_command_output(&["status", "--porcelain"])?;
        let lines: Vec<&str> = status_output.lines().collect();
        
        if lines.is_empty() {
            self.status = "Clean working directory".to_string();
        } else {
            self.status = format!("{} files changed", lines.len());
        }

        Ok(())
    }

    fn run_git_command(&self, args: &[&str]) -> Result<()> {
        let output = Command::new("git")
            .args(args)
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git command failed: {}", error));
        }

        Ok(())
    }

    fn run_git_command_output(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .output()?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Git command failed: {}", error));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn show_error(&self, error: &str) {
        println!("❌ Error: {}", error);
        self.pause();
    }

    fn pause(&self) {
        print!("\nPress Enter to continue...");
        io::stdout().flush().unwrap();
        let mut _input = String::new();
        io::stdin().read_line(&mut _input).unwrap();
    }
}
