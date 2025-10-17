use crate::buffer_manager::BufferManager;
use crate::config::Config;
use crate::cursor::CursorManager;
use crate::sidebar::{GitStatus, Sidebar, SidebarMode};
use crate::syntax::SyntaxHighlighter;
use crate::theme::{get_ui_style, hex_to_color, ThemeManager};
use anyhow::Result;
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
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
    Search,
    Replace,
    QuitConfirm,
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
    search_query: String,
    replace_text: String,
    case_sensitive: bool,
    last_search_position: Option<(usize, usize)>,
    search_matches: Vec<(usize, usize, usize)>,
    unsaved_buffers_to_check: Vec<usize>,
    quit_confirmed: bool,
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
            search_query: String::new(),
            replace_text: String::new(),
            case_sensitive: false,
            last_search_position: None,
            search_matches: Vec::new(),
            unsaved_buffers_to_check: Vec::new(),
            quit_confirmed: false,
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
            Mode::Search => self.handle_search_mode(key)?,
            Mode::Replace => self.handle_replace_mode(key)?,
            Mode::QuitConfirm => self.handle_quit_confirm_mode(key)?,
        }
        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
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
            // Search and replace
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Search;
                self.search_query.clear();
                self.search_matches.clear();
                let case_mode = if self.case_sensitive { "Case" } else { "Ignore case" };
                self.status_message = format!("Search [{}]: ", case_mode);
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
                self.status_message = String::from("-- NORMAL --");
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
                self.status_message = String::from("-- NORMAL --");
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
                                        self.buffer_manager.open_file(&path, &self.syntax_highlighter)?;
                                        self.status_message = format!("Opened: {}", path.display());
                                    }
                                    crate::sidebar::SidebarItem::Directory(path, _expanded) => {
                                        sidebar.toggle_directory(&path)?;
                                    }
                                    crate::sidebar::SidebarItem::Parent => {
                                        sidebar.navigate_to_parent()?;
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
                    // Click in editor area
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

            // Simple rendering with selection support (no syntax highlighting for now when selection is active)
            if self.buffer_manager.current().selection.is_some() {
                for ch in line.chars() {
                    let is_selected = self.buffer_manager.current().is_position_selected(row, col);
                    let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                    let mut style = get_ui_style(theme, "foreground");

                    // Check if this position matches the bracket under cursor or its match
                    let is_matching_bracket = matching_bracket
                        .map_or(false, |(match_row, match_col)|
                            match_row == row && match_col == col
                        );
                    let is_cursor_bracket = cursor_row == row && cursor_pos.1 == col;

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

                    spans.push(Span::styled(ch.to_string(), style));
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

                                if is_bracket && self.config.editor.rainbow_brackets {
                                    // Get bracket depth for rainbow coloring
                                    let depth = self.buffer_manager.current().get_bracket_depth_at((row, current_col));
                                    ratatui_style = ratatui_style.fg(self.get_rainbow_color(depth));

                                    // Highlight matching brackets
                                    if is_matching_bracket || is_cursor_bracket {
                                        ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(80, 80, 80))
                                            .add_modifier(ratatui::style::Modifier::BOLD);
                                    }
                                } else {
                                    // Normal syntax highlighting
                                    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(
                                        style.foreground.r,
                                        style.foreground.g,
                                        style.foreground.b,
                                    ));
                                }

                                if is_current_line && self.config.editor.highlight_current_line {
                                    if !is_matching_bracket && !is_cursor_bracket {
                                        ratatui_style = ratatui_style.bg(hex_to_color(&theme.ui.current_line));
                                    }
                                }

                                spans.push(Span::styled(ch.to_string(), ratatui_style));
                                current_col += 1;
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
                    // No syntax highlighting available - still handle brackets
                    for ch in line.chars() {
                        let is_bracket = matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>');
                        let mut style = get_ui_style(theme, "foreground");

                        // Check if this position matches the bracket under cursor or its match
                        let is_matching_bracket = matching_bracket
                            .map_or(false, |(match_row, match_col)|
                                match_row == row && match_col == col
                            );
                        let is_cursor_bracket = cursor_row == row && cursor_pos.1 == col;

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

                        spans.push(Span::styled(ch.to_string(), style));
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

        let mode_str = match self.mode {
            Mode::Normal => " NORMAL ",
            Mode::Insert => " INSERT ",
            Mode::Visual => " VISUAL ",
            Mode::Command => " COMMAND ",
            Mode::Search => " SEARCH ",
            Mode::Replace => " REPLACE ",
            Mode::QuitConfirm => " QUIT? ",
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
            Mode::Command | Mode::Search | Mode::Replace => Style::default()
                .fg(hex_to_color(&theme.ui.status_bar.foreground))
                .bg(hex_to_color(&theme.ui.status_bar.background)),
            Mode::QuitConfirm => Style::default()
                .fg(ratatui::style::Color::Rgb(255, 100, 100))
                .bg(hex_to_color(&theme.ui.status_bar.background))
                .add_modifier(Modifier::BOLD),
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
