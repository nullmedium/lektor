use crate::buffer_manager::BufferManager;
use crate::config::Config;
use crate::sidebar::{GitStatus, Sidebar, SidebarMode};
use crate::syntax::SyntaxHighlighter;
use crate::theme::{get_ui_style, hex_to_color, ThemeManager};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
}

pub struct App {
    pub config: Config,
    pub buffer_manager: BufferManager,
    pub sidebar: Option<Sidebar>,
    pub syntax_highlighter: SyntaxHighlighter,
    pub theme_manager: ThemeManager,
    pub mode: Mode,
    pub should_quit: bool,
    pub status_message: String,
    pub show_sidebar: bool,
    pub viewport_offset: usize,
    pub command_buffer: String,
    clipboard: Option<Clipboard>,
}

impl App {
    pub fn new(config: Config) -> Result<Self> {
        Self::new_with_dir(config, std::env::current_dir()?)
    }

    pub fn new_with_dir(config: Config, working_dir: PathBuf) -> Result<Self> {
        let mut theme_manager = ThemeManager::new();
        if !theme_manager.set_theme(&config.theme.name) {
            theme_manager.set_theme("Dark");
        }

        let sidebar = Sidebar::new(working_dir).ok();

        // Set the syntect theme from config
        let mut syntax_highlighter = SyntaxHighlighter::new();
        syntax_highlighter.set_theme(&config.theme.syntax_theme);

        // Try to initialize clipboard
        let clipboard = Clipboard::new().ok();

        Ok(Self {
            config,
            buffer_manager: BufferManager::new(),
            sidebar,
            syntax_highlighter,
            theme_manager,
            mode: Mode::Normal,
            should_quit: false,
            status_message: String::from("Ready"),
            show_sidebar: true,
            viewport_offset: 0,
            command_buffer: String::new(),
            clipboard,
        })
    }

    pub fn open_file(&mut self, path: &PathBuf) -> Result<()> {
        self.buffer_manager.open_file(path, &self.syntax_highlighter)?;
        self.status_message = format!("Opened: {}", path.display());
        Ok(())
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
        }
        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => self.should_quit = true,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => self.save_file()?,
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.show_sidebar = !self.show_sidebar;
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
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                self.mode = Mode::Insert;
                self.status_message = String::from("-- INSERT --");
            }
            (KeyCode::Char('v'), KeyModifiers::NONE) => {
                self.mode = Mode::Visual;
                self.status_message = String::from("-- VISUAL --");
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
            // Normal movement (clears selection)
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, KeyModifiers::NONE) => {
                if self.show_sidebar && self.sidebar.is_some() {
                    self.show_sidebar = false;
                } else {
                    self.buffer_manager.current_mut().move_cursor_left();
                    self.buffer_manager.current_mut().clear_selection();
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, KeyModifiers::NONE) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_down();
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_down();
                    self.buffer_manager.current_mut().clear_selection();
                    self.update_viewport();
                }
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, KeyModifiers::NONE) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_up();
                    }
                } else {
                    self.buffer_manager.current_mut().move_cursor_up();
                    self.buffer_manager.current_mut().clear_selection();
                    self.update_viewport();
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
                    self.buffer_manager.current_mut().move_cursor_right();
                    self.buffer_manager.current_mut().clear_selection();
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
                                return Ok(());
                            }

                            if let Some(path) = sidebar.get_selected_path() {
                                let path = path.clone();
                                if path.is_file() {
                                    self.open_file(&path)?;
                                    self.show_sidebar = false;
                                } else {
                                    if let Some(sidebar) = &mut self.sidebar {
                                        sidebar.toggle_expanded()?;
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
                self.status_message = String::from("-- NORMAL --");
                self.buffer_manager.current_mut().clear_selection();
            }
            // CUA bindings in insert mode
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => self.cut()?,
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.copy()?,
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => self.paste()?,
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => self.select_all(),
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
                self.buffer_manager.current_mut().delete_selection();
                self.buffer_manager.current_mut().insert_char(c);
            }
            (KeyCode::Enter, _) => {
                self.buffer_manager.current_mut().delete_selection();
                self.buffer_manager.current_mut().insert_char('\n');
            }
            (KeyCode::Backspace, _) => {
                if self.buffer_manager.current().selection.is_some() {
                    self.buffer_manager.current_mut().delete_selection();
                } else {
                    self.buffer_manager.current_mut().delete_char();
                }
            }
            (KeyCode::Delete, _) => {
                if self.buffer_manager.current().selection.is_some() {
                    self.buffer_manager.current_mut().delete_selection();
                } else {
                    self.buffer_manager.current_mut().delete_forward();
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
            (KeyCode::Left, KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_left();
                self.buffer_manager.current_mut().clear_selection();
            }
            (KeyCode::Right, KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_right();
                self.buffer_manager.current_mut().clear_selection();
            }
            (KeyCode::Up, KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_up();
                self.buffer_manager.current_mut().clear_selection();
                self.update_viewport();
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_down();
                self.buffer_manager.current_mut().clear_selection();
                self.update_viewport();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_visual_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status_message = String::from("-- NORMAL --");
                self.buffer_manager.current_mut().selection = None;
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
                self.status_message = String::from("-- NORMAL --");
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
        // Remove leading colon if present
        let command = if self.command_buffer.starts_with(':') {
            &self.command_buffer[1..]
        } else {
            &self.command_buffer
        };

        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "q" | "quit" => self.should_quit = true,
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
                self.open_file(&path)?;
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

    pub fn draw(&mut self, frame: &mut Frame) {
        let theme = self.theme_manager.get_current_theme();
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

        self.draw_editor(frame, editor_layout[0]);
        self.draw_status_bar(frame, editor_layout[1]);

        if self.mode == Mode::Command {
            self.draw_command_line(frame, editor_layout[2]);
        } else {
            self.draw_message_line(frame, editor_layout[2]);
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

            let cursor_row = self.buffer_manager.current().cursor_position.0;
            let is_current_line = self.viewport_offset + i == cursor_row;

            // Build spans character by character to handle selection
            let row = self.viewport_offset + i;
            let mut col = 0;

            // Simple rendering with selection support (no syntax highlighting for now when selection is active)
            if self.buffer_manager.current().selection.is_some() {
                for ch in line.chars() {
                    let is_selected = self.buffer_manager.current().is_position_selected(row, col);
                    let mut style = get_ui_style(theme, "foreground");

                    if is_selected {
                        style = style.bg(hex_to_color(&theme.ui.selection));
                    } else if is_current_line && self.config.editor.highlight_current_line {
                        style = style.bg(hex_to_color(&theme.ui.current_line));
                    }

                    spans.push(Span::styled(ch.to_string(), style));
                    col += 1;
                }
            } else {
                // Apply syntax highlighting if available and no selection
                if let Some(syntax) = syntax {
                    if let Ok(highlighted) = self.syntax_highlighter.highlight_line(line, syntax) {
                        for (style, text) in highlighted {
                            let mut ratatui_style = Style::default();

                            ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ));

                            if is_current_line && self.config.editor.highlight_current_line {
                                ratatui_style = ratatui_style.bg(hex_to_color(&theme.ui.current_line));
                            }

                            spans.push(Span::styled(text, ratatui_style));
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
                    // No syntax highlighting available
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

        let mode_str = match self.mode {
            Mode::Normal => " NORMAL ",
            Mode::Insert => " INSERT ",
            Mode::Visual => " VISUAL ",
            Mode::Command => " COMMAND ",
        };

        let mode_style = match self.mode {
            Mode::Normal => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.mode_normal))
                .bg(hex_to_color(&theme.ui.status_bar.background))
                .add_modifier(Modifier::BOLD),
            Mode::Insert => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.mode_insert))
                .bg(hex_to_color(&theme.ui.status_bar.background))
                .add_modifier(Modifier::BOLD),
            Mode::Visual => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.mode_visual))
                .bg(hex_to_color(&theme.ui.status_bar.background))
                .add_modifier(Modifier::BOLD),
            Mode::Command => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.foreground))
                .bg(hex_to_color(&theme.ui.status_bar.background)),
        };

        // Buffer info: [1/3] filename [+]
        let buffer_info = format!(
            "[{}/{}]",
            self.buffer_manager.current_buffer_index(),
            self.buffer_manager.buffer_count()
        );

        let file_info = if let Some(path) = &self.buffer_manager.current().file_path {
            format!(" {} ", path.display())
        } else {
            String::from(" [No Name] ")
        };

        let modified_indicator = if self.buffer_manager.current().modified { "[+]" } else { "" };

        let position = format!(
            " {}:{}",
            self.buffer_manager.current().cursor_position.0 + 1,
            self.buffer_manager.current().cursor_position.1 + 1
        );

        let mut spans = vec![
            Span::styled(mode_str, mode_style),
            Span::styled(
                format!(" {} ", buffer_info),
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background)),
            ),
            Span::styled(
                format!("{}{}", file_info, modified_indicator),
                Style::default()
                    .fg(hex_to_color(&theme.ui.status_bar.foreground))
                    .bg(hex_to_color(&theme.ui.status_bar.background)),
            ),
        ];

        let remaining_width = area.width as usize
            - mode_str.len()
            - file_info.len()
            - modified_indicator.len()
            - position.len();

        spans.push(Span::styled(
            " ".repeat(remaining_width),
            Style::default().bg(hex_to_color(&theme.ui.status_bar.background)),
        ));

        spans.push(Span::styled(
            position,
            Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.foreground))
                .bg(hex_to_color(&theme.ui.status_bar.background)),
        ));

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
}
