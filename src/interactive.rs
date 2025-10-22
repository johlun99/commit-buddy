use anyhow::Result;
use crate::config::Config;
use crate::git;
use crate::ai;
use crate::github;
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
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct FileItem {
    pub path: String,
    pub status: FileStatus,
    pub selected: bool,
}

#[derive(Clone, PartialEq)]
pub enum FileStatus {
    Staged,
    Modified,
    Untracked,
    Deleted,
}

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
    pub in_file_mode: bool,
    pub file_items: Vec<FileItem>,
    pub file_list_state: ListState,
    pub in_display_mode: bool,
    pub display_content: String,
    pub display_title: String,
    pub in_loading_mode: bool,
    pub loading_message: String,
    pub loading_spinner: usize,
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
            in_file_mode: false,
            file_items: Vec::new(),
            file_list_state: ListState::default(),
            in_display_mode: false,
            display_content: String::new(),
            display_title: String::new(),
            in_loading_mode: false,
            loading_message: String::new(),
            loading_spinner: 0,
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
        let mut last_spinner_update = Instant::now();
        loop {
            terminal.draw(|f| self.ui(f))?;

            // Update spinner if in loading mode
            if self.in_loading_mode && last_spinner_update.elapsed() >= Duration::from_millis(100) {
                self.loading_spinner = (self.loading_spinner + 1) % 10;
                last_spinner_update = Instant::now();
            }

            // Use a timeout for event reading to allow spinner updates
            let timeout = if self.in_loading_mode {
                Duration::from_millis(50)
            } else {
                Duration::from_millis(100)
            };

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.in_loading_mode {
                            // Only allow quit during loading
                            match key.code {
                                KeyCode::Char('q') => {
                                    self.should_quit = true;
                                }
                                _ => {}
                            }
                        } else if self.in_commit_mode {
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
                        } else if self.in_file_mode {
                            match key.code {
                                KeyCode::Up => {
                                    self.navigate_file_up();
                                }
                                KeyCode::Down => {
                                    self.navigate_file_down();
                                }
                                KeyCode::Char(' ') => {
                                    self.toggle_file_staging().await?;
                                }
                                KeyCode::Char('a') => {
                                    self.stage_all_files().await?;
                                }
                                KeyCode::Char('u') => {
                                    self.unstage_all_files().await?;
                                }
                                KeyCode::Esc => {
                                    self.exit_file_mode().await?;
                                }
                                _ => {}
                            }
                        } else if self.in_display_mode {
                            match key.code {
                                KeyCode::Esc => {
                                    self.exit_display_mode();
                                }
                                KeyCode::Char('q') => {
                                    self.should_quit = true;
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
                                KeyCode::Char('f') => {
                                    self.enter_file_mode().await?;
                                }
                                _ => {}
                            }
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
        if self.in_loading_mode {
            self.render_loading_mode(f);
        } else if self.in_commit_mode {
            self.render_commit_mode(f);
        } else if self.in_file_mode {
            self.render_file_mode(f);
        } else if self.in_display_mode {
            self.render_display_mode(f);
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
        let footer_text = "Press 'q' to quit | 'r' to refresh | 'f' for files | 'Tab' to switch tabs | ↑↓ to navigate | Enter to select";
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

    fn render_file_mode(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(5), // Instructions
                Constraint::Min(0),    // File list
                Constraint::Length(3),  // Footer
            ])
            .split(f.size());

        // Header
        let header = Paragraph::new(Text::styled(
            "📁 File Staging/Unstaging",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, chunks[0]);

        // Instructions
        let instructions = Paragraph::new(Text::styled(
            "Use ↑↓ to navigate files | Space to stage/unstage | 'a' to stage all | 'u' to unstage all | Esc to return",
            Style::default().fg(Color::Yellow),
        ))
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(instructions, chunks[1]);

        // File list
        let items: Vec<ListItem> = self.file_items
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let status_icon = match file.status {
                    FileStatus::Staged => "✅",
                    FileStatus::Modified => "📝",
                    FileStatus::Untracked => "❓",
                    FileStatus::Deleted => "🗑️",
                };
                
                let style = if self.file_list_state.selected() == Some(i) {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    match file.status {
                        FileStatus::Staged => Style::default().fg(Color::Green),
                        FileStatus::Modified => Style::default().fg(Color::Yellow),
                        FileStatus::Untracked => Style::default().fg(Color::Red),
                        FileStatus::Deleted => Style::default().fg(Color::Magenta),
                    }
                };
                
                ListItem::new(Line::from(Span::styled(
                    format!("{} {}", status_icon, file.path),
                    style,
                )))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Files")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_stateful_widget(list, chunks[2], &mut self.file_list_state);

        // Footer
        let footer_text = "Space: Toggle | 'a': Stage All | 'u': Unstage All | Esc: Back";
        let footer = Paragraph::new(Text::styled(
            footer_text,
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, chunks[3]);
    }

    fn render_loading_mode(&mut self, f: &mut Frame) {
        // Render the normal UI first
        if self.in_commit_mode {
            self.render_commit_mode(f);
        } else if self.in_file_mode {
            self.render_file_mode(f);
        } else if self.in_display_mode {
            self.render_display_mode(f);
        } else {
            self.render_main_ui(f);
        }

        // Create a centered dialog overlay
        let popup_area = centered_rect(60, 25, f.size());
        
        // Semi-transparent background
        let background = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Black).fg(Color::White));
        f.render_widget(background, popup_area);

        // Inner content area
        let inner_area = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer
            ])
            .split(popup_area);

        // Header
        let header = Paragraph::new(Text::styled(
            "🤖 AI Processing",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, inner_area[0]);

        // Loading content with spinner
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spinner = spinner_chars[self.loading_spinner % spinner_chars.len()];
        
        let loading_text = format!(
            "{}\n\n{}\n\nPlease wait while AI processes your request...",
            spinner,
            self.loading_message
        );

        let content = Paragraph::new(Text::styled(
            loading_text,
            Style::default().fg(Color::Yellow),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(content, inner_area[1]);

        // Footer
        let footer_text = "AI is working... Please wait";
        let footer = Paragraph::new(Text::styled(
            footer_text,
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, inner_area[2]);
    }

    fn render_display_mode(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3),  // Footer
            ])
            .split(f.size());

        // Header
        let header = Paragraph::new(Text::styled(
            &self.display_title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(header, chunks[0]);

        // Content
        let content = Paragraph::new(Text::styled(
            &self.display_content,
            Style::default().fg(Color::White),
        ))
        .block(Block::default().borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });

        f.render_widget(content, chunks[1]);

        // Footer
        let footer_text = "Press 'q' to quit | Esc to go back | ↑↓ to scroll";
        let footer = Paragraph::new(Text::styled(
            footer_text,
            Style::default().fg(Color::Gray),
        ))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

        f.render_widget(footer, chunks[2]);
    }

    fn render_menu(&mut self, f: &mut Frame, area: ratatui::layout::Rect) {
        let tabs = vec!["Git Operations", "AI Features", "Utilities"];
        let current_tab = tabs[self.current_tab];

        let menu_items = match self.current_tab {
            0 => vec![
                "📁 Manage files (f)",
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
                "🚀 Create PR with AI description",
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
            0 => self.enter_file_mode().await?,
            1 => self.add_files_to_staging().await?,
            2 => self.commit_changes().await?,
            3 => self.push_to_remote().await?,
            4 => self.pull_from_remote().await?,
            5 => self.switch_branch().await?,
            6 => self.merge_branch().await?,
            7 => self.view_status().await?,
            _ => {}
        }
        Ok(())
    }

    async fn handle_ai_operation(&mut self, selected: usize) -> Result<()> {
        match selected {
            0 => self.show_pr_description().await?,
            1 => self.create_pr_with_ai_description().await?,
            2 => self.show_generated_tests().await?,
            3 => self.show_improved_commit_message().await?,
            4 => self.start_interactive_commit(false).await?,
            5 => self.show_changelog().await?,
            6 => self.show_code_review().await?,
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
        let output = Command::new("git")
            .args(&["add", "."])
            .output()?;
        
        if output.status.success() {
            // Files staged successfully
        }
        
        Ok(())
    }

    async fn commit_changes(&mut self) -> Result<()> {
        self.start_interactive_commit(false).await?;
        Ok(())
    }

    async fn push_to_remote(&mut self) -> Result<()> {
        let output = Command::new("git")
            .args(&["push"])
            .output()?;
        
        if output.status.success() {
            // Push successful
        } else {
            // Push failed - could show error message in TUI
        }
        
        Ok(())
    }

    async fn pull_from_remote(&mut self) -> Result<()> {
        let output = Command::new("git")
            .args(&["pull"])
            .output()?;
        
        if output.status.success() {
            // Pull successful
        } else {
            // Pull failed - could show error message in TUI
        }
        
        Ok(())
    }

    async fn switch_branch(&mut self) -> Result<()> {
        // Simple implementation - could be enhanced with branch selection
        let output = Command::new("git")
            .args(&["checkout", "-b", "new-branch"])
            .output()?;
        
        if output.status.success() {
            // Branch created successfully
        } else {
            // Branch creation failed
        }
        
        Ok(())
    }

    async fn merge_branch(&mut self) -> Result<()> {
        // Simple implementation
        let output = Command::new("git")
            .args(&["merge", "main"])
            .output()?;
        
        if output.status.success() {
            // Merge successful
        } else {
            // Merge failed
        }
        
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
            let output = Command::new("git")
                .args(&["add", "."])
                .output()?;
            
            if output.status.success() {
                // Files staged successfully
            }
        }

        self.start_loading("Generating commit suggestions...".to_string());

        // Get staged changes and generate AI suggestions
        let diff_info = git::get_staged_changes()?;
        
        if diff_info.commits.is_empty() {
            // No staged changes, show message and return
            self.stop_loading();
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

        self.stop_loading();

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

    // File mode methods
    async fn enter_file_mode(&mut self) -> Result<()> {
        self.load_file_items().await?;
        self.in_file_mode = true;
        self.file_list_state.select(Some(0));
        Ok(())
    }

    async fn exit_file_mode(&mut self) -> Result<()> {
        self.in_file_mode = false;
        self.file_items.clear();
        self.file_list_state.select(None);
        // Refresh git status to show updated file states
        self.update_git_status().await?;
        Ok(())
    }

    async fn load_file_items(&mut self) -> Result<()> {
        self.file_items.clear();
        
        // Get git status
        let output = Command::new("git")
            .args(&["status", "--porcelain"])
            .output()?;
        let status_output = String::from_utf8_lossy(&output.stdout);

        for line in status_output.lines() {
            if line.len() >= 2 {
                let status = &line[0..2];
                let file = &line[3..];
                
                let file_status = match status {
                    "A " | "M " | "D " => FileStatus::Staged,
                    " M" | " D" => FileStatus::Modified,
                    "??" => FileStatus::Untracked,
                    "AM" | "MM" => FileStatus::Staged, // Show as staged if any part is staged
                    _ => continue,
                };
                
                self.file_items.push(FileItem {
                    path: file.to_string(),
                    status: file_status,
                    selected: false,
                });
            }
        }
        
        Ok(())
    }

    fn navigate_file_up(&mut self) {
        let current = self.file_list_state.selected().unwrap_or(0);
        if current > 0 {
            self.file_list_state.select(Some(current - 1));
        } else {
            self.file_list_state.select(Some(self.file_items.len() - 1));
        }
    }

    fn navigate_file_down(&mut self) {
        let current = self.file_list_state.selected().unwrap_or(0);
        if current < self.file_items.len() - 1 {
            self.file_list_state.select(Some(current + 1));
        } else {
            self.file_list_state.select(Some(0));
        }
    }

    async fn toggle_file_staging(&mut self) -> Result<()> {
        let selected = self.file_list_state.selected().unwrap_or(0);
        
        if selected < self.file_items.len() {
            let file = &self.file_items[selected];
            
            match file.status {
                FileStatus::Staged => {
                    // Unstage the file
                    let output = Command::new("git")
                        .args(&["reset", "HEAD", "--", &file.path])
                        .output()?;
                    
                    if output.status.success() {
                        // File unstaged successfully - status will be updated by refresh
                    }
                }
                FileStatus::Modified | FileStatus::Untracked => {
                    // Stage the file
                    let output = Command::new("git")
                        .args(&["add", &file.path])
                        .output()?;
                    
                    if output.status.success() {
                        // File staged successfully - status will be updated by refresh
                    }
                }
                FileStatus::Deleted => {
                    // Handle deleted files
                    let output = Command::new("git")
                        .args(&["rm", &file.path])
                        .output()?;
                    
                    if output.status.success() {
                        // File removed successfully - status will be updated by refresh
                    }
                }
            }
            
            // Reload file items to reflect changes
            self.load_file_items().await?;
            
            // Also refresh the main git status
            self.update_git_status().await?;
        }
        
        Ok(())
    }

    async fn stage_all_files(&mut self) -> Result<()> {
        let output = Command::new("git")
            .args(&["add", "."])
            .output()?;
        
        if output.status.success() {
            // Files staged successfully
        }
        
        self.load_file_items().await?;
        // Refresh the main git status
        self.update_git_status().await?;
        Ok(())
    }

    async fn unstage_all_files(&mut self) -> Result<()> {
        let output = Command::new("git")
            .args(&["reset", "HEAD", "--", "."])
            .output()?;
        
        if output.status.success() {
            // Files unstaged successfully
        }
        
        self.load_file_items().await?;
        // Refresh the main git status
        self.update_git_status().await?;
        Ok(())
    }

    // PR creation method
    async fn create_pr_with_ai_description(&mut self) -> Result<()> {
        // Check if GitHub token is available
        if !self.config.has_github_token() {
            // Could show a message in TUI about missing GitHub token
            return Ok(());
        }

        self.start_loading("Creating PR with AI description...".to_string());

        // Get current branch
        let output = Command::new("git")
            .args(&["branch", "--show-current"])
            .output()?;
        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Get base branch (default to master)
        let base_branch = self.config.get_default_branch();

        // Generate PR description using AI
        let diff_info = git::get_diff_info(base_branch)?;
        let pr_description = ai::generate_pr_description(&diff_info, &self.config).await?;

        // Get repository info
        let github_config = github::load_github_config()?;
        let _repo_info = github::get_repository_info(&github_config).await?;

        // Create PR info
        let pr_info = github::PullRequest {
            title: format!("feat: {}", current_branch.replace('-', " ").replace('_', " ")),
            body: pr_description,
            head: current_branch.clone(),
            base: base_branch.to_string(),
        };

        // Create the PR
        let _pr_url = github::create_pull_request(&github_config, &pr_info).await?;

        self.stop_loading();

        // Could show success message in TUI
        // For now, the PR is created successfully
        
        Ok(())
    }

    // Display mode methods
    fn exit_display_mode(&mut self) {
        self.in_display_mode = false;
        self.display_content.clear();
        self.display_title.clear();
    }

    async fn show_pr_description(&mut self) -> Result<()> {
        self.start_loading("Generating PR description...".to_string());
        
        let base_branch = self.config.get_default_branch();
        let diff_info = git::get_diff_info(base_branch)?;
        let description = ai::generate_pr_description(&diff_info, &self.config).await?;
        
        self.stop_loading();
        
        self.display_title = "📋 AI-Generated PR Description".to_string();
        self.display_content = description;
        self.in_display_mode = true;
        
        Ok(())
    }

    async fn show_generated_tests(&mut self) -> Result<()> {
        self.start_loading("Generating unit tests...".to_string());
        
        let base_branch = self.config.get_default_branch();
        let diff_info = git::get_diff_info(base_branch)?;
        let tests = ai::generate_tests(&diff_info, "auto", &self.config).await?;
        
        self.stop_loading();
        
        self.display_title = "🧪 AI-Generated Unit Tests".to_string();
        self.display_content = tests;
        self.in_display_mode = true;
        
        Ok(())
    }

    async fn show_improved_commit_message(&mut self) -> Result<()> {
        self.start_loading("Improving commit message...".to_string());
        
        let message = ai::improve_commit_message("HEAD", &self.config).await?;
        
        self.stop_loading();
        
        self.display_title = "💬 AI-Improved Commit Message".to_string();
        self.display_content = message;
        self.in_display_mode = true;
        
        Ok(())
    }

    async fn show_changelog(&mut self) -> Result<()> {
        self.start_loading("Generating changelog...".to_string());
        
        let base_branch = self.config.get_default_branch();
        let diff_info = git::get_diff_info(base_branch)?;
        let changelog = ai::generate_changelog(&diff_info, &self.config).await?;
        
        self.stop_loading();
        
        self.display_title = "📋 AI-Generated Changelog".to_string();
        self.display_content = changelog;
        self.in_display_mode = true;
        
        Ok(())
    }

    async fn show_code_review(&mut self) -> Result<()> {
        self.start_loading("Performing code review...".to_string());
        
        let base_branch = self.config.get_default_branch();
        let diff_info = git::get_diff_info(base_branch)?;
        let review = ai::code_review(&diff_info, &self.config).await?;
        
        self.stop_loading();
        
        self.display_title = "🔍 AI Code Review".to_string();
        self.display_content = review;
        self.in_display_mode = true;
        
        Ok(())
    }

    // Loading helper methods
    fn start_loading(&mut self, message: String) {
        self.in_loading_mode = true;
        self.loading_message = message;
        self.loading_spinner = 0;
    }

    fn stop_loading(&mut self) {
        self.in_loading_mode = false;
        self.loading_message.clear();
        self.loading_spinner = 0;
    }
}

// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}