use anyhow::Result;
use crate::config::Config;
use crate::git;
use crate::ai;
use git2;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::process::Command;

#[derive(Clone)]
pub struct GitStatus {
    pub branch: String,
    pub status: String,
    pub staged_files: Vec<String>,
    pub unstaged_files: Vec<String>,
    pub untracked_files: Vec<String>,
}

pub struct InteractiveCli {
    pub config: Config,
    pub git_status: GitStatus,
    pub list_state: ListState,
    pub current_tab: usize,
    pub should_quit: bool,
    pub commit_suggestions: Vec<String>,
    pub commit_list_state: ListState,
    pub in_commit_mode: bool,
}

impl InteractiveCli {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            git_status: GitStatus {
                branch: "unknown".to_string(),
                status: "unknown".to_string(),
                staged_files: Vec::new(),
                unstaged_files: Vec::new(),
                untracked_files: Vec::new(),
            },
            list_state: ListState::default(),
            current_tab: 0,
            should_quit: false,
            commit_suggestions: Vec::new(),
            commit_list_state: ListState::default(),
            in_commit_mode: false,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Initial status update
        self.update_git_status().await?;
        self.list_state.select(Some(0));

        // Main event loop
        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if self.in_commit_mode {
                        match key.code {
                            KeyCode::Up => {
                                self.navigate_commit_up();
                            }
                            KeyCode::Down => {
                                self.navigate_commit_down();
                            }
                            KeyCode::Enter => {
                                self.execute_commit().await?;
                            }
                            KeyCode::Esc => {
                                self.exit_commit_mode();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => {
                                self.should_quit = true;
                            }
                            KeyCode::Up => {
                                self.navigate_up();
                            }
                            KeyCode::Down => {
                                self.navigate_down();
                            }
                            KeyCode::Tab => {
                                self.next_tab();
                            }
                            KeyCode::BackTab => {
                                self.prev_tab();
                            }
                            KeyCode::Enter => {
                                self.handle_selection().await?;
                            }
                            KeyCode::Char('r') => {
                                self.update_git_status().await?;
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        Ok(())
    }

    fn ui(&mut self, f: &mut Frame) {
        if self.in_commit_mode {
            self.render_commit_mode(f);
        } else {
            self.render_main_ui(f);
        }
    }

    fn render_main_ui(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(3), // Status
                Constraint::Min(0),    // Main content
                Constraint::Length(3),  // Footer
            ])
            .split(f.size());

        // Header
        let header = Paragraph::new(Text::styled(
            "🤖 COMMIT BUDDY - AI-Powered Git Companion",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, chunks[0]);

        // Status bar
        let status_text = format!(
            "Branch: {} | Status: {} | AI: {}",
            self.git_status.branch,
            self.git_status.status,
            if self.config.has_openai_key() {
                "✅ Enabled"
            } else {
                "❌ Disabled"
            }
        );
        let status = Paragraph::new(Text::styled(
            status_text,
            Style::default().fg(Color::Yellow),
        ))
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(status, chunks[1]);

        // Main content area
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        // Left panel - Menu
        self.render_menu(f, main_chunks[0]);

        // Right panel - File status
        self.render_file_status(f, main_chunks[1]);

        // Footer
        let footer_text = "Press 'q' to quit | 'r' to refresh | 'Tab' to switch tabs | ↑↓ to navigate | Enter to select";
        let footer = Paragraph::new(Text::styled(
            footer_text,
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, chunks[3]);
    }

    fn render_commit_mode(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(5), // Instructions
                Constraint::Min(0),    // Commit suggestions
                Constraint::Length(3),  // Footer
            ])
            .split(f.size());

        // Header
        let header = Paragraph::new(Text::styled(
            "💬 AI-Powered Commit Message Selection",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, chunks[0]);

        // Instructions
        let instructions = Paragraph::new(Text::styled(
            "🤖 AI has generated conventional commit message suggestions.\nSelect one with ↑↓ and press Enter to commit, or 'Esc' to cancel.",
            Style::default().fg(Color::Yellow),
        ))
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(instructions, chunks[1]);

        // Commit suggestions
        let items: Vec<ListItem> = self.commit_suggestions
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if self.commit_list_state.selected() == Some(i) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(
                    format!("{}. {}", i + 1, suggestion),
                    style,
                )))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Commit Message Suggestions")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, chunks[2], &mut self.commit_list_state);

        // Footer
        let footer_text = "Press ↑↓ to navigate | Enter to select | Esc to cancel";
        let footer = Paragraph::new(Text::styled(
            footer_text,
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, chunks[3]);
    }

    fn render_menu(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let tabs = vec!["Git Operations", "AI Features", "Utilities"];
        let current_tab = tabs[self.current_tab];

        let menu_items = match self.current_tab {
            0 => vec![
                "📝 Add files to staging",
                "💾 Commit changes",
                "🚀 Push to remote",
                "📥 Pull from remote",
                "🌿 Switch branch",
                "🔀 Merge branch",
                "📋 View status",
            ],
            1 => vec![
                "✨ Generate PR description",
                "🧪 Generate unit tests",
                "💬 Improve commit message",
                "📝 Interactive commit",
                "📋 Generate changelog",
                "🔍 Code review",
            ],
            2 => vec![
                "🔄 Refresh status",
                "⚙️ Configuration",
                "❌ Exit",
            ],
            _ => vec![],
        };

        let items: Vec<ListItem> = menu_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if self.list_state.selected() == Some(i) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(Span::styled(*item, style)))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("{} | {}", current_tab, "Use ↑↓ to navigate"))
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_file_status(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Staged files header
                Constraint::Percentage(33), // Staged files
                Constraint::Length(3), // Unstaged files header
                Constraint::Percentage(33), // Unstaged files
                Constraint::Length(3), // Untracked files header
                Constraint::Percentage(34), // Untracked files
            ])
            .split(area);

        // Staged files
        let staged_header = Paragraph::new(Text::styled(
            format!("📁 Staged Files ({})", self.git_status.staged_files.len()),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ))
        .block(Block::default().borders(Borders::ALL));

        let staged_items: Vec<ListItem> = self.git_status.staged_files
            .iter()
            .map(|file| ListItem::new(Line::from(Span::styled(file, Style::default().fg(Color::Green)))))
            .collect();

        let staged_list = List::new(staged_items)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(staged_header, chunks[0]);
        f.render_widget(staged_list, chunks[1]);

        // Unstaged files
        let unstaged_header = Paragraph::new(Text::styled(
            format!("📝 Modified Files ({})", self.git_status.unstaged_files.len()),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
        .block(Block::default().borders(Borders::ALL));

        let unstaged_items: Vec<ListItem> = self.git_status.unstaged_files
            .iter()
            .map(|file| ListItem::new(Line::from(Span::styled(file, Style::default().fg(Color::Yellow)))))
            .collect();

        let unstaged_list = List::new(unstaged_items)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(unstaged_header, chunks[2]);
        f.render_widget(unstaged_list, chunks[3]);

        // Untracked files
        let untracked_header = Paragraph::new(Text::styled(
            format!("❓ Untracked Files ({})", self.git_status.untracked_files.len()),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
        .block(Block::default().borders(Borders::ALL));

        let untracked_items: Vec<ListItem> = self.git_status.untracked_files
            .iter()
            .map(|file| ListItem::new(Line::from(Span::styled(file, Style::default().fg(Color::Red)))))
            .collect();

        let untracked_list = List::new(untracked_items)
            .block(Block::default().borders(Borders::ALL));

        f.render_widget(untracked_header, chunks[4]);
        f.render_widget(untracked_list, chunks[5]);
    }

    fn navigate_up(&mut self) {
        let current = self.list_state.selected().unwrap_or(0);
        let max_items = self.get_current_menu_items().len();
        if current > 0 {
            self.list_state.select(Some(current - 1));
        } else {
            self.list_state.select(Some(max_items - 1));
        }
    }

    fn navigate_down(&mut self) {
        let current = self.list_state.selected().unwrap_or(0);
        let max_items = self.get_current_menu_items().len();
        if current < max_items - 1 {
            self.list_state.select(Some(current + 1));
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % 3;
        self.list_state.select(Some(0));
    }

    fn prev_tab(&mut self) {
        self.current_tab = if self.current_tab > 0 {
            self.current_tab - 1
        } else {
            2
        };
        self.list_state.select(Some(0));
    }

    fn get_current_menu_items(&self) -> Vec<&str> {
        match self.current_tab {
            0 => vec![
                "📝 Add files to staging",
                "💾 Commit changes",
                "🚀 Push to remote",
                "📥 Pull from remote",
                "🌿 Switch branch",
                "🔀 Merge branch",
                "📋 View status",
            ],
            1 => vec![
                "✨ Generate PR description",
                "🧪 Generate unit tests",
                "💬 Improve commit message",
                "📝 Interactive commit",
                "📋 Generate changelog",
                "🔍 Code review",
            ],
            2 => vec![
                "🔄 Refresh status",
                "⚙️ Configuration",
                "❌ Exit",
            ],
            _ => vec![],
        }
    }

    async fn handle_selection(&mut self) -> Result<()> {
        let selected = self.list_state.selected().unwrap_or(0);
        
        match self.current_tab {
            0 => self.handle_git_operation(selected).await?,
            1 => self.handle_ai_operation(selected).await?,
            2 => self.handle_utility(selected).await?,
            _ => {}
        }
        
        // Refresh status after operations
        self.update_git_status().await?;
        Ok(())
    }

    async fn handle_git_operation(&mut self, selected: usize) -> Result<()> {
        match selected {
            0 => self.add_files_to_staging().await?,
            1 => self.commit_changes().await?,
            2 => self.push_to_remote().await?,
            3 => self.pull_from_remote().await?,
            4 => self.switch_branch().await?,
            5 => self.merge_branch().await?,
            6 => self.view_status().await?,
            _ => {}
        }
        Ok(())
    }

    async fn handle_ai_operation(&mut self, selected: usize) -> Result<()> {
        match selected {
            0 => git::generate_pr_description("master", "markdown", &self.config).await?,
            1 => git::generate_tests("master", "auto", &self.config).await?,
            2 => git::improve_commit_message(None, &self.config).await?,
            3 => git::interactive_commit(false, &self.config).await?,
            4 => git::generate_changelog("master", None, &self.config).await?,
            5 => git::code_review("master", &self.config).await?,
            _ => {}
        }
        Ok(())
    }

    async fn handle_utility(&mut self, selected: usize) -> Result<()> {
        match selected {
            0 => self.update_git_status().await?,
            1 => self.show_configuration().await?,
            2 => self.should_quit = true,
            _ => {}
        }
        Ok(())
    }

    async fn update_git_status(&mut self) -> Result<()> {
        // Get current branch
        let output = Command::new("git")
            .args(&["branch", "--show-current"])
            .output()?;
        self.git_status.branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get git status
        let output = Command::new("git")
            .args(&["status", "--porcelain"])
            .output()?;
        let status_output = String::from_utf8_lossy(&output.stdout);

        // Parse status
        self.git_status.staged_files.clear();
        self.git_status.unstaged_files.clear();
        self.git_status.untracked_files.clear();

        for line in status_output.lines() {
            if line.len() >= 2 {
                let status = &line[0..2];
                let file = &line[3..];
                
                match status {
                    "A " | "M " | "D " => self.git_status.staged_files.push(file.to_string()),
                    " M" | " D" => self.git_status.unstaged_files.push(file.to_string()),
                    "??" => self.git_status.untracked_files.push(file.to_string()),
                    "AM" | "MM" => {
                        self.git_status.staged_files.push(file.to_string());
                        self.git_status.unstaged_files.push(file.to_string());
                    }
                    _ => {}
                }
            }
        }

        // Update status text
        let total_changes = self.git_status.staged_files.len() + 
                          self.git_status.unstaged_files.len() + 
                          self.git_status.untracked_files.len();
        
        self.git_status.status = if total_changes == 0 {
            "Clean working directory".to_string()
        } else {
            format!("{} files changed", total_changes)
        };

        Ok(())
    }

    async fn add_files_to_staging(&mut self) -> Result<()> {
        // Simple implementation - stage all changes
        Command::new("git").args(&["add", "."]).status()?;
        Ok(())
    }

    async fn commit_changes(&mut self) -> Result<()> {
        self.start_interactive_commit(false).await?;
        Ok(())
    }

    async fn push_to_remote(&mut self) -> Result<()> {
        Command::new("git").args(&["push"]).status()?;
        Ok(())
    }

    async fn pull_from_remote(&mut self) -> Result<()> {
        Command::new("git").args(&["pull"]).status()?;
        Ok(())
    }

    async fn switch_branch(&mut self) -> Result<()> {
        // Simple implementation - could be enhanced with branch selection
        Command::new("git").args(&["checkout", "-b", "new-branch"]).status()?;
        Ok(())
    }

    async fn merge_branch(&mut self) -> Result<()> {
        // Simple implementation
        Command::new("git").args(&["merge", "main"]).status()?;
        Ok(())
    }

    async fn view_status(&mut self) -> Result<()> {
        // Status is already displayed in the UI
        Ok(())
    }

    async fn show_configuration(&mut self) -> Result<()> {
        // Could show a configuration panel
        Ok(())
    }

    // Commit mode methods
    async fn start_interactive_commit(&mut self, all: bool) -> Result<()> {
        if all {
            // Stage all changes
            Command::new("git").args(&["add", "."]).status()?;
        }

        // Get staged changes and generate AI suggestions
        let diff_info = git::get_staged_changes()?;
        
        if diff_info.commits.is_empty() {
            // No staged changes, show message and return
            return Ok(());
        }

        // Generate AI suggestions
        self.commit_suggestions = ai::generate_commit_suggestions(&diff_info, &self.config).await?;
        
        if self.commit_suggestions.is_empty() {
            // Fallback if AI fails
            self.commit_suggestions = vec![
                "feat: add new functionality".to_string(),
                "fix: resolve issue".to_string(),
                "chore: update code".to_string(),
            ];
        }

        // Enter commit mode
        self.in_commit_mode = true;
        self.commit_list_state.select(Some(0));
        
        Ok(())
    }

    fn navigate_commit_up(&mut self) {
        let current = self.commit_list_state.selected().unwrap_or(0);
        if current > 0 {
            self.commit_list_state.select(Some(current - 1));
        } else {
            self.commit_list_state.select(Some(self.commit_suggestions.len() - 1));
        }
    }

    fn navigate_commit_down(&mut self) {
        let current = self.commit_list_state.selected().unwrap_or(0);
        if current < self.commit_suggestions.len() - 1 {
            self.commit_list_state.select(Some(current + 1));
        } else {
            self.commit_list_state.select(Some(0));
        }
    }

    async fn execute_commit(&mut self) -> Result<()> {
        let selected = self.commit_list_state.selected().unwrap_or(0);
        
        if selected < self.commit_suggestions.len() {
            let commit_message = &self.commit_suggestions[selected];
            
            // Perform the actual commit
            let repo = git2::Repository::open(".")?;
            let mut index = repo.index()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            
            let signature = repo.signature()?;
            let head = repo.head()?;
            let parent_commit = head.peel_to_commit()?;
            
            let _commit_id = repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                commit_message,
                &tree,
                &[&parent_commit],
            )?;
            
            // Exit commit mode and refresh status
            self.exit_commit_mode();
            self.update_git_status().await?;
        }
        
        Ok(())
    }

    fn exit_commit_mode(&mut self) {
        self.in_commit_mode = false;
        self.commit_suggestions.clear();
        self.commit_list_state.select(None);
    }
}