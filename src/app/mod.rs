pub mod diff;
pub mod editing;
pub mod events;
pub mod mode_handlers;
pub mod render;
pub mod split_ops;

use crate::{
    buffer::TextBuffer,
    buffer_manager::BufferManager,
    config::Config,
    theme::ThemeManager,
    sidebar::{Sidebar, SidebarMode, GitStatus},
    split::SplitManager,
    syntax::SyntaxHighlighter,
    undo::UndoManager,
};
use self::diff::DiffInfo;
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyEvent, MouseEvent};
use git2::Repository;
use ratatui::Frame;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
    Search,
    Replace,
    QuitConfirm,
}

pub struct App {
    pub mode: Mode,
    pub buffer_manager: BufferManager,
    pub should_quit: bool,
    pub config: Config,
    pub theme_manager: ThemeManager,
    pub syntax_highlighter: SyntaxHighlighter,
    pub sidebar: Option<Sidebar>,
    pub show_sidebar: bool,
    pub split_manager: Option<SplitManager>,
    pub viewport_offset: usize,
    pub status_message: String,
    pub command_buffer: String,
    pub search_query: String,
    pub replace_text: String,
    pub search_matches: Vec<(usize, usize, usize)>, // (row, start_col, end_col)
    pub current_match_index: usize,
    pub case_sensitive: bool,
    pub clipboard: Option<Clipboard>,
    pub undo_manager: UndoManager,
    pub git_repo: Option<Repository>,
    pub git_branch: Option<String>,
    pub git_status_cache: Option<(usize, usize)>, // (staged_count, modified_count)
    pub last_git_cache_update: std::time::Instant,
    pub diff_info: Option<DiffInfo>,
    pub unsaved_buffers_to_check: Vec<usize>,
    pub last_key: Option<char>,
    pub processing_ctrl_w: bool,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        Self::new_with_dir(config, std::env::current_dir()?)
    }

    pub fn new_with_dir(config: Config, working_dir: PathBuf) -> Result<Self> {
        std::env::set_current_dir(&working_dir)?;
        let theme_manager = ThemeManager::new();
        let syntax_highlighter = SyntaxHighlighter::new();
        let buffer_manager = BufferManager::new();
        let undo_manager = UndoManager::new();

        // Try to initialize clipboard
        let clipboard = match Clipboard::new() {
            Ok(c) => {
                eprintln!("Clipboard initialized successfully");
                Some(c)
            }
            Err(e) => {
                eprintln!("Failed to initialize clipboard: {}", e);
                None
            }
        };

        // Try to open git repository
        let git_repo = Repository::open(".").ok();
        let git_branch = if let Some(ref repo) = git_repo {
            repo.head()
                .ok()
                .and_then(|head| head.shorthand().map(|s| s.to_string()))
        } else {
            None
        };

        let sidebar = match Sidebar::new(std::env::current_dir()?) {
            Ok(s) => Some(s),
            Err(_) => None,
        };

        Ok(App {
            mode: Mode::Normal,
            buffer_manager,
            should_quit: false,
            config,
            theme_manager,
            syntax_highlighter,
            sidebar,
            show_sidebar: false,
            split_manager: None,
            viewport_offset: 0,
            status_message: String::from("Ready"),
            command_buffer: String::new(),
            search_query: String::new(),
            replace_text: String::new(),
            search_matches: Vec::new(),
            current_match_index: 0,
            case_sensitive: false,
            clipboard,
            undo_manager,
            git_repo,
            git_branch,
            git_status_cache: None,
            last_git_cache_update: std::time::Instant::now(),
            diff_info: None,
            unsaved_buffers_to_check: Vec::new(),
            last_key: None,
            processing_ctrl_w: false,
        })
    }

    pub fn run(&mut self, frame: &mut Frame) -> Result<()> {
        self.draw(frame);
        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    pub fn load_file(&mut self, path: PathBuf) -> Result<()> {
        self.buffer_manager.open_file(&path, &self.syntax_highlighter)?;
        // Note: sidebar directory update removed as the method doesn't exist
        self.status_message = format!("Loaded: {}", path.display());
        Ok(())
    }

    pub fn save_current_buffer(&mut self) -> Result<()> {
        self.buffer_manager.current_mut().save()?;
        self.status_message = String::from("File saved");
        Ok(())
    }

    pub fn save_file(&mut self) -> Result<()> {
        self.save_current_buffer()
    }

    pub fn toggle_sidebar(&mut self) {
        self.show_sidebar = !self.show_sidebar;
        if self.show_sidebar {
            self.status_message = String::from("Sidebar shown");
        } else {
            self.status_message = String::from("Sidebar hidden");
        }
    }

    pub fn update_git_cache(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_git_cache_update).as_secs() >= 5 {
            if let Some(ref repo) = self.git_repo {
                if let Ok(statuses) = repo.statuses(None) {
                    let mut staged_count = 0;
                    let mut modified_count = 0;

                    for status in statuses.iter() {
                        let flags = status.status();
                        if flags.is_index_new() || flags.is_index_modified() || flags.is_index_deleted() {
                            staged_count += 1;
                        }
                        if flags.is_wt_new() || flags.is_wt_modified() || flags.is_wt_deleted() {
                            modified_count += 1;
                        }
                    }

                    self.git_status_cache = Some((staged_count, modified_count));
                }
            }
            self.last_git_cache_update = now;
        }
    }

    pub fn execute_command(&mut self, command: &str) -> Result<()> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "q" | "quit" => {
                self.try_quit();
            }
            "q!" | "quit!" => {
                self.should_quit = true;
            }
            "w" | "write" => {
                if parts.len() > 1 {
                    // Save with new filename
                    let path = PathBuf::from(parts[1]);
                    self.buffer_manager.current_mut().file_path = Some(path.clone());
                    self.buffer_manager.current_mut().save()?;
                    self.status_message = format!("Saved as: {}", path.display());
                } else {
                    self.save_current_buffer()?;
                }
            }
            "wq" | "x" => {
                self.save_current_buffer()?;
                self.should_quit = true;
            }
            "sp" | "split" => {
                self.split_horizontal()?;
            }
            "vsp" | "vsplit" => {
                self.split_vertical()?;
            }
            "diff" | "wd" => {
                self.show_diff_view()?;
            }
            "theme" => {
                if parts.len() > 1 {
                    if self.theme_manager.set_theme(parts[1]) {
                        self.status_message = format!("Theme changed to: {}", parts[1]);
                    } else {
                        self.status_message = format!("Failed to load theme: {}", parts[1]);
                    }
                } else {
                    self.status_message = String::from("Usage: :theme <theme_name>");
                }
            }
            "cd" => {
                if parts.len() > 1 {
                    match std::env::set_current_dir(parts[1]) {
                        Ok(_) => {
                            self.status_message = format!("Changed directory to: {}", parts[1]);
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to change directory: {}", e);
                        }
                    }
                } else {
                    self.status_message = format!("Current directory: {}", std::env::current_dir()?.display());
                }
            }
            "help" => {
                self.status_message = String::from("Commands: q quit, w write, wq save&quit, sp split, vsp vsplit, diff, theme <name>, cd <dir>");
            }
            _ => {
                self.status_message = format!("Unknown command: {}", parts[0]);
            }
        }

        Ok(())
    }

    fn find_and_highlight_matches(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            return;
        }

        let buffer = self.buffer_manager.current();
        let mut matches = Vec::new();

        for (line_idx, line) in buffer.content.lines().enumerate() {
            let line_str = line.to_string();
            let search_in = if self.case_sensitive {
                line_str.clone()
            } else {
                line_str.to_lowercase()
            };

            let search_for = if self.case_sensitive {
                self.search_query.clone()
            } else {
                self.search_query.to_lowercase()
            };

            let mut start = 0;
            while let Some(pos) = search_in[start..].find(&search_for) {
                let actual_pos = start + pos;
                matches.push((line_idx, actual_pos, actual_pos + search_for.len()));
                start = actual_pos + 1;
            }
        }

        self.search_matches = matches;
        self.current_match_index = 0;
    }

    pub fn go_to_next_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match_index = (self.current_match_index + 1) % self.search_matches.len();
            let (row, col, _) = self.search_matches[self.current_match_index];
            self.buffer_manager.current_mut().cursor_position = (row, col);
            self.update_viewport();
            self.status_message = format!("Match {} of {}", self.current_match_index + 1, self.search_matches.len());
        }
    }

    pub fn go_to_previous_match(&mut self) {
        if !self.search_matches.is_empty() {
            self.current_match_index = if self.current_match_index == 0 {
                self.search_matches.len() - 1
            } else {
                self.current_match_index - 1
            };
            let (row, col, _) = self.search_matches[self.current_match_index];
            self.buffer_manager.current_mut().cursor_position = (row, col);
            self.update_viewport();
            self.status_message = format!("Match {} of {}", self.current_match_index + 1, self.search_matches.len());
        }
    }

    pub fn open_file(&mut self, path: &PathBuf) -> Result<()> {
        self.buffer_manager.open_file(path, &self.syntax_highlighter)?;
        self.status_message = format!("Opened: {}", path.display());
        Ok(())
    }
}
