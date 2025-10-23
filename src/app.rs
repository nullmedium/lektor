use crate::buffer::TextBuffer;
use crate::buffer_manager::BufferManager;
use crate::config::Config;
use crate::sidebar::{GitStatus, Sidebar, SidebarMode};
use crate::split::{SplitManager, SplitDirection, Pane};
use crate::syntax::SyntaxHighlighter;
use crate::theme::{get_ui_style, hex_to_color, ThemeManager};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use git2::Repository;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    pub config: Config,
    pub buffer_manager: BufferManager,
    pub sidebar: Option<Sidebar>,
    pub split_manager: Option<SplitManager>,
    pub syntax_highlighter: SyntaxHighlighter,
    pub theme_manager: ThemeManager,
    pub mode: Mode,
    pub should_quit: bool,
    pub status_message: String,
    pub show_sidebar: bool,
    pub viewport_offset: usize,
    pub command_buffer: String,
    clipboard: Option<Clipboard>,
    search_query: String,
    replace_text: String,
    case_sensitive: bool,
    last_search_position: Option<(usize, usize)>,
    search_matches: Vec<(usize, usize, usize)>,
    unsaved_buffers_to_check: Vec<usize>,
    quit_confirmed: bool,
    last_key: Option<KeyCode>,
    git_repo: Option<Repository>,
    git_branch: Option<String>,
    git_status_cache: Option<(usize, usize)>, // (staged, modified)
    git_cache_timestamp: std::time::Instant,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        Self::new_with_dir(config, std::env::current_dir()?)
    }

    fn update_git_cache(&mut self) {
        // Update git cache immediately without time check
        self.git_cache_timestamp = Instant::now();

        if let Some(ref repo) = self.git_repo {
            // Update branch name
            self.git_branch = repo.head().ok().and_then(|head| {
                head.shorthand().map(|s| s.to_string())
            });

            // Update status counts
            if let Ok(statuses) = repo.statuses(None) {
                let mut modified_files = 0;
                let mut staged_files = 0;

                for entry in statuses.iter() {
                    let status = entry.status();
                    if status.contains(git2::Status::WT_MODIFIED)
                        || status.contains(git2::Status::WT_NEW)
                        || status.contains(git2::Status::WT_DELETED) {
                        modified_files += 1;
                    }
                    if status.contains(git2::Status::INDEX_NEW)
                        || status.contains(git2::Status::INDEX_MODIFIED)
                        || status.contains(git2::Status::INDEX_DELETED) {
                        staged_files += 1;
                    }
                }

                self.git_status_cache = Some((staged_files, modified_files));
            } else {
                self.git_status_cache = None;
            }
        }
    }

    pub fn new_with_dir(config: Config, working_dir: PathBuf) -> Result<Self> {
        let mut theme_manager = ThemeManager::new();
        if !theme_manager.set_theme(&config.theme.name) {
            theme_manager.set_theme("Dark");
        }

        let sidebar = Sidebar::new(working_dir.clone()).ok();

        // Set the syntect theme from config
        let mut syntax_highlighter = SyntaxHighlighter::new();
        syntax_highlighter.set_theme(&config.theme.syntax_theme);

        // Try to initialize clipboard
        let clipboard = Clipboard::new().ok();

        // Try to open git repository and cache initial branch
        let git_repo = Repository::open(&working_dir).ok();
        let git_branch = git_repo.as_ref().and_then(|repo| {
            repo.head().ok().and_then(|head| {
                head.shorthand().map(|s| s.to_string())
            })
        });

        // Initialize git status cache
        let git_status_cache = git_repo.as_ref().and_then(|repo| {
            repo.statuses(None).ok().map(|statuses| {
                let mut modified_files = 0;
                let mut staged_files = 0;

                for entry in statuses.iter() {
                    let status = entry.status();
                    if status.contains(git2::Status::WT_MODIFIED)
                        || status.contains(git2::Status::WT_NEW)
                        || status.contains(git2::Status::WT_DELETED) {
                        modified_files += 1;
                    }
                    if status.contains(git2::Status::INDEX_NEW)
                        || status.contains(git2::Status::INDEX_MODIFIED)
                        || status.contains(git2::Status::INDEX_DELETED) {
                        staged_files += 1;
                    }
                }

                (staged_files, modified_files)
            })
        });

        Ok(Self {
            config,
            buffer_manager: BufferManager::new(),
            sidebar,
            split_manager: None,
            syntax_highlighter,
            theme_manager,
            mode: Mode::Normal,
            should_quit: false,
            status_message: String::from("Ready"),
            show_sidebar: true,
            viewport_offset: 0,
            command_buffer: String::new(),
            clipboard,
            search_query: String::new(),
            replace_text: String::new(),
            case_sensitive: false,
            last_search_position: None,
            search_matches: Vec::new(),
            unsaved_buffers_to_check: Vec::new(),
            quit_confirmed: false,
            last_key: None,
            git_repo,
            git_branch,
            git_status_cache,
            git_cache_timestamp: Instant::now(),
        })
    }

    pub fn open_file(&mut self, path: &PathBuf) -> Result<()> {
        self.buffer_manager.open_file(path, &self.syntax_highlighter)?;
        self.status_message = format!("Opened: {}", path.display());

        // Update git cache when opening a file
        self.update_git_cache();

        // Don't initialize split_manager here - only when actually splitting

        Ok(())
    }

    pub fn split_horizontal(&mut self) -> Result<()> {
        // Initialize split manager if needed
        if self.split_manager.is_none() {
            let terminal_size = crossterm::terminal::size()?;
            let sidebar_width = if self.show_sidebar { 20 } else { 0 };
            self.split_manager = Some(SplitManager::new(
                self.buffer_manager.current_index,
                terminal_size.0,
                terminal_size.1.saturating_sub(2),
                sidebar_width,
            ));

            // Now perform the split with the same buffer index
            if let Some(split_manager) = &mut self.split_manager {
                if split_manager.split_current(SplitDirection::Horizontal, self.buffer_manager.current_index) {
                    self.status_message = String::from("Split horizontally");
                } else {
                    self.status_message = String::from("Failed to split");
                }
            }
        } else {
            // Split manager already exists, perform split
            if let Some(split_manager) = &mut self.split_manager {
                // Use the same buffer index for the new pane
                if split_manager.split_current(SplitDirection::Horizontal, self.buffer_manager.current_index) {
                    self.status_message = String::from("Split horizontally");
                } else {
                    self.status_message = String::from("Failed to split");
                }
            }
        }
        Ok(())
    }

    pub fn split_vertical(&mut self) -> Result<()> {
        // Initialize split manager if needed
        if self.split_manager.is_none() {
            let terminal_size = crossterm::terminal::size()?;
            let sidebar_width = if self.show_sidebar { 20 } else { 0 };
            self.split_manager = Some(SplitManager::new(
                self.buffer_manager.current_index,
                terminal_size.0,
                terminal_size.1.saturating_sub(2),
                sidebar_width,
            ));

            // Now perform the split with the same buffer index
            if let Some(split_manager) = &mut self.split_manager {
                if split_manager.split_current(SplitDirection::Vertical, self.buffer_manager.current_index) {
                    self.status_message = String::from("Split vertically");
                } else {
                    self.status_message = String::from("Failed to split");
                }
            }
        } else {
            // Split manager already exists, perform split
            if let Some(split_manager) = &mut self.split_manager {
                // Use the same buffer index for the new pane
                if split_manager.split_current(SplitDirection::Vertical, self.buffer_manager.current_index) {
                    self.status_message = String::from("Split vertically");
                } else {
                    self.status_message = String::from("Failed to split");
                }
            }
        }
        Ok(())
    }

    pub fn next_pane(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            split_manager.next_pane();
            self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);
        }
    }

    fn get_pane_buffer(&self, pane: &Pane) -> &TextBuffer {
        &self.buffer_manager.buffers[pane.buffer_index]
    }

    fn get_pane_buffer_mut(&mut self, pane: &Pane) -> &mut TextBuffer {
        &mut self.buffer_manager.buffers[pane.buffer_index]
    }

    fn get_active_buffer_mut(&mut self) -> Option<&mut TextBuffer> {
        if let Some(split_manager) = &mut self.split_manager {
            if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                let buffer_index = buffer_index;
                return Some(&mut self.buffer_manager.buffers[buffer_index]);
            }
        }
        Some(self.buffer_manager.current_mut())
    }

    fn update_active_pane_cursor(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                let buffer_index = buffer_index;
                let buffer = &self.buffer_manager.buffers[buffer_index];
                pane.cursor_x = buffer.cursor_position.1;
                pane.cursor_y = buffer.cursor_position.0;
                pane.adjust_viewport(buffer.cursor_position.0);
            }
        }
    }

    pub fn previous_pane(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            split_manager.previous_pane();
            self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);
        }
    }

    pub fn save_file(&mut self) -> Result<()> {
        if self.buffer_manager.current().file_path.is_none() {
            // For unnamed files, prompt for a filename
            self.status_message = String::from("Save as: Enter filename in command mode (:w filename)");
            self.mode = Mode::Command;
            self.command_buffer = String::from(":w ");
        } else {
            self.buffer_manager.current_mut().save()?;
            self.status_message = if let Some(path) = &self.buffer_manager.current().file_path {
                format!("Saved: {}", path.display())
            } else {
                String::from("Buffer saved")
            };

            // Update git cache after saving
            self.update_git_cache();

            // Refresh sidebar after saving to update Git status
            if let Some(sidebar) = &mut self.sidebar {
                sidebar.refresh()?;
            }
        }
        Ok(())
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key)?,
            Mode::Insert => self.handle_insert_mode(key)?,
            Mode::Visual => self.handle_visual_mode(key)?,
            Mode::Command => self.handle_command_mode(key)?,
            Mode::Search => self.handle_search_mode(key)?,
            Mode::Replace => self.handle_replace_mode(key)?,
            Mode::QuitConfirm => self.handle_quit_confirm_mode(key)?,
        }
        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        // Clear last_key if it's not 'd', 'y', or 'w' being pressed
        let should_keep_last_key = matches!(
            (key.code, key.modifiers),
            (KeyCode::Char('d'), KeyModifiers::NONE) |
            (KeyCode::Char('y'), KeyModifiers::NONE) |
            (KeyCode::Char('w'), KeyModifiers::CONTROL)
        );

        // Also keep last_key if we're processing a Ctrl+W command
        let processing_ctrl_w = matches!(self.last_key, Some(KeyCode::Char('w'))) && matches!(
            key.code,
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('v') | KeyCode::Char('V') |
            KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Char('h') | KeyCode::Char('l') |
            KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Left | KeyCode::Right |
            KeyCode::Up | KeyCode::Down
        );

        if !should_keep_last_key && !processing_ctrl_w {
            if self.last_key.is_some() {
                self.last_key = None;
                // Clear the command indicator from status
                if self.status_message == "d" || self.status_message == "y" || self.status_message == "^W" {
                    self.status_message.clear();
                }
            }
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => self.try_quit(),
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => self.save_file()?,
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                // Undo
                if self.buffer_manager.current_mut().undo() {
                    self.status_message = String::from("Undo");
                } else {
                    self.status_message = String::from("Nothing to undo");
                }
            }
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                // Redo
                if self.buffer_manager.current_mut().redo() {
                    self.status_message = String::from("Redo");
                } else {
                    self.status_message = String::from("Nothing to redo");
                }
            }
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.show_sidebar = !self.show_sidebar;
            }
            // Visual feedback toggles
            (KeyCode::Char('w'), KeyModifiers::ALT) => {
                // Toggle whitespace visualization
                self.config.editor.show_whitespace = !self.config.editor.show_whitespace;
                self.status_message = if self.config.editor.show_whitespace {
                    String::from("Whitespace visualization enabled")
                } else {
                    String::from("Whitespace visualization disabled")
                };
            }
            (KeyCode::Char('r'), KeyModifiers::ALT) => {
                // Toggle column rulers
                self.config.editor.show_column_ruler = !self.config.editor.show_column_ruler;
                self.status_message = if self.config.editor.show_column_ruler {
                    String::from("Column rulers enabled (80, 100, 120)")
                } else {
                    String::from("Column rulers disabled")
                };
            }
            (KeyCode::Char('i'), KeyModifiers::ALT) => {
                // Toggle indent guides
                self.config.editor.show_indent_guides = !self.config.editor.show_indent_guides;
                self.status_message = if self.config.editor.show_indent_guides {
                    String::from("Indent guides enabled")
                } else {
                    String::from("Indent guides disabled")
                };
            }
            // Line operations
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                // Duplicate line
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer_index = pane.buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.duplicate_line();
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().duplicate_line();
                    self.update_viewport();
                }
                self.status_message = String::from("Line duplicated");
            }
            (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                // Join lines
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer_index = pane.buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.join_lines();
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().join_lines();
                    self.update_viewport();
                }
                self.status_message = String::from("Lines joined");
            }
            (KeyCode::Up, KeyModifiers::ALT) => {
                // Move line up
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer_index = pane.buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_line_up();
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().move_line_up();
                    self.update_viewport();
                }
                self.status_message = String::from("Line moved up");
            }
            (KeyCode::Down, KeyModifiers::ALT) => {
                // Move line down
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer_index = pane.buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_line_down();
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().move_line_down();
                    self.update_viewport();
                }
                self.status_message = String::from("Line moved down");
            }
            // Split view commands (Ctrl+W followed by another key)
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                // Store 'w' as the last key to wait for next command
                self.last_key = Some(KeyCode::Char('w'));
                self.status_message = String::from("^W");
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                // Refresh sidebar
                if let Some(sidebar) = &mut self.sidebar {
                    sidebar.refresh()?;
                    self.status_message = String::from("Sidebar refreshed");
                }
            }
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                // Toggle sidebar mode between files and buffers
                if let Some(sidebar) = &mut self.sidebar {
                    sidebar.toggle_mode();

                    // If switching to buffer mode, load buffer list
                    if sidebar.mode == SidebarMode::Buffers {
                        let buffer_list = self.buffer_manager.get_buffer_info_list();
                        sidebar.load_buffer_list(buffer_list);
                        self.status_message = String::from("Showing buffers");
                    } else {
                        // Refresh file list when switching back
                        sidebar.refresh()?;
                        self.status_message = String::from("Showing files");
                    }
                }
            }
            // CUA bindings
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => self.cut()?,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.copy()?,
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => self.paste()?,
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => self.select_all(),
            (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                // Delete from cursor to end of line (like Ctrl+K in many editors)
                let deleted_text = self.buffer_manager.current_mut().delete_to_end_of_line();
                if !deleted_text.is_empty() {
                    // Store in clipboard
                    if let Some(ref mut clipboard) = self.clipboard {
                        let _ = clipboard.set_text(deleted_text);
                    }
                    self.status_message = String::from("Deleted to end of line");
                }
            }
            // Search and replace
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Search;
                self.search_query.clear();
                self.search_matches.clear();
                let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                self.status_message = format!("Search [{}]: ", case_mode);
            }
            (KeyCode::Char('/'), KeyModifiers::CONTROL) |
            (KeyCode::Char('_'), KeyModifiers::CONTROL) |
            (KeyCode::Char('/'), KeyModifiers::ALT) |
            (KeyCode::Char('7'), KeyModifiers::CONTROL) => {
                // Toggle comment for current line or selection
                // Multiple keybindings for terminal compatibility:
                // Ctrl+/ (standard), Ctrl+_ (some terminals), Alt+/ (alternative), Ctrl+7 (some mappings)
                self.toggle_comment();
            }
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Replace;
                self.search_query.clear();
                self.replace_text.clear();
                self.search_matches.clear();
                let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                self.status_message = format!("Search for [{}]: ", case_mode);
            }
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                self.mode = Mode::Insert;
                self.status_message.clear();
            }
            (KeyCode::Char('v'), KeyModifiers::NONE) if !matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                self.mode = Mode::Visual;
                self.status_message.clear();
            }
            (KeyCode::Char(':'), KeyModifiers::NONE) => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
            }
            // Selection with Shift+Arrow keys
            (KeyCode::Left, mods) if mods.contains(KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                if mods.contains(KeyModifiers::CONTROL) {
                    self.buffer_manager.current_mut().move_cursor_word_left();
                } else {
                    self.buffer_manager.current_mut().move_cursor_left();
                }
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Right, mods) if mods.contains(KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                if mods.contains(KeyModifiers::CONTROL) {
                    self.buffer_manager.current_mut().move_cursor_word_right();
                } else {
                    self.buffer_manager.current_mut().move_cursor_right();
                }
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Up, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_up();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            (KeyCode::Down, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_down();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            // Handle split commands after Ctrl+W
            (KeyCode::Char('s'), KeyModifiers::NONE) | (KeyCode::Char('S'), KeyModifiers::SHIFT) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Horizontal split
                self.split_horizontal()?;
                self.last_key = None;
            }
            (KeyCode::Char('v'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Vertical split
                self.split_vertical()?;
                self.last_key = None;
            }
            (KeyCode::Char('V'), KeyModifiers::SHIFT) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Vertical split (capital V)
                self.split_vertical()?;
                self.last_key = None;
            }
            (KeyCode::Char('w'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Cycle through panes
                self.next_pane();
                self.last_key = None;
            }
            (KeyCode::Char('W'), KeyModifiers::SHIFT) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Cycle backwards through panes
                self.previous_pane();
                self.last_key = None;
            }
            (KeyCode::Char('h'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to left pane (for now just cycle)
                self.previous_pane();
                self.last_key = None;
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to right pane (for now just cycle)
                self.next_pane();
                self.last_key = None;
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to below pane (for now just cycle)
                self.next_pane();
                self.last_key = None;
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to above pane (for now just cycle)
                self.previous_pane();
                self.last_key = None;
            }
            (KeyCode::Left, KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to left pane with arrow key
                self.previous_pane();
                self.last_key = None;
            }
            (KeyCode::Right, KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to right pane with arrow key
                self.next_pane();
                self.last_key = None;
            }
            (KeyCode::Down, KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to below pane with arrow key
                self.next_pane();
                self.last_key = None;
            }
            (KeyCode::Up, KeyModifiers::NONE) if matches!(self.last_key, Some(KeyCode::Char('w'))) => {
                // Move to above pane with arrow key
                self.previous_pane();
                self.last_key = None;
            }
            // Vim-style dd command (delete line)
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                // Check if the last key was also 'd' for dd command
                if matches!(self.last_key, Some(KeyCode::Char('d'))) {
                    // Delete the current line
                    let deleted_line = self.buffer_manager.current_mut().delete_line();
                    // Store in clipboard
                    if let Some(ref mut clipboard) = self.clipboard {
                        let _ = clipboard.set_text(deleted_line);
                    }
                    self.status_message = String::from("Line deleted");
                    self.last_key = None; // Reset
                } else if !matches!(self.last_key, Some(KeyCode::Char('w'))) {
                    // Only set 'd' as last key if we're not in the middle of a Ctrl+W command
                    self.last_key = Some(KeyCode::Char('d'));
                    self.status_message = String::from("d");
                }
            }
            // Vim-style yy command (yank/copy line)
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                // Check if the last key was also 'y' for yy command
                if matches!(self.last_key, Some(KeyCode::Char('y'))) {
                    // Yank (copy) the current line
                    let yanked_line = self.buffer_manager.current().yank_line();
                    // Store in clipboard
                    if let Some(ref mut clipboard) = self.clipboard {
                        let _ = clipboard.set_text(yanked_line);
                    }
                    self.status_message = String::from("Line yanked");
                    self.last_key = None; // Reset
                } else {
                    // Store 'y' as the last key
                    self.last_key = Some(KeyCode::Char('y'));
                    self.status_message = String::from("y");
                }
            }
            // Normal movement (clears selection)
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, KeyModifiers::NONE) => {
                if self.show_sidebar && self.sidebar.is_some() {
                    self.show_sidebar = false;
                } else {
                    // Handle movement in split views or normal buffer
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            let buffer_index = buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.move_cursor_left();
                            buffer.clear_selection();
                            pane.cursor_x = buffer.cursor_position.1;
                            pane.cursor_y = buffer.cursor_position.0;
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().move_cursor_left();
                        self.buffer_manager.current_mut().clear_selection();
                    }
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, KeyModifiers::NONE) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_down();
                    }
                } else {
                    // Handle movement in split views or normal buffer
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            let buffer_index = buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.move_cursor_down();
                            buffer.clear_selection();
                            pane.cursor_x = buffer.cursor_position.1;
                            pane.cursor_y = buffer.cursor_position.0;
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().move_cursor_down();
                        self.buffer_manager.current_mut().clear_selection();
                        self.update_viewport();
                    }
                }
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, KeyModifiers::NONE) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_up();
                    }
                } else {
                    // Handle movement in split views or normal buffer
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            let buffer_index = buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.move_cursor_up();
                            buffer.clear_selection();
                            pane.cursor_x = buffer.cursor_position.1;
                            pane.cursor_y = buffer.cursor_position.0;
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().move_cursor_up();
                        self.buffer_manager.current_mut().clear_selection();
                        self.update_viewport();
                    }
                }
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, KeyModifiers::NONE) => {
                if self.show_sidebar && self.sidebar.is_some() {
                    if let Some(sidebar) = &mut self.sidebar {
                        if sidebar.get_selected_path().is_some() {
                            sidebar.toggle_expanded()?;
                        }
                    }
                } else {
                    // Handle movement in split views or normal buffer
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            let buffer_index = buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.move_cursor_right();
                            buffer.clear_selection();
                            pane.cursor_x = buffer.cursor_position.1;
                            pane.cursor_y = buffer.cursor_position.0;
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().move_cursor_right();
                        self.buffer_manager.current_mut().clear_selection();
                    }
                }
            }
            // Word movement with Ctrl
            (KeyCode::Left, KeyModifiers::CONTROL) => {
                self.buffer_manager.current_mut().move_cursor_word_left();
                self.buffer_manager.current_mut().clear_selection();
            }
            (KeyCode::Right, KeyModifiers::CONTROL) => {
                self.buffer_manager.current_mut().move_cursor_word_right();
                self.buffer_manager.current_mut().clear_selection();
            }
            (KeyCode::Enter, _) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        // Check if in buffer mode
                        if sidebar.mode == SidebarMode::Buffers {
                            if let Some(buffer_index) = sidebar.get_selected_buffer_index() {
                                self.buffer_manager.go_to_buffer(buffer_index);
                                self.show_sidebar = false;
                                self.status_message = format!("Switched to buffer {}", buffer_index + 1);
                            }
                        } else {
                            // File mode
                            // Check if ".." is selected (parent directory)
                            if sidebar.is_parent_selected() {
                                sidebar.navigate_to_parent()?;
                                self.update_git_cache();
                                return Ok(());
                            }

                            if let Some(path) = sidebar.get_selected_path() {
                                let path = path.clone();
                                if path.is_file() {
                                    // Handle file that may no longer exist
                                    match self.open_file(&path) {
                                        Ok(_) => {
                                            self.show_sidebar = false;
                                        }
                                        Err(e) => {
                                            self.status_message = format!("Error opening file: {}", e);
                                            // Don't hide sidebar on error so user can select another file
                                        }
                                    }
                                } else {
                                    if let Some(sidebar) = &mut self.sidebar {
                                        sidebar.toggle_expanded()?;
                                        // Update git cache when navigating into directories
                                        self.update_git_cache();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            (KeyCode::Char('0'), KeyModifiers::NONE) => self.buffer_manager.current_mut().move_to_line_start(),
            (KeyCode::Char('$'), KeyModifiers::NONE) => self.buffer_manager.current_mut().move_to_line_end(),
            // Tab handling in normal mode
            (KeyCode::BackTab, _) => {
                // Shift+Tab (BackTab) - unindent selection or current line
                if self.buffer_manager.current().has_selection() {
                    self.buffer_manager.current_mut().unindent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                } else {
                    // Unindent current line without moving cursor
                    let saved_col = self.buffer_manager.current().cursor_position.1;
                    let saved_row = self.buffer_manager.current().cursor_position.0;

                    // Create temporary selection for the line
                    self.buffer_manager.current_mut().cursor_position = (saved_row, 0);
                    self.buffer_manager.current_mut().start_selection();
                    self.buffer_manager.current_mut().cursor_position = (saved_row, self.buffer_manager.current().get_line(saved_row).len());
                    self.buffer_manager.current_mut().update_selection();

                    // Unindent and get the amount removed
                    let removed = self.buffer_manager.current_mut().unindent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                    self.buffer_manager.current_mut().clear_selection();

                    // Restore cursor position, adjusting by the amount unindented
                    self.buffer_manager.current_mut().cursor_position = (saved_row, saved_col.saturating_sub(removed));
                }
            }
            (KeyCode::Tab, _) => {
                // Tab - indent selection or current line
                if self.buffer_manager.current().has_selection() {
                    self.buffer_manager.current_mut().indent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                } else {
                    // Indent current line without losing cursor position
                    let saved_col = self.buffer_manager.current().cursor_position.1;
                    let saved_row = self.buffer_manager.current().cursor_position.0;

                    // Create temporary selection for the line
                    self.buffer_manager.current_mut().cursor_position = (saved_row, 0);
                    self.buffer_manager.current_mut().start_selection();
                    self.buffer_manager.current_mut().cursor_position = (saved_row, self.buffer_manager.current().get_line(saved_row).len());
                    self.buffer_manager.current_mut().update_selection();

                    // Indent
                    self.buffer_manager.current_mut().indent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                    self.buffer_manager.current_mut().clear_selection();

                    // Restore cursor position (adjusted for indent)
                    let indent_amount = if self.config.editor.use_spaces { self.config.editor.tab_width } else { 1 };
                    self.buffer_manager.current_mut().cursor_position = (saved_row, saved_col + indent_amount);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.status_message.clear();
                self.buffer_manager.current_mut().clear_selection();
            }
            // CUA bindings in insert mode
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => self.cut()?,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.copy()?,
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => self.paste()?,
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => self.select_all(),
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                // Undo
                if self.buffer_manager.current_mut().undo() {
                    self.status_message = String::from("Undo");
                } else {
                    self.status_message = String::from("Nothing to undo");
                }
            }
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                // Redo
                if self.buffer_manager.current_mut().redo() {
                    self.status_message = String::from("Redo");
                } else {
                    self.status_message = String::from("Nothing to redo");
                }
            }
            // Comment toggling with Ctrl+/
            (KeyCode::Char('/'), KeyModifiers::CONTROL) => {
                self.toggle_comment();
            }
            // Some terminals send Ctrl+/ as Ctrl+_ (ASCII 31)
            (KeyCode::Char('_'), KeyModifiers::CONTROL) => {
                self.toggle_comment();
            }
            // Alternative: Alt+/ for terminals where Ctrl+/ doesn't work
            (KeyCode::Char('/'), KeyModifiers::ALT) => {
                self.toggle_comment();
            }
            // Also try Ctrl+7 (some terminals map this)
            (KeyCode::Char('7'), KeyModifiers::CONTROL) => {
                self.toggle_comment();
            }
            // Selection with Shift+Arrow keys in insert mode
            (KeyCode::Left, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_left();
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Right, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_right();
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Up, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_up();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            (KeyCode::Down, KeyModifiers::SHIFT) => {
                if self.buffer_manager.current().selection.is_none() {
                    self.buffer_manager.current_mut().start_selection();
                }
                self.buffer_manager.current_mut().move_cursor_down();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            // Typing replaces selection
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                // Auto-close brackets and quotes
                let auto_close_char = match c {
                    '(' => Some(')'),
                    '[' => Some(']'),
                    '{' => Some('}'),
                    '"' => Some('"'),
                    '\'' => Some('\''),
                    '`' => Some('`'),
                    _ => None,
                };

                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        let buffer_index = buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.delete_selection();
                        buffer.insert_char(c);

                        // Auto-close if applicable
                        if let Some(close_char) = auto_close_char {
                            // For quotes, only auto-close if not already inside quotes
                            if matches!(c, '"' | '\'' | '`') {
                                // Simple heuristic: only auto-close if there's whitespace or nothing after cursor
                                let (row, col) = buffer.cursor_position;
                                let line = buffer.content.line(row);
                                if col >= line.len_chars() || line.char(col) == ' ' || line.char(col) == '\n' {
                                    let cursor_pos = buffer.cursor_position;
                                    buffer.insert_char(close_char);
                                    buffer.cursor_position = cursor_pos; // Move cursor back between the quotes
                                }
                            } else {
                                // Always auto-close brackets
                                let cursor_pos = buffer.cursor_position;
                                buffer.insert_char(close_char);
                                buffer.cursor_position = cursor_pos; // Move cursor back between the brackets
                            }
                        }

                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().delete_selection();
                    self.buffer_manager.current_mut().insert_char(c);

                    // Auto-close if applicable
                    if let Some(close_char) = auto_close_char {
                        // For quotes, only auto-close if not already inside quotes
                        if matches!(c, '"' | '\'' | '`') {
                            // Simple heuristic: only auto-close if there's whitespace or nothing after cursor
                            let buffer = self.buffer_manager.current();
                            let (row, col) = buffer.cursor_position;
                            let line = buffer.content.line(row);
                            if col >= line.len_chars() || line.char(col) == ' ' || line.char(col) == '\n' {
                                let cursor_pos = self.buffer_manager.current().cursor_position;
                                self.buffer_manager.current_mut().insert_char(close_char);
                                self.buffer_manager.current_mut().cursor_position = cursor_pos; // Move cursor back between the quotes
                            }
                        } else {
                            // Always auto-close brackets
                            let cursor_pos = self.buffer_manager.current().cursor_position;
                            self.buffer_manager.current_mut().insert_char(close_char);
                            self.buffer_manager.current_mut().cursor_position = cursor_pos; // Move cursor back between the brackets
                        }
                    }
                }
            }
            (KeyCode::Enter, _) => {
                // Get indentation before any mutable borrows
                let indent = if let Some(split_manager) = &self.split_manager {
                    if let Some(index) = split_manager.get_active_buffer_index() {
                        let buffer = &self.buffer_manager.buffers[index];
                        self.get_smart_indent(buffer)
                    } else {
                        0
                    }
                } else {
                    let buffer = self.buffer_manager.current();
                    self.get_smart_indent(buffer)
                };

                // Now apply the changes
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer = &mut self.buffer_manager.buffers[pane.buffer_index];
                        buffer.delete_selection();
                        buffer.insert_char('\n');

                        // Apply smart indentation
                        for _ in 0..indent {
                            buffer.insert_char(' ');
                        }

                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().delete_selection();
                    self.buffer_manager.current_mut().insert_char('\n');

                    for _ in 0..indent {
                        self.buffer_manager.current_mut().insert_char(' ');
                    }
                }
            }
            (KeyCode::Backspace, _) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer_index = pane.buffer_index;
                        let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        if buffer.selection.is_some() {
                            buffer.delete_selection();
                        } else {
                            buffer.delete_char();
                        }
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    if self.buffer_manager.current().selection.is_some() {
                        self.buffer_manager.current_mut().delete_selection();
                    } else {
                        self.buffer_manager.current_mut().delete_char();
                    }
                }
            }
            (KeyCode::Delete, _) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        if buffer.selection.is_some() {
                            buffer.delete_selection();
                        } else {
                            buffer.delete_forward();
                        }
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    if self.buffer_manager.current().selection.is_some() {
                        self.buffer_manager.current_mut().delete_selection();
                    } else {
                        self.buffer_manager.current_mut().delete_forward();
                    }
                }
            }
            (KeyCode::BackTab, _) => {
                // Shift+Tab (BackTab) - unindent selection or current line
                if self.buffer_manager.current().has_selection() {
                    self.buffer_manager.current_mut().unindent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                } else {
                    // Unindent current line without moving cursor
                    let saved_col = self.buffer_manager.current().cursor_position.1;
                    let saved_row = self.buffer_manager.current().cursor_position.0;

                    // Create temporary selection for the line
                    self.buffer_manager.current_mut().cursor_position = (saved_row, 0);
                    self.buffer_manager.current_mut().start_selection();
                    self.buffer_manager.current_mut().cursor_position = (saved_row, self.buffer_manager.current().get_line(saved_row).len());
                    self.buffer_manager.current_mut().update_selection();

                    // Unindent and get the amount removed
                    let removed = self.buffer_manager.current_mut().unindent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                    self.buffer_manager.current_mut().clear_selection();

                    // Restore cursor position, adjusting by the amount unindented
                    self.buffer_manager.current_mut().cursor_position = (saved_row, saved_col.saturating_sub(removed));
                }
            }
            (KeyCode::Tab, _) => {
                // Tab - indent selection or insert tab
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        if buffer.has_selection() {
                            buffer.indent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                        } else {
                            // Normal tab insertion
                            if self.config.editor.use_spaces {
                                for _ in 0..self.config.editor.tab_width {
                                    buffer.insert_char(' ');
                                }
                            } else {
                                buffer.insert_char('\t');
                            }
                        }
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    if self.buffer_manager.current().has_selection() {
                        self.buffer_manager.current_mut().indent_selection(self.config.editor.use_spaces, self.config.editor.tab_width);
                    } else {
                        // Normal tab insertion
                        if self.config.editor.use_spaces {
                            for _ in 0..self.config.editor.tab_width {
                                self.buffer_manager.current_mut().insert_char(' ');
                            }
                        } else {
                            self.buffer_manager.current_mut().insert_char('\t');
                        }
                    }
                }
            }
            (KeyCode::Left, KeyModifiers::NONE) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_cursor_left();
                        buffer.clear_selection();
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_left();
                    self.buffer_manager.current_mut().clear_selection();
                }
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_cursor_right();
                        buffer.clear_selection();
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_right();
                    self.buffer_manager.current_mut().clear_selection();
                }
            }
            (KeyCode::Up, KeyModifiers::NONE) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_cursor_up();
                        buffer.clear_selection();
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_up();
                    self.buffer_manager.current_mut().clear_selection();
                    self.update_viewport();
                }
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                if let Some(split_manager) = &mut self.split_manager {
                    if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                        buffer.move_cursor_down();
                        buffer.clear_selection();
                        pane.cursor_x = buffer.cursor_position.1;
                        pane.cursor_y = buffer.cursor_position.0;
                        pane.adjust_viewport(buffer.cursor_position.0);
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_down();
                    self.buffer_manager.current_mut().clear_selection();
                    self.update_viewport();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_visual_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.status_message.clear();
                self.buffer_manager.current_mut().selection = None;
            }
            (KeyCode::Char('/'), KeyModifiers::CONTROL) |
            (KeyCode::Char('_'), KeyModifiers::CONTROL) |
            (KeyCode::Char('/'), KeyModifiers::ALT) |
            (KeyCode::Char('7'), KeyModifiers::CONTROL) => {
                // Toggle comment for selection
                // Multiple keybindings for terminal compatibility
                self.toggle_comment();
                // Stay in visual mode to allow further operations
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_command_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.status_message.clear();
            }
            KeyCode::Enter => {
                self.execute_command()?;
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Char(c) => self.command_buffer.push(c),
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn execute_command(&mut self) -> Result<()> {
        // Remove leading colon if present and clone to avoid borrow issues
        let command = if self.command_buffer.starts_with(':') {
            self.command_buffer[1..].to_string()
        } else {
            self.command_buffer.clone()
        };

        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        // Try session commands first
        let args = if parts.len() > 1 { &parts[1..] } else { &[] };
        if let Ok(handled) = crate::session_commands::execute_session_command(self, parts[0], args) {
            if handled {
                return Ok(());
            }
        }

        match parts[0] {
            "q" | "quit" => self.try_quit(),
            "w" | "write" => {
                if parts.len() > 1 {
                    // Save with a specific filename (save as)
                    let path = PathBuf::from(parts[1]);
                    self.buffer_manager.current_mut().save_as(&path)?;
                    self.status_message = format!("Saved: {}", path.display());
                } else if self.buffer_manager.current().file_path.is_some() {
                    // Save existing file
                    self.buffer_manager.current_mut().save()?;
                    if let Some(path) = &self.buffer_manager.current().file_path {
                        self.status_message = format!("Saved: {}", path.display());
                    }
                } else {
                    // No filename provided for new file
                    self.status_message = String::from("No filename provided. Use :w filename");
                }

                // Refresh sidebar after saving to update Git status
                if let Some(sidebar) = &mut self.sidebar {
                    sidebar.refresh()?;
                }
            }
            "wq" => {
                if parts.len() > 1 {
                    // Save as with filename then quit
                    let path = PathBuf::from(parts[1]);
                    self.buffer_manager.current_mut().save_as(&path)?;
                    // Refresh sidebar after saving
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.refresh()?;
                    }
                    self.should_quit = true;
                } else if self.buffer_manager.current().file_path.is_some() {
                    self.buffer_manager.current_mut().save()?;
                    // Refresh sidebar after saving
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.refresh()?;
                    }
                    self.should_quit = true;
                } else {
                    self.status_message = String::from("No filename provided. Use :wq filename");
                }
            }
            "e" | "edit" if parts.len() > 1 => {
                let path = PathBuf::from(parts[1]);
                if let Err(e) = self.open_file(&path) {
                    self.status_message = format!("Error opening file: {}", e);
                }
            }
            "bn" | "bnext" => {
                if self.buffer_manager.buffer_count() > 1 {
                    self.buffer_manager.next_buffer();
                    if let Some(path) = &self.buffer_manager.current().file_path {
                        self.status_message = format!("Switched to: {}", path.display());
                    } else {
                        self.status_message = String::from("Switched to: [No Name]");
                    }
                } else {
                    self.status_message = String::from("No next buffer");
                }
            }
            "bp" | "bprevious" => {
                if self.buffer_manager.buffer_count() > 1 {
                    self.buffer_manager.previous_buffer();
                    if let Some(path) = &self.buffer_manager.current().file_path {
                        self.status_message = format!("Switched to: {}", path.display());
                    } else {
                        self.status_message = String::from("Switched to: [No Name]");
                    }
                } else {
                    self.status_message = String::from("No previous buffer");
                }
            }
            "bd" | "bdelete" => {
                if self.buffer_manager.buffer_count() > 1 {
                    if let Some(path) = &self.buffer_manager.current().file_path {
                        let filename = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "[No Name]".to_string());
                        self.buffer_manager.close_current()?;
                        self.status_message = format!("Closed: {}", filename);
                    } else {
                        self.buffer_manager.close_current()?;
                        self.status_message = String::from("Closed: [No Name]");
                    }
                } else {
                    self.status_message = String::from("Cannot close last buffer");
                }
            }
            "ls" | "buffers" => {
                let buffer_list = self.buffer_manager.get_buffer_list();
                self.status_message = format!("Buffers: {}", buffer_list.join(", "));
            }
            "sort" => {
                // Sort selected lines in ascending order
                if self.buffer_manager.current().has_selection() {
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.sort_selected_lines(true);
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().sort_selected_lines(true);
                        self.update_viewport();
                    }
                    self.status_message = String::from("Lines sorted (ascending)");
                } else {
                    self.status_message = String::from("No selection - select lines to sort");
                }
            }
            "sort!" => {
                // Sort selected lines in descending order
                if self.buffer_manager.current().has_selection() {
                    if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            let buffer_index = pane.buffer_index;
                            let buffer = &mut self.buffer_manager.buffers[buffer_index];
                            buffer.sort_selected_lines(false);
                            pane.adjust_viewport(buffer.cursor_position.0);
                        }
                    } else {
                        self.buffer_manager.current_mut().sort_selected_lines(false);
                        self.update_viewport();
                    }
                    self.status_message = String::from("Lines sorted (descending)");
                } else {
                    self.status_message = String::from("No selection - select lines to sort");
                }
            }
            _ => {
                self.status_message = format!("Unknown command: {}", parts[0]);
            }
        }

        Ok(())
    }

    fn update_viewport(&mut self) {
        let viewport_height = 20;
        let cursor_row = self.buffer_manager.current().cursor_position.0;

        if cursor_row < self.viewport_offset {
            self.viewport_offset = cursor_row;
        } else if cursor_row >= self.viewport_offset + viewport_height {
            self.viewport_offset = cursor_row - viewport_height + 1;
        }
    }

    fn copy(&mut self) -> Result<()> {
        if let Some(text) = self.buffer_manager.current().get_selected_text() {
            if let Some(ref mut clipboard) = self.clipboard {
                match clipboard.set_text(text) {
                    Ok(_) => self.status_message = String::from("Copied to clipboard"),
                    Err(_) => self.status_message = String::from("Failed to copy to clipboard"),
                }
            } else {
                self.status_message = String::from("Clipboard not available");
            }
        }
        Ok(())
    }

    fn cut(&mut self) -> Result<()> {
        if let Some(text) = self.buffer_manager.current_mut().delete_selection() {
            if let Some(ref mut clipboard) = self.clipboard {
                match clipboard.set_text(text) {
                    Ok(_) => self.status_message = String::from("Cut to clipboard"),
                    Err(_) => self.status_message = String::from("Failed to cut to clipboard"),
                }
            } else {
                self.status_message = String::from("Clipboard not available");
            }
        }
        Ok(())
    }

    fn paste(&mut self) -> Result<()> {
        if let Some(ref mut clipboard) = self.clipboard {
            if let Ok(text) = clipboard.get_text() {
                // Delete any selected text first
                self.buffer_manager.current_mut().delete_selection();
                // Insert clipboard content
                self.buffer_manager.current_mut().insert_str(&text);
                self.status_message = String::from("Pasted from clipboard");
            }
        } else {
            self.status_message = String::from("Clipboard not available");
        }
        Ok(())
    }

    fn select_all(&mut self) {
        self.buffer_manager.current_mut().cursor_position = (0, 0);
        self.buffer_manager.current_mut().start_selection();
        // Move to end of document
        let last_line = self.buffer_manager.current().line_count().saturating_sub(1);
        let last_col = self.buffer_manager.current().get_line(last_line).len().saturating_sub(1);
        self.buffer_manager.current_mut().cursor_position = (last_line, last_col);
        self.buffer_manager.current_mut().update_selection();
        self.status_message = String::from("Selected all");
    }

    fn try_quit(&mut self) {
        // Check for unsaved buffers
        let unsaved = self.buffer_manager.get_modified_buffers();

        if unsaved.is_empty() {
            self.should_quit = true;
        } else {
            // Store unsaved buffers and enter quit confirm mode
            self.unsaved_buffers_to_check = unsaved.clone();
            self.mode = Mode::QuitConfirm;

            // Get list of unsaved files
            let unsaved_files: Vec<String> = unsaved.iter()
                .map(|&idx| {
                    let buffer = &self.buffer_manager.buffers[idx];
                    if let Some(path) = &buffer.file_path {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("[Unknown]")
                            .to_string()
                    } else {
                        format!("[Buffer {}]", idx + 1)
                    }
                })
                .collect();

            self.status_message = format!(
                "Save modified buffers? {} unsaved: {} (y/n/c)",
                unsaved.len(),
                unsaved_files.join(", ")
            );
        }
    }

    fn handle_search_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.search_matches.clear();
                self.status_message.clear();
            }
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                // Toggle case sensitivity
                self.case_sensitive = !self.case_sensitive;
                let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };

                // Re-search with new case sensitivity if we have a query
                if !self.search_query.is_empty() {
                    self.search_matches = self.buffer_manager.current().find_all_matches(&self.search_query, self.case_sensitive);
                    let count = self.search_matches.len();
                    self.status_message = format!("Search [{}]: {} ({} matches)", case_mode, self.search_query, count);
                } else {
                    self.status_message = format!("Search [{}]: ", case_mode);
                }
            }
            (KeyCode::Enter, _) => {
                // Perform search
                if !self.search_query.is_empty() {
                    // Find all matches for highlighting
                    self.search_matches = self.buffer_manager.current().find_all_matches(&self.search_query, self.case_sensitive);

                    let start_pos = self.buffer_manager.current().cursor_position;
                    if let Some(found) = self.buffer_manager.current().find_next(&self.search_query, start_pos, self.case_sensitive) {
                        self.buffer_manager.current_mut().cursor_position = found;
                        self.last_search_position = Some(found);

                        // Select the found text
                        self.buffer_manager.current_mut().start_selection();
                        let end_col = found.1 + self.search_query.len();
                        self.buffer_manager.current_mut().cursor_position = (found.0, end_col);
                        self.buffer_manager.current_mut().update_selection();

                        // Adjust viewport to show the result
                        self.update_viewport();

                        let match_count = self.search_matches.len();
                        let current_match = self.search_matches.iter()
                            .position(|(r, c, _)| *r == found.0 && *c == found.1)
                            .map(|i| i + 1)
                            .unwrap_or(0);
                        self.status_message = format!("Found: {} ({}/{})", self.search_query, current_match, match_count);
                    } else {
                        self.status_message = format!("Not found: {}", self.search_query);
                    }
                }
                self.mode = Mode::Normal;
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.search_query.push(c);
                // Update search matches as user types for live preview
                if !self.search_query.is_empty() {
                    self.search_matches = self.buffer_manager.current().find_all_matches(&self.search_query, self.case_sensitive);
                    let count = self.search_matches.len();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search [{}]: {} ({} matches)", case_mode, self.search_query, count);
                } else {
                    self.search_matches.clear();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search [{}]: {}", case_mode, self.search_query);
                }
            }
            (KeyCode::Backspace, _) => {
                self.search_query.pop();
                // Update search matches as user types for live preview
                if !self.search_query.is_empty() {
                    self.search_matches = self.buffer_manager.current().find_all_matches(&self.search_query, self.case_sensitive);
                    let count = self.search_matches.len();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search [{}]: {} ({} matches)", case_mode, self.search_query, count);
                } else {
                    self.search_matches.clear();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search [{}]: {}", case_mode, self.search_query);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_replace_mode(&mut self, key: KeyEvent) -> Result<()> {
        // Check if we're in the second phase (entering replacement text)
        let entering_replacement = self.search_query.contains('\0');

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.replace_text.clear();
                self.search_matches.clear();
                self.status_message.clear();
            }
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                // Toggle case sensitivity
                self.case_sensitive = !self.case_sensitive;
                let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };

                // Only show status if not in confirmation mode
                if self.search_query.matches('\0').count() < 2 {
                    if !entering_replacement {
                        self.status_message = format!("Search for [{}]: {}", case_mode, self.search_query.trim_end_matches('\0'));
                    } else {
                        // Re-find matches with new case sensitivity
                        let search_text = self.search_query.trim_end_matches('\0');
                        self.search_matches = self.buffer_manager.current().find_all_matches(search_text, self.case_sensitive);
                        self.status_message = format!(
                            "Replace '{}' with [{}] ({} matches): {}",
                            search_text,
                            case_mode,
                            self.search_matches.len(),
                            self.replace_text
                        );
                    }
                }
            }
            (KeyCode::Enter, _) => {
                if !entering_replacement {
                    // First Enter - finish search query, highlight matches, move to replacement
                    if !self.search_query.is_empty() {
                        // Find all matches and highlight them
                        self.search_matches = self.buffer_manager.current().find_all_matches(&self.search_query, self.case_sensitive);

                        if self.search_matches.is_empty() {
                            self.status_message = format!("Not found: {}", self.search_query);
                            self.mode = Mode::Normal;
                        } else {
                            // Move cursor to first match
                            if let Some((row, col, _)) = self.search_matches.first() {
                                self.buffer_manager.current_mut().cursor_position = (*row, *col);
                                self.update_viewport();
                            }

                            // Mark that we're entering replacement text with a null separator
                            self.search_query.push('\0');
                            self.status_message = format!(
                                "Replace '{}' with ({} matches): {}",
                                self.search_query.trim_end_matches('\0'),
                                self.search_matches.len(),
                                self.replace_text
                            );
                        }
                    }
                } else {
                    // Second Enter - show replace options
                    let search_text = self.search_query.trim_end_matches('\0');
                    self.status_message = format!(
                        "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit",
                        search_text,
                        self.replace_text
                    );

                    // Set a flag to indicate we're waiting for replace confirmation
                    self.search_query.push('\0'); // Add another marker
                }
            }
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                // Check if we're waiting for replace confirmation (two null markers)
                if self.search_query.matches('\0').count() >= 2 {
                    let search_text = self.search_query.split('\0').next().unwrap_or("").to_string();
                    match c {
                        'y' | 'Y' => {
                            // Replace current occurrence and move to next
                            if let Some((row, col, _)) = self.search_matches.first().copied() {
                                self.buffer_manager.current_mut().cursor_position = (row, col);
                                self.buffer_manager.current_mut().replace(&search_text, &self.replace_text, false);

                                // Remove this match and find next
                                self.search_matches.remove(0);

                                // Update matches positions after replacement
                                let len_diff = self.replace_text.len() as i32 - search_text.len() as i32;
                                for (match_row, match_col, match_end) in &mut self.search_matches {
                                    if *match_row == row && *match_col > col {
                                        *match_col = (*match_col as i32 + len_diff).max(0) as usize;
                                        *match_end = (*match_end as i32 + len_diff).max(0) as usize;
                                    }
                                }

                                if !self.search_matches.is_empty() {
                                    // Move to next match
                                    if let Some((next_row, next_col, _)) = self.search_matches.first() {
                                        self.buffer_manager.current_mut().cursor_position = (*next_row, *next_col);
                                        self.update_viewport();
                                    }
                                    self.status_message = format!(
                                        "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit ({} left)",
                                        search_text,
                                        self.replace_text,
                                        self.search_matches.len()
                                    );
                                } else {
                                    self.status_message = format!("Replaced all occurrences");
                                    self.mode = Mode::Normal;
                                    self.search_matches.clear();
                                }
                            }
                        }
                        'n' | 'N' => {
                            // Skip current and move to next
                            if !self.search_matches.is_empty() {
                                self.search_matches.remove(0);
                                if !self.search_matches.is_empty() {
                                    if let Some((next_row, next_col, _)) = self.search_matches.first() {
                                        self.buffer_manager.current_mut().cursor_position = (*next_row, *next_col);
                                        self.update_viewport();
                                    }
                                    self.status_message = format!(
                                        "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit ({} left)",
                                        search_text,
                                        self.replace_text,
                                        self.search_matches.len()
                                    );
                                } else {
                                    self.status_message = String::from("No more matches");
                                    self.mode = Mode::Normal;
                                    self.search_matches.clear();
                                }
                            }
                        }
                        'a' | 'A' => {
                            // Replace all remaining
                            let mut replaced_count = 0;
                            let matches = self.search_matches.clone();

                            // Process matches in reverse to avoid position issues
                            for (row, col, _) in matches.iter().rev() {
                                self.buffer_manager.current_mut().cursor_position = (*row, *col);
                                self.buffer_manager.current_mut().replace(&search_text, &self.replace_text, false);
                                replaced_count += 1;
                            }

                            self.status_message = format!("Replaced {} occurrences", replaced_count);
                            self.mode = Mode::Normal;
                            self.search_matches.clear();
                        }
                        'q' | 'Q' => {
                            // Quit replace mode
                            self.mode = Mode::Normal;
                            self.search_matches.clear();
                            self.status_message = String::from("Replace cancelled");
                        }
                        _ => {
                            self.status_message = format!(
                                "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit",
                                search_text,
                                self.replace_text
                            );
                        }
                    }
                } else if !entering_replacement {
                    // Entering search text
                    self.search_query.push(c);
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search for [{}]: {}", case_mode, self.search_query);
                } else {
                    // Entering replacement text
                    self.replace_text.push(c);
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!(
                        "Replace '{}' with [{}] ({} matches): {}",
                        self.search_query.trim_end_matches('\0'),
                        case_mode,
                        self.search_matches.len(),
                        self.replace_text
                    );
                }
            }
            (KeyCode::Backspace, _) => {
                if self.search_query.matches('\0').count() >= 2 {
                    // In confirmation mode, don't allow backspace
                    return Ok(());
                } else if !entering_replacement {
                    self.search_query.pop();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!("Search for [{}]: {}", case_mode, self.search_query);
                } else {
                    self.replace_text.pop();
                    let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                    self.status_message = format!(
                        "Replace '{}' with [{}] ({} matches): {}",
                        self.search_query.trim_end_matches('\0'),
                        case_mode,
                        self.search_matches.len(),
                        self.replace_text
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_quit_confirm_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Save all unsaved buffers
                for &idx in &self.unsaved_buffers_to_check {
                    let buffer = &mut self.buffer_manager.buffers[idx];
                    if buffer.file_path.is_some() {
                        buffer.save()?;
                    } else {
                        // For unnamed buffers, we need to prompt for a name
                        // For now, skip them and warn
                        self.status_message = String::from("Cannot save unnamed buffers. Use :w filename first");
                        self.mode = Mode::Normal;
                        return Ok(());
                    }
                }
                self.should_quit = true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Quit without saving
                self.should_quit = true;
            }
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                // Cancel quit
                self.mode = Mode::Normal;
                self.status_message = String::from("Quit cancelled");
                self.unsaved_buffers_to_check.clear();
            }
            _ => {
                self.status_message = format!(
                    "Save modified buffers? (y)es / (n)o / (c)ancel"
                );
            }
        }
        Ok(())
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Calculate the position in the editor area
                let col = mouse.column as usize;
                let row = mouse.row as usize;

                // Check if click is in sidebar
                if self.show_sidebar && col < self.config.sidebar.width as usize {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.handle_click(row);

                        // Handle sidebar item selection
                        if sidebar.mode == SidebarMode::Files {
                            if let Some(selected_item) = sidebar.get_selected_item() {
                                match selected_item {
                                    crate::sidebar::SidebarItem::File(path) => {
                                        // Handle file that may no longer exist
                                        match self.buffer_manager.open_file(&path, &self.syntax_highlighter) {
                                            Ok(_) => {
                                                self.status_message = format!("Opened: {}", path.display());
                                            }
                                            Err(e) => {
                                                self.status_message = format!("Error opening file: {}", e);
                                            }
                                        }
                                    }
                                    crate::sidebar::SidebarItem::Directory(path, _expanded) => {
                                        sidebar.toggle_directory(&path)?;
                                    }
                                    crate::sidebar::SidebarItem::Parent => {
                                        sidebar.navigate_to_parent()?;
                                        self.update_git_cache();
                                    }
                                }
                            }
                        } else if sidebar.mode == SidebarMode::Buffers {
                            let selected_idx = sidebar.selected_index;
                            if selected_idx > 0 {
                                let buffer_idx = selected_idx - 1;
                                if self.buffer_manager.go_to_buffer(buffer_idx) {
                                    self.status_message = format!("Switched to buffer {}", buffer_idx + 1);
                                }
                            }
                        }
                    }
                } else {
                    // Click in editor area - handle split views
                    if let Some(split_manager) = &mut self.split_manager {
                        // Find which pane was clicked
                        if split_manager.handle_click(col as u16, row as u16) {
                            // Successfully switched to clicked pane
                            self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);

                            // Now handle click within that pane
                            if let Some(pane) = split_manager.get_active_pane() {
                                // Check if click is within pane bounds
                                if col as u16 >= pane.x && (col as u16) < pane.x + pane.width &&
                                   row as u16 >= pane.y && (row as u16) < pane.y + pane.height {

                                    let pane_col = (col as u16 - pane.x) as usize;
                                    let pane_row = (row as u16 - pane.y) as usize;

                                    // Account for line numbers (if shown)
                                    let line_number_width = if self.config.editor.show_line_numbers {
                                        5 // Fixed width for line numbers
                                    } else {
                                        0
                                    };

                                    let content_col = pane_col.saturating_sub(line_number_width);
                                    let content_row = pane_row + pane.viewport_offset;

                                    // Set cursor position if within content bounds
                                    let buffer_index = pane.buffer_index;
                                    let buffer = &mut self.buffer_manager.buffers[buffer_index];
                                    if content_row < buffer.content.len_lines() {
                                        let line = buffer.content.line(content_row);
                                        let line_len = line.len_chars().saturating_sub(1);
                                        let actual_col = content_col.min(line_len);

                                        buffer.cursor_position = (content_row, actual_col);
                                        pane.cursor_x = actual_col;
                                        pane.cursor_y = content_row;
                                        buffer.clear_selection();
                                    }
                                }
                            }
                        }
                    } else {
                        // No split manager, use original single-pane logic
                        let editor_col = if self.show_sidebar {
                            col.saturating_sub(self.config.sidebar.width as usize + 1)
                        } else {
                            col
                        };

                        // Account for line numbers (if shown)
                        let line_number_width = if self.config.editor.show_line_numbers {
                            let max_line = self.buffer_manager.current().content.len_lines();
                            format!("{}", max_line).len() + 2
                        } else {
                            0
                        };

                        let content_col = editor_col.saturating_sub(line_number_width);
                        // Don't subtract 1 from row - the mouse coordinates are already 0-based
                        let content_row = row + self.viewport_offset;

                        // Set cursor position if within content bounds
                        let buffer = self.buffer_manager.current_mut();
                        if content_row < buffer.content.len_lines() {
                            let line = buffer.content.line(content_row);
                            let line_len = line.len_chars().saturating_sub(1);
                            let actual_col = content_col.min(line_len);

                            buffer.cursor_position = (content_row, actual_col);
                            buffer.clear_selection();
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Handle text selection with mouse drag
                if !self.show_sidebar || mouse.column >= self.config.sidebar.width {
                    let editor_col = if self.show_sidebar {
                        mouse.column.saturating_sub(self.config.sidebar.width + 1)
                    } else {
                        mouse.column
                    } as usize;

                    let line_number_width = if self.config.editor.show_line_numbers {
                        let max_line = self.buffer_manager.current().content.len_lines();
                        format!("{}", max_line).len() + 2
                    } else {
                        0
                    };

                    let content_col = editor_col.saturating_sub(line_number_width);
                    // Don't subtract 1 from row - the mouse coordinates are already 0-based
                    let content_row = (mouse.row as usize) + self.viewport_offset;

                    let buffer = self.buffer_manager.current_mut();
                    if content_row < buffer.content.len_lines() {
                        if buffer.selection.is_none() {
                            buffer.start_selection();
                        }

                        let line = buffer.content.line(content_row);
                        let line_len = line.len_chars().saturating_sub(1);
                        let actual_col = content_col.min(line_len);

                        buffer.cursor_position = (content_row, actual_col);
                        buffer.update_selection();
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                // Scroll down
                let max_offset = self.buffer_manager.current().content.len_lines()
                    .saturating_sub(10);
                if self.viewport_offset < max_offset {
                    self.viewport_offset += 3;
                }
            }
            MouseEventKind::ScrollUp => {
                // Scroll up
                self.viewport_offset = self.viewport_offset.saturating_sub(3);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        // Don't update git cache on every draw - it's too expensive

        let _theme = self.theme_manager.get_current_theme();
        let size = frame.area();

        let main_layout = if self.show_sidebar {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(self.config.sidebar.width),
                    Constraint::Min(0),
                ])
                .split(size)
                .to_vec()
        } else {
            vec![Rect::default(), size]
        };

        if self.show_sidebar {
            self.draw_sidebar(frame, main_layout[0]);
        }

        let editor_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(main_layout[1]);

        // Use split manager if available, otherwise draw single editor
        if self.split_manager.is_some() {
            self.draw_splits(frame, editor_layout[0]);
        } else {
            self.draw_editor(frame, editor_layout[0]);
        }

        self.draw_status_bar(frame, editor_layout[1]);

        if self.mode == Mode::Command {
            self.draw_command_line(frame, editor_layout[2]);
        } else {
            self.draw_message_line(frame, editor_layout[2]);
        }
    }

    fn draw_splits(&mut self, frame: &mut Frame, area: Rect) {
        // Recursively draw all panes
        let active_index = self.split_manager.as_ref().map(|sm| sm.active_pane_index).unwrap_or(0);

        // Temporarily take ownership to avoid borrow conflicts
        let mut split_manager = self.split_manager.take();

        if let Some(ref mut sm) = split_manager {
            Self::draw_split_node_static(self, frame, area, &mut sm.root, active_index);
        }

        // Restore the split manager
        self.split_manager = split_manager;
    }

    fn draw_split_node_static(app: &mut App, frame: &mut Frame, area: Rect, node: &mut crate::split::SplitNode, active_index: usize) {
        use crate::split::SplitNode;

        match node {
            SplitNode::Leaf(pane) => {
                // Draw the pane
                app.draw_pane(frame, area, pane, active_index == 0);
            }
            SplitNode::Split { direction, ratio, first, second } => {
                use crate::split::SplitDirection;

                let (first_area, second_area) = match direction {
                    SplitDirection::Horizontal => {
                        let first_height = (area.height as f32 * *ratio) as u16;
                        (
                            Rect::new(area.x, area.y, area.width, first_height),
                            Rect::new(area.x, area.y + first_height, area.width, area.height - first_height),
                        )
                    }
                    SplitDirection::Vertical => {
                        let first_width = (area.width as f32 * *ratio) as u16;
                        (
                            Rect::new(area.x, area.y, first_width, area.height),
                            Rect::new(area.x + first_width, area.y, area.width - first_width, area.height),
                        )
                    }
                };

                // Count panes in first subtree to determine active index for second
                let first_pane_count = App::count_panes(first);

                if active_index < first_pane_count {
                    App::draw_split_node_static(app, frame, first_area, first, active_index);
                    App::draw_split_node_static(app, frame, second_area, second, usize::MAX); // Not active
                } else {
                    App::draw_split_node_static(app, frame, first_area, first, usize::MAX); // Not active
                    App::draw_split_node_static(app, frame, second_area, second, active_index - first_pane_count);
                }
            }
        }
    }

    fn count_panes(node: &crate::split::SplitNode) -> usize {
        use crate::split::SplitNode;
        match node {
            SplitNode::Leaf(_) => 1,
            SplitNode::Split { first, second, .. } => {
                Self::count_panes(first) + Self::count_panes(second)
            }
        }
    }

    fn draw_pane(&mut self, frame: &mut Frame, area: Rect, pane: &mut Pane, is_active: bool) {
        let theme = self.theme_manager.get_current_theme();

        // Get the buffer from the buffer_manager
        let buffer = &self.buffer_manager.buffers[pane.buffer_index];

        // Draw border around pane
        let border_style = if is_active {
            Style::default().fg(ratatui::style::Color::Green)
        } else {
            Style::default().fg(ratatui::style::Color::Gray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Update pane dimensions
        pane.x = inner_area.x;
        pane.y = inner_area.y;
        pane.width = inner_area.width;
        pane.height = inner_area.height;

        // Draw the buffer content with syntax highlighting
        let viewport_height = inner_area.height as usize;
        let lines = buffer.get_visible_lines(pane.viewport_offset, viewport_height);
        let mut paragraph_lines = Vec::new();

        // Get syntax definition if available
        let syntax = if let Some(syntax_name) = &buffer.syntax_name {
            self.syntax_highlighter.find_syntax_by_name(syntax_name)
        } else if let Some(path) = &buffer.file_path {
            self.syntax_highlighter.detect_syntax(path)
        } else {
            None
        };

        for (i, line) in lines.iter().enumerate() {
            let line_number = pane.viewport_offset + i + 1;
            let mut spans = Vec::new();

            if self.config.editor.show_line_numbers {
                spans.push(ratatui::text::Span::styled(
                    format!("{:4} ", line_number),
                    get_ui_style(theme, "line_numbers"),
                ));
            }

            let cursor_pos = buffer.cursor_position;
            let cursor_row = cursor_pos.0;
            let is_current_line = pane.viewport_offset + i == cursor_row && is_active;

            // Check for matching bracket at cursor position
            let matching_bracket = if is_active && cursor_row == pane.viewport_offset + i && self.config.editor.highlight_matching_bracket {
                buffer.find_matching_bracket(cursor_pos)
            } else {
                None
            };

            // Build spans character by character to handle selection
            let row = pane.viewport_offset + i;

            // Calculate indent level for indent guides
            let indent_level = if self.config.editor.show_indent_guides {
                line.chars().take_while(|c| *c == ' ' || *c == '\t').count() / self.config.editor.tab_width
            } else {
                0
            };

            // Apply syntax highlighting if available and no selection
            if buffer.selection.is_none() && syntax.is_some() {
                if let Some(syntax) = syntax {
                    if let Ok(highlighted) = self.syntax_highlighter.highlight_line(line, syntax) {
                        let mut current_col = 0;
                        let line_chars: Vec<char> = line.chars().collect();

                        for (style, text) in highlighted {
                            for ch in text.chars() {
                                let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                                let mut ratatui_style = Style::default();

                                // Check if this position matches the bracket under cursor or its match
                                let is_matching_bracket = matching_bracket
                                    .map_or(false, |(match_row, match_col)|
                                        match_row == row && match_col == current_col
                                    );
                                let is_cursor_bracket = is_active && cursor_row == row && cursor_pos.1 == current_col;

                                // Check for column ruler
                                let is_column_ruler = self.config.editor.show_column_ruler &&
                                    self.config.editor.column_ruler_positions.contains(&current_col);

                                // Check for whitespace visualization
                                let display_char = if self.config.editor.show_whitespace {
                                    match ch {
                                        ' ' => '·',
                                        '\t' => '→',
                                        _ => ch,
                                    }
                                } else {
                                    ch
                                };

                                // Check for trailing whitespace
                                let is_trailing_whitespace = self.config.editor.show_whitespace &&
                                    (ch == ' ' || ch == '\t') &&
                                    current_col >= line.trim_end().len();

                                // Check for indent guide
                                let is_indent_guide = self.config.editor.show_indent_guides &&
                                    current_col % self.config.editor.tab_width == 0 &&
                                    current_col < indent_level * self.config.editor.tab_width &&
                                    ch == ' ';

                                if is_bracket && self.config.editor.rainbow_brackets {
                                    // Get bracket depth for rainbow coloring
                                    let depth = buffer.get_bracket_depth_at((row, current_col));
                                    ratatui_style = ratatui_style.fg(self.get_rainbow_color(depth));

                                    // Highlight matching brackets
                                    if is_matching_bracket || is_cursor_bracket {
                                        ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(80, 80, 80))
                                            .add_modifier(ratatui::style::Modifier::BOLD);
                                    }
                                } else if is_trailing_whitespace {
                                    // Highlight trailing whitespace
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Red)
                                        .bg(ratatui::style::Color::Rgb(60, 20, 20));
                                } else if is_indent_guide {
                                    // Draw indent guide
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                                    spans.push(Span::styled("│", ratatui_style));
                                    current_col += 1;
                                    continue;
                                } else if is_column_ruler {
                                    // Highlight column ruler position
                                    ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(40, 40, 40));
                                } else if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                    // Dim whitespace characters
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                                } else {
                                    // Apply syntax highlighting color
                                    let fg = style.foreground;
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(
                                        fg.r,
                                        fg.g,
                                        fg.b,
                                    ));

                                    // Apply style modifiers
                                    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                                        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::BOLD);
                                    }
                                    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                                        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::ITALIC);
                                    }
                                    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                                        ratatui_style = ratatui_style.add_modifier(ratatui::style::Modifier::UNDERLINED);
                                    }
                                }

                                // Apply current line highlighting
                                if is_current_line && self.config.editor.highlight_current_line {
                                    if !is_matching_bracket && !is_cursor_bracket && !is_column_ruler {
                                        ratatui_style = ratatui_style.bg(hex_to_color(&theme.ui.current_line));
                                    }
                                }

                                spans.push(Span::styled(display_char.to_string(), ratatui_style));
                                current_col += 1;
                            }
                        }

                        // Add column rulers for positions beyond line length
                        if self.config.editor.show_column_ruler {
                            for &ruler_pos in &self.config.editor.column_ruler_positions {
                                if ruler_pos >= current_col {
                                    let spaces_to_ruler = ruler_pos - current_col;
                                    for _ in 0..spaces_to_ruler {
                                        spans.push(Span::styled(" ", Style::default()));
                                        current_col += 1;
                                    }
                                    if ruler_pos == current_col {
                                        spans.push(Span::styled("│", Style::default()
                                            .fg(ratatui::style::Color::Rgb(60, 60, 60))));
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback to simple rendering
                        spans.push(ratatui::text::Span::raw(line.to_string()));
                    }
                }
            } else if buffer.selection.is_some() {
                // Render with selection support
                let mut col = 0;
                for ch in line.chars() {
                    let is_selected = buffer.is_position_selected(row, col);
                    let mut style = get_ui_style(theme, "foreground");

                    // Check for whitespace visualization
                    let display_char = if self.config.editor.show_whitespace {
                        match ch {
                            ' ' => '·',
                            '\t' => '→',
                            _ => ch,
                        }
                    } else {
                        ch
                    };

                    // Check for indent guide
                    let is_indent_guide = self.config.editor.show_indent_guides &&
                        col % self.config.editor.tab_width == 0 &&
                        col < indent_level * self.config.editor.tab_width &&
                        ch == ' ';

                    if is_indent_guide {
                        style = style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        if is_selected {
                            style = style.bg(hex_to_color(&theme.ui.selection));
                        } else if is_current_line && self.config.editor.highlight_current_line {
                            style = style.bg(hex_to_color(&theme.ui.current_line));
                        }

                        if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                            style = style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                        }

                        spans.push(Span::styled(display_char.to_string(), style));
                    }
                    col += 1;
                }
            } else {
                // Simple text rendering without syntax highlighting
                let mut col = 0;
                for ch in line.chars() {
                    let mut style = get_ui_style(theme, "foreground");

                    // Check for whitespace visualization
                    let display_char = if self.config.editor.show_whitespace {
                        match ch {
                            ' ' => '·',
                            '\t' => '→',
                            _ => ch,
                        }
                    } else {
                        ch
                    };

                    // Check for indent guide
                    let is_indent_guide = self.config.editor.show_indent_guides &&
                        col % self.config.editor.tab_width == 0 &&
                        col < indent_level * self.config.editor.tab_width &&
                        ch == ' ';

                    if is_indent_guide {
                        style = style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        if is_current_line && self.config.editor.highlight_current_line {
                            style = style.bg(hex_to_color(&theme.ui.current_line));
                        }

                        if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                            style = style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                        }

                        spans.push(Span::styled(display_char.to_string(), style));
                    }
                    col += 1;
                }
            }

            paragraph_lines.push(ratatui::text::Line::from(spans));
        }

        let paragraph = Paragraph::new(paragraph_lines);
        frame.render_widget(paragraph, inner_area);

        // Draw cursor if this is the active pane
        if is_active {
            let cursor_pos = buffer.cursor_position;
            let screen_row = cursor_pos.0.saturating_sub(pane.viewport_offset);
            let screen_col = cursor_pos.1 + if self.config.editor.show_line_numbers { 5 } else { 0 };

            if screen_row < viewport_height {
                frame.set_cursor_position((inner_area.x + screen_col as u16, inner_area.y + screen_row as u16));
            }
        }
    }

    fn draw_sidebar(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme_manager.get_current_theme();

        if let Some(sidebar) = &mut self.sidebar {
            sidebar.update_scroll(area.height as usize - 2);

            let items: Vec<ListItem> = sidebar
                .get_visible_entries(area.height as usize - 2)
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let indent = "  ".repeat(entry.level);
                    let icon = if entry.name == ".." {
                        "↑ "  // Special icon for parent directory
                    } else if entry.is_dir {
                        if entry.is_expanded { "▼ " } else { "▶ " }
                    } else {
                        "  "
                    };

                    let mut style = Style::default()
                        .fg(hex_to_color(&theme.ui.sidebar.foreground));

                    if sidebar.scroll_offset + i == sidebar.selected_index {
                        style = style.bg(hex_to_color(&theme.ui.sidebar.selected));
                    }

                    if let Some(git_status) = entry.git_status {
                        style = match git_status {
                            GitStatus::Modified => style.fg(hex_to_color(&theme.ui.sidebar.git_modified)),
                            GitStatus::Added => style.fg(hex_to_color(&theme.ui.sidebar.git_added)),
                            GitStatus::Deleted => style.fg(hex_to_color(&theme.ui.sidebar.git_deleted)),
                            _ => style,
                        };
                    }

                    ListItem::new(format!("{}{}{}", indent, icon, entry.name))
                        .style(style)
                })
                .collect();

            let title = match sidebar.mode {
                SidebarMode::Files => "Files",
                SidebarMode::Buffers => "Buffers",
            };

            let sidebar_widget = List::new(items)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::RIGHT)
                        .border_style(get_ui_style(theme, "border"))
                )
                .style(Style::default().bg(hex_to_color(&theme.ui.sidebar.background)));

            frame.render_widget(sidebar_widget, area);
        }
    }

    fn get_rainbow_color(&self, depth: usize) -> ratatui::style::Color {
        // Rainbow colors for different bracket depths
        let colors = [
            ratatui::style::Color::Rgb(255, 100, 100),  // Red
            ratatui::style::Color::Rgb(255, 200, 100),  // Orange
            ratatui::style::Color::Rgb(255, 255, 100),  // Yellow
            ratatui::style::Color::Rgb(100, 255, 100),  // Green
            ratatui::style::Color::Rgb(100, 200, 255),  // Blue
            ratatui::style::Color::Rgb(200, 100, 255),  // Purple
            ratatui::style::Color::Rgb(255, 100, 200),  // Pink
        ];
        colors[depth % colors.len()]
    }

    fn draw_editor(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.theme_manager.get_current_theme();
        let viewport_height = area.height as usize;

        let lines = self.buffer_manager.current().get_visible_lines(self.viewport_offset, viewport_height);
        let mut paragraph_lines = Vec::new();

        // Get syntax definition if available
        let syntax = if let Some(syntax_name) = &self.buffer_manager.current().syntax_name {
            self.syntax_highlighter.find_syntax_by_name(syntax_name)
        } else if let Some(path) = &self.buffer_manager.current().file_path {
            self.syntax_highlighter.detect_syntax(path)
        } else {
            None
        };

        for (i, line) in lines.iter().enumerate() {
            let line_number = self.viewport_offset + i + 1;
            let mut spans = Vec::new();

            if self.config.editor.show_line_numbers {
                spans.push(Span::styled(
                    format!("{:4} ", line_number),
                    get_ui_style(theme, "line_numbers"),
                ));
            }

            let cursor_pos = self.buffer_manager.current().cursor_position;
            let cursor_row = cursor_pos.0;
            let is_current_line = self.viewport_offset + i == cursor_row;

            // Check for matching bracket at cursor position
            let matching_bracket = if cursor_row == self.viewport_offset + i && self.config.editor.highlight_matching_bracket {
                self.buffer_manager.current().find_matching_bracket(cursor_pos)
            } else {
                None
            };

            // Build spans character by character to handle selection
            let row = self.viewport_offset + i;
            let mut col = 0;

            // Calculate indent level for indent guides
            let indent_level = if self.config.editor.show_indent_guides {
                line.chars().take_while(|c| *c == ' ' || *c == '\t').count() / self.config.editor.tab_width
            } else {
                0
            };

            // Simple rendering with selection support (no syntax highlighting for now when selection is active)
            if self.buffer_manager.current().selection.is_some() {
                for ch in line.chars() {
                    let is_selected = self.buffer_manager.current().is_position_selected(row, col);
                    let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                    let mut style = get_ui_style(theme, "foreground");

                    // Check for whitespace visualization
                    let display_char = if self.config.editor.show_whitespace {
                        match ch {
                            ' ' => '·',
                            '\t' => '→',
                            _ => ch,
                        }
                    } else {
                        ch
                    };

                    // Check for indent guide
                    let is_indent_guide = self.config.editor.show_indent_guides &&
                        col % self.config.editor.tab_width == 0 &&
                        col < indent_level * self.config.editor.tab_width &&
                        ch == ' ';

                    // Check if this position matches the bracket under cursor or its match
                    let is_matching_bracket = matching_bracket
                        .map_or(false, |(match_row, match_col)|
                            match_row == row && match_col == col
                        );
                    let is_cursor_bracket = cursor_row == row && cursor_pos.1 == col;

                    if is_indent_guide {
                        style = style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        if is_bracket && self.config.editor.rainbow_brackets && !is_selected {
                            // Get bracket depth for rainbow coloring
                            let depth = self.buffer_manager.current().get_bracket_depth_at((row, col));
                            style = style.fg(self.get_rainbow_color(depth));

                            // Highlight matching brackets
                            if is_matching_bracket || is_cursor_bracket {
                                style = style.bg(ratatui::style::Color::Rgb(80, 80, 80))
                                    .add_modifier(ratatui::style::Modifier::BOLD);
                            }
                        }

                        if is_selected {
                            style = style.bg(hex_to_color(&theme.ui.selection));
                        } else if is_current_line && self.config.editor.highlight_current_line {
                            if !is_matching_bracket && !is_cursor_bracket {
                                style = style.bg(hex_to_color(&theme.ui.current_line));
                            }
                        }

                        if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                            style = style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                        }

                        spans.push(Span::styled(display_char.to_string(), style));
                    }
                    col += 1;
                }
            } else {
                // Apply syntax highlighting if available and no selection
                if let Some(syntax) = syntax {
                    if let Ok(highlighted) = self.syntax_highlighter.highlight_line(line, syntax) {
                        let mut current_col = 0;
                        for (style, text) in highlighted {
                            for ch in text.chars() {
                                let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                                let mut ratatui_style = Style::default();

                                // Check if this position matches the bracket under cursor or its match
                                let is_matching_bracket = matching_bracket
                                    .map_or(false, |(match_row, match_col)|
                                        match_row == row && match_col == current_col
                                    );
                                let is_cursor_bracket = cursor_row == row && cursor_pos.1 == current_col;

                                // Check for column ruler
                                let is_column_ruler = self.config.editor.show_column_ruler &&
                                    self.config.editor.column_ruler_positions.contains(&current_col);

                                // Check for whitespace visualization
                                let display_char = if self.config.editor.show_whitespace {
                                    match ch {
                                        ' ' => '·',
                                        '\t' => '→',
                                        _ => ch,
                                    }
                                } else {
                                    ch
                                };

                                // Check for trailing whitespace
                                let is_trailing_whitespace = self.config.editor.show_whitespace &&
                                    (ch == ' ' || ch == '\t') &&
                                    current_col >= line.trim_end().len();

                                // Check for indent guide
                                let is_indent_guide = self.config.editor.show_indent_guides &&
                                    current_col % self.config.editor.tab_width == 0 &&
                                    current_col < indent_level * self.config.editor.tab_width &&
                                    ch == ' ';

                                if is_bracket && self.config.editor.rainbow_brackets {
                                    // Get bracket depth for rainbow coloring
                                    let depth = self.buffer_manager.current().get_bracket_depth_at((row, current_col));
                                    ratatui_style = ratatui_style.fg(self.get_rainbow_color(depth));

                                    // Highlight matching brackets
                                    if is_matching_bracket || is_cursor_bracket {
                                        ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(80, 80, 80))
                                            .add_modifier(ratatui::style::Modifier::BOLD);
                                    }
                                } else if is_trailing_whitespace {
                                    // Highlight trailing whitespace
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Red)
                                        .bg(ratatui::style::Color::Rgb(60, 20, 20));
                                } else if is_indent_guide {
                                    // Draw indent guide
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                                    spans.push(Span::styled("│", ratatui_style));
                                    current_col += 1;
                                    continue;
                                } else if is_column_ruler {
                                    // Highlight column ruler position
                                    ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(40, 40, 40));
                                } else if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                    // Dim whitespace characters
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                                } else {
                                    // Normal syntax highlighting
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(
                                        style.foreground.r,
                                        style.foreground.g,
                                        style.foreground.b,
                                    ));
                                }

                                if is_current_line && self.config.editor.highlight_current_line {
                                    if !is_matching_bracket && !is_cursor_bracket && !is_column_ruler {
                                        ratatui_style = ratatui_style.bg(hex_to_color(&theme.ui.current_line));
                                    }
                                }

                                spans.push(Span::styled(display_char.to_string(), ratatui_style));
                                current_col += 1;
                            }
                        }

                        // Add column rulers for positions beyond line length
                        if self.config.editor.show_column_ruler {
                            for &ruler_pos in &self.config.editor.column_ruler_positions {
                                if ruler_pos >= current_col {
                                    let spaces_to_ruler = ruler_pos - current_col;
                                    for _ in 0..spaces_to_ruler {
                                        spans.push(Span::styled(" ", Style::default()));
                                        current_col += 1;
                                    }
                                    if ruler_pos == current_col {
                                        spans.push(Span::styled("│", Style::default()
                                            .fg(ratatui::style::Color::Rgb(60, 60, 60))));
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback if highlighting fails
                        if is_current_line && self.config.editor.highlight_current_line {
                            spans.push(Span::styled(
                                line.clone(),
                                get_ui_style(theme, "current_line"),
                            ));
                        } else {
                            spans.push(Span::styled(
                                line.clone(),
                                get_ui_style(theme, "foreground"),
                            ));
                        }
                    }
                } else {
                    // No syntax highlighting available - still handle brackets and visual feedback
                    for ch in line.chars() {
                        let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                        let mut style = get_ui_style(theme, "foreground");

                        // Check for whitespace visualization
                        let display_char = if self.config.editor.show_whitespace {
                            match ch {
                                ' ' => '·',
                                '\t' => '→',
                                _ => ch,
                            }
                        } else {
                            ch
                        };

                        // Check for indent guide
                        let is_indent_guide = self.config.editor.show_indent_guides &&
                            col % self.config.editor.tab_width == 0 &&
                            col < indent_level * self.config.editor.tab_width &&
                            ch == ' ';

                        // Check if this position matches the bracket under cursor or its match
                        let is_matching_bracket = matching_bracket
                            .map_or(false, |(match_row, match_col)|
                                match_row == row && match_col == col
                            );
                        let is_cursor_bracket = cursor_row == row && cursor_pos.1 == col;

                        if is_indent_guide {
                            style = style.fg(ratatui::style::Color::Rgb(60, 60, 60));
                            spans.push(Span::styled("│", style));
                        } else {
                            if is_bracket && self.config.editor.rainbow_brackets {
                                // Get bracket depth for rainbow coloring
                                let depth = self.buffer_manager.current().get_bracket_depth_at((row, col));
                                style = style.fg(self.get_rainbow_color(depth));

                                // Highlight matching brackets
                                if is_matching_bracket || is_cursor_bracket {
                                    style = style.bg(ratatui::style::Color::Rgb(80, 80, 80))
                                        .add_modifier(ratatui::style::Modifier::BOLD);
                                }
                            }

                            if is_current_line && self.config.editor.highlight_current_line {
                                if !is_matching_bracket && !is_cursor_bracket {
                                    style = style.bg(hex_to_color(&theme.ui.current_line));
                                }
                            }

                            if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                style = style.fg(ratatui::style::Color::Rgb(80, 80, 80));
                            }

                            spans.push(Span::styled(display_char.to_string(), style));
                        }
                        col += 1;
                    }
                }
            }

            paragraph_lines.push(Line::from(spans));
        }

        let editor_widget = Paragraph::new(paragraph_lines)
            .style(Style::default().bg(hex_to_color(&theme.ui.background)))
            .wrap(Wrap { trim: false });

        frame.render_widget(editor_widget, area);

        if self.mode == Mode::Insert || self.mode == Mode::Normal {
            let cursor_col = if self.config.editor.show_line_numbers {
                self.buffer_manager.current().cursor_position.1 + 5
            } else {
                self.buffer_manager.current().cursor_position.1
            };

            let cursor_row = self.buffer_manager.current().cursor_position.0 - self.viewport_offset;

            if cursor_row < viewport_height {
                frame.set_cursor_position((
                    area.x + cursor_col as u16,
                    area.y + cursor_row as u16,
                ));
            }
        }
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme_manager.get_current_theme();

        // Mode indicator with consistent width
        let mode_str = match self.mode {
            Mode::Normal => " NOR ",
            Mode::Insert => " INS ",
            Mode::Visual => " VIS ",
            Mode::Command => " CMD ",
            Mode::Search => " SRC ",
            Mode::Replace => " REP ",
            Mode::QuitConfirm => " Q? ",
        };

        let mode_style = match self.mode {
            Mode::Normal => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.background))
                .bg(hex_to_color(&theme.ui.status_bar.mode_normal))
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.background))
                .bg(hex_to_color(&theme.ui.status_bar.mode_insert))
                .add_modifier(Modifier::BOLD),
            Mode::Visual => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.background))
                .bg(hex_to_color(&theme.ui.status_bar.mode_visual))
                .add_modifier(Modifier::BOLD),
            Mode::Command | Mode::Search | Mode::Replace => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.background))
                .bg(hex_to_color(&theme.ui.status_bar.foreground))
                .add_modifier(Modifier::BOLD),
            Mode::QuitConfirm => Style::default()
                .fg(ratatui::style::Color::White)
                .bg(ratatui::style::Color::Rgb(200, 50, 50))
                .add_modifier(Modifier::BOLD),
        };

        // Get file name or [No Name]
        let file_info = if let Some(path) = &self.buffer_manager.current().file_path {
            if let Some(file_name) = path.file_name() {
                file_name.to_string_lossy().to_string()
            } else {
                path.display().to_string()
            }
        } else {
            String::from("[No Name]")
        };

        // Modified indicator
        let modified = if self.buffer_manager.current().modified { " ●" } else { "" };

        // Buffer count if more than 1
        let buffer_info = if self.buffer_manager.buffer_count() > 1 {
            format!(" [{}/{}]",
                self.buffer_manager.current_buffer_index(),
                self.buffer_manager.buffer_count()
            )
        } else {
            String::new()
        };

        // File type/syntax
        let file_type = if let Some(syntax_name) = &self.buffer_manager.current().syntax_name {
            format!(" {} ", syntax_name.to_lowercase())
        } else {
            String::from(" text ")
        };

        // Cursor position - line:col
        let position = format!(
            " {}:{} ",
            self.buffer_manager.current().cursor_position.0 + 1,
            self.buffer_manager.current().cursor_position.1 + 1
        );

        // Git information (using cached values)
        let git_info = if self.git_repo.is_some() {
            if let Some(ref branch_name) = self.git_branch {
                let status_indicator = if let Some((staged_count, modified_count)) = self.git_status_cache {
                    if modified_count > 0 || staged_count > 0 {
                        format!(" +{}~{}", staged_count, modified_count)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                format!(" 󰊢 {}{} ", branch_name, status_indicator)
            } else {
                String::from(" 󰊢 no branch ")
            }
        } else {
            String::new()
        };

        // Line ending type (for future use, hardcoded for now)
        let line_ending = " LF ";

        // Calculate spacing
        let left_content = format!("{} {}{}{}", mode_str, file_info, modified, buffer_info);
        let right_content = format!("{}{}{}{}", git_info, file_type, line_ending, position);
        let left_len = left_content.chars().count();
        let right_len = right_content.chars().count();
        let total_len = left_len + right_len;

        let spacing = if total_len < area.width as usize {
            area.width as usize - total_len
        } else {
            1
        };

        let mut spans = vec![
            // Mode indicator
            Span::styled(mode_str, mode_style),
            // Separator
            Span::styled(
                " ",
                Style::default().bg(hex_to_color(&theme.ui.status_bar.background)),
            ),
            // File name
            Span::styled(
                format!("{}{}", file_info, modified),
                Style::default()
                    .fg(if self.buffer_manager.current().modified {
                        hex_to_color(&theme.ui.status_bar.mode_insert) // Use insert color for modified
                    } else {
                        hex_to_color(&theme.ui.status_bar.foreground)
                    })
                    .bg(hex_to_color(&theme.ui.status_bar.background))
                    .add_modifier(if self.buffer_manager.current().modified {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ),
            // Buffer info
            Span::styled(
                buffer_info,
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background))
                    .add_modifier(Modifier::DIM),
            ),
            // Spacing
            Span::styled(
                " ".repeat(spacing),
                Style::default().bg(hex_to_color(&theme.ui.status_bar.background)),
            ),
            // Git info
            Span::styled(
                git_info,
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.mode_visual)) // Use a distinct color for git info
                    .bg(hex_to_color(&theme.ui.status_bar.background))
                    .add_modifier(Modifier::BOLD),
            ),
            // File type
            Span::styled(
                file_type,
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background))
                    .add_modifier(Modifier::DIM),
            ),
            // Line ending
            Span::styled(
                line_ending,
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background))
                    .add_modifier(Modifier::DIM),
            ),
            // Position
            Span::styled(
                position,
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background)),
            ),
        ];

        let status_line = Line::from(spans);
        let status_widget = Paragraph::new(vec![status_line]);

        frame.render_widget(status_widget, area);
    }

    fn draw_command_line(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme_manager.get_current_theme();

        let command_text = format!(":{}", self.command_buffer);
        let command_widget = Paragraph::new(command_text.as_str())
            .style(
                Style::default()
                    .fg(hex_to_color(&theme.ui.foreground))
                    .bg(hex_to_color(&theme.ui.background)),
            );

        frame.render_widget(command_widget, area);

        frame.set_cursor_position((
            area.x + 1 + self.command_buffer.len() as u16,
            area.y,
        ));
    }

    fn draw_message_line(&self, frame: &mut Frame, area: Rect) {
        let theme = self.theme_manager.get_current_theme();

        let message = Paragraph::new(self.status_message.as_str())
            .style(Style::default()
                .fg(hex_to_color(&theme.ui.foreground))
                .bg(hex_to_color(&theme.ui.background)));

        frame.render_widget(message, area);
    }

    // Smart editing helper methods
    fn get_smart_indent(&self, buffer: &TextBuffer) -> usize {
        let (row, _) = buffer.cursor_position;

        // If we're on the first line, no indentation
        if row == 0 {
            return 0;
        }

        // Get the previous line's content
        let prev_line = buffer.get_line(row.saturating_sub(1));

        // Count leading spaces/tabs
        let mut indent = 0;
        for ch in prev_line.chars() {
            if ch == ' ' {
                indent += 1;
            } else if ch == '\t' {
                indent += self.config.editor.tab_width;
            } else {
                break;
            }
        }

        // Check if previous line ends with opening brace/bracket for language-aware indentation
        let trimmed = prev_line.trim_end();
        if trimmed.ends_with('{') || trimmed.ends_with('[') || trimmed.ends_with('(') {
            // Check for language-specific rules
            if let Some(path) = &buffer.file_path {
                if let Some(syntax) = self.syntax_highlighter.detect_syntax(path) {
                    let lang = syntax.name.to_lowercase();
                    // Add extra indentation for block start
                    if matches!(lang.as_str(), "rust" | "c" | "c++" | "java" | "javascript" | "typescript" | "go" | "python") {
                        indent += self.config.editor.tab_width;
                    }
                }
            } else {
                // Default to adding indentation for any opening brace
                indent += self.config.editor.tab_width;
            }
        }

        // Check for Python-style colon (simple heuristic)
        if trimmed.ends_with(':') {
            if let Some(path) = &buffer.file_path {
                if let Some(syntax) = self.syntax_highlighter.detect_syntax(path) {
                    if syntax.name.to_lowercase() == "python" {
                        indent += self.config.editor.tab_width;
                    }
                }
            }
        }

        indent
    }

    fn toggle_comment(&mut self) {
        // Get buffer index and comment syntax first
        let (buffer_index, comment_syntax, has_selection) = if let Some(split_manager) = &self.split_manager {
            if let Some(index) = split_manager.get_active_buffer_index() {
                let buffer = &self.buffer_manager.buffers[index];
                let syntax = self.get_comment_syntax_for_buffer(buffer);
                let has_sel = buffer.has_selection();
                (index, syntax, has_sel)
            } else {
                return;
            }
        } else {
            let index = self.buffer_manager.current_index;
            let buffer = &self.buffer_manager.buffers[index];
            let syntax = self.get_comment_syntax_for_buffer(buffer);
            let has_sel = buffer.has_selection();
            (index, syntax, has_sel)
        };

        // Now modify the buffer
        let buffer = &mut self.buffer_manager.buffers[buffer_index];

        if has_selection {
            // Toggle comments for selected lines
            Self::toggle_block_comment_impl(buffer, &comment_syntax);
            self.status_message = String::from("Block comment toggled");
        } else {
            // Toggle comment for current line
            let is_commented = Self::toggle_line_comment_impl(buffer, &comment_syntax);
            self.status_message = if is_commented {
                String::from("Line commented")
            } else {
                String::from("Line uncommented")
            };
        }
    }

    fn get_comment_syntax_for_buffer(&self, buffer: &TextBuffer) -> String {
        if let Some(path) = &buffer.file_path {
            if let Some(syntax) = self.syntax_highlighter.detect_syntax(path) {
                match syntax.name.to_lowercase().as_str() {
                    "rust" | "c" | "c++" | "java" | "javascript" | "typescript" | "go" => "//",
                    "python" | "ruby" | "shell" | "bash" | "yaml" | "toml" => "#",
                    "html" | "xml" => "<!--",
                    "css" | "scss" | "less" => "/*",
                    "sql" => "--",
                    "lua" => "--",
                    "vim" => "\"",
                    _ => "//"
                }.to_string()
            } else {
                "//".to_string()
            }
        } else {
            "//".to_string()
        }
    }

    fn toggle_line_comment_impl(buffer: &mut TextBuffer, comment_syntax: &str) -> bool {
        let (row, _) = buffer.cursor_position;
        let line = buffer.get_line(row);

        // Check if line is already commented
        let trimmed = line.trim_start();
        let is_commented = trimmed.starts_with(comment_syntax);

        let line_start_idx = buffer.content.line_to_char(row);

        if is_commented {
            // Remove comment
            let spaces_before = line.len() - trimmed.len();
            let remove_start = line_start_idx + spaces_before;
            let remove_end = remove_start + comment_syntax.len();

            // Also remove a space after comment if present
            let remove_end = if buffer.content.get_char(remove_end) == Some(' ') {
                remove_end + 1
            } else {
                remove_end
            };

            buffer.content.remove(remove_start..remove_end);

            // Adjust cursor position if needed
            if buffer.cursor_position.1 > spaces_before {
                let removed_len = remove_end - remove_start;
                buffer.cursor_position.1 = buffer.cursor_position.1.saturating_sub(removed_len);
            }
        } else {
            // Add comment at the beginning of the line (after leading whitespace)
            let spaces_before = line.len() - trimmed.len();
            let insert_pos = line_start_idx + spaces_before;
            let comment_str = format!("{} ", comment_syntax);

            buffer.content.insert(insert_pos, &comment_str);

            // Adjust cursor position
            if buffer.cursor_position.1 >= spaces_before {
                buffer.cursor_position.1 += comment_str.len();
            }
        }

        buffer.modified = true;
        !is_commented  // Return true if we added a comment, false if we removed one
    }

    fn toggle_block_comment_impl(buffer: &mut TextBuffer, comment_syntax: &str) {
        if let Some(ref selection) = buffer.selection.clone() {
            let start_line = selection.start.0;
            let end_line = selection.end.0;

            // Check if all lines are commented
            let mut all_commented = true;
            for line_num in start_line..=end_line {
                let line = buffer.get_line(line_num);
                let trimmed = line.trim_start();
                if !trimmed.is_empty() && !trimmed.starts_with(comment_syntax) {
                    all_commented = false;
                    break;
                }
            }

            // Toggle comments for each line in reverse order to maintain indices
            for line_num in (start_line..=end_line).rev() {
                let line = buffer.get_line(line_num);
                let trimmed = line.trim_start();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                let line_start_idx = buffer.content.line_to_char(line_num);
                let spaces_before = line.len() - trimmed.len();

                if all_commented {
                    // Remove comment
                    let remove_start = line_start_idx + spaces_before;
                    let remove_end = remove_start + comment_syntax.len();

                    // Also remove a space after comment if present
                    let remove_end = if buffer.content.get_char(remove_end) == Some(' ') {
                        remove_end + 1
                    } else {
                        remove_end
                    };

                    buffer.content.remove(remove_start..remove_end);
                } else {
                    // Add comment
                    let insert_pos = line_start_idx + spaces_before;
                    let comment_str = format!("{} ", comment_syntax);
                    buffer.content.insert(insert_pos, &comment_str);
                }
            }

            // Maintain selection after commenting
            buffer.clear_selection();
            buffer.cursor_position = (start_line, 0);
            buffer.start_selection();
            buffer.cursor_position = (end_line, buffer.get_line(end_line).len());
            buffer.update_selection();

            buffer.modified = true;
        }
    }
}
