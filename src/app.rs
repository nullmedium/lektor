use crate::buffer::TextBuffer;
use crate::config::Config;
use crate::sidebar::{GitStatus, Sidebar};
use crate::syntax::SyntaxHighlighter;
use crate::theme::{get_ui_style, hex_to_color, ThemeManager};
use anyhow::Result;
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
    pub buffer: TextBuffer,
    pub sidebar: Option<Sidebar>,
    pub syntax_highlighter: SyntaxHighlighter,
    pub theme_manager: ThemeManager,
    pub mode: Mode,
    pub should_quit: bool,
    pub status_message: String,
    pub show_sidebar: bool,
    pub viewport_offset: usize,
    pub command_buffer: String,
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

        Ok(Self {
            config,
            buffer: TextBuffer::new(),
            sidebar,
            syntax_highlighter: SyntaxHighlighter::new(),
            theme_manager,
            mode: Mode::Normal,
            should_quit: false,
            status_message: String::from("Ready"),
            show_sidebar: true,
            viewport_offset: 0,
            command_buffer: String::new(),
        })
    }

    pub fn open_file(&mut self, path: &PathBuf) -> Result<()> {
        self.buffer = TextBuffer::from_file(path)?;
        self.status_message = format!("Opened: {}", path.display());

        // Set syntax highlighting for the file
        if let Some(syntax) = self.syntax_highlighter.detect_syntax(path) {
            self.buffer.syntax_name = Some(syntax.name.clone());
        }

        Ok(())
    }

    pub fn save_file(&mut self) -> Result<()> {
        self.buffer.save()?;
        self.status_message = if let Some(path) = &self.buffer.file_path {
            format!("Saved: {}", path.display())
        } else {
            String::from("Buffer saved")
        };
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
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                if self.show_sidebar && self.sidebar.is_some() {
                    self.show_sidebar = false;
                } else {
                    self.buffer.move_cursor_left();
                }
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_down();
                    }
                } else {
                    self.buffer.move_cursor_down();
                    self.update_viewport();
                }
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.move_up();
                    }
                } else {
                    self.buffer.move_cursor_up();
                    self.update_viewport();
                }
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                if self.show_sidebar && self.sidebar.is_some() {
                    if let Some(sidebar) = &mut self.sidebar {
                        if sidebar.get_selected_path().is_some() {
                            sidebar.toggle_expanded()?;
                        }
                    }
                } else {
                    self.buffer.move_cursor_right();
                }
            }
            (KeyCode::Enter, _) => {
                if self.show_sidebar {
                    if let Some(sidebar) = &mut self.sidebar {
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
            (KeyCode::Char('0'), KeyModifiers::NONE) => self.buffer.move_to_line_start(),
            (KeyCode::Char('$'), KeyModifiers::NONE) => self.buffer.move_to_line_end(),
            _ => {}
        }
        Ok(())
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status_message = String::from("-- NORMAL --");
            }
            KeyCode::Char(c) => self.buffer.insert_char(c),
            KeyCode::Enter => self.buffer.insert_char('\n'),
            KeyCode::Backspace => self.buffer.delete_char(),
            KeyCode::Delete => self.buffer.delete_forward(),
            KeyCode::Tab => {
                if self.config.editor.use_spaces {
                    for _ in 0..self.config.editor.tab_width {
                        self.buffer.insert_char(' ');
                    }
                } else {
                    self.buffer.insert_char('\t');
                }
            }
            KeyCode::Left => self.buffer.move_cursor_left(),
            KeyCode::Right => self.buffer.move_cursor_right(),
            KeyCode::Up => {
                self.buffer.move_cursor_up();
                self.update_viewport();
            }
            KeyCode::Down => {
                self.buffer.move_cursor_down();
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
                self.buffer.selection = None;
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
        let parts: Vec<&str> = self.command_buffer.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "q" | "quit" => self.should_quit = true,
            "w" | "write" => self.save_file()?,
            "wq" => {
                self.save_file()?;
                self.should_quit = true;
            }
            "e" | "edit" if parts.len() > 1 => {
                let path = PathBuf::from(parts[1]);
                self.open_file(&path)?;
            }
            _ => {
                self.status_message = format!("Unknown command: {}", parts[0]);
            }
        }

        Ok(())
    }

    fn update_viewport(&mut self) {
        let viewport_height = 20;
        let cursor_row = self.buffer.cursor_position.0;

        if cursor_row < self.viewport_offset {
            self.viewport_offset = cursor_row;
        } else if cursor_row >= self.viewport_offset + viewport_height {
            self.viewport_offset = cursor_row - viewport_height + 1;
        }
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
                    let icon = if entry.is_dir {
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

            let sidebar_widget = List::new(items)
                .block(
                    Block::default()
                        .title("Files")
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

        let lines = self.buffer.get_visible_lines(self.viewport_offset, viewport_height);
        let mut paragraph_lines = Vec::new();

        // Get syntax definition if available
        let syntax = if let Some(syntax_name) = &self.buffer.syntax_name {
            self.syntax_highlighter.find_syntax_by_name(syntax_name)
        } else if let Some(path) = &self.buffer.file_path {
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

            let cursor_row = self.buffer.cursor_position.0;
            let is_current_line = self.viewport_offset + i == cursor_row;

            // Apply syntax highlighting if available
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

            paragraph_lines.push(Line::from(spans));
        }

        let editor_widget = Paragraph::new(paragraph_lines)
            .style(Style::default().bg(hex_to_color(&theme.ui.background)))
            .wrap(Wrap { trim: false });

        frame.render_widget(editor_widget, area);

        if self.mode == Mode::Insert || self.mode == Mode::Normal {
            let cursor_col = if self.config.editor.show_line_numbers {
                self.buffer.cursor_position.1 + 5
            } else {
                self.buffer.cursor_position.1
            };

            let cursor_row = self.buffer.cursor_position.0 - self.viewport_offset;

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

        let file_info = if let Some(path) = &self.buffer.file_path {
            format!(" {} ", path.display())
        } else {
            String::from(" [No Name] ")
        };

        let modified_indicator = if self.buffer.modified { "[+]" } else { "" };

        let position = format!(
            " {}:{}",
            self.buffer.cursor_position.0 + 1,
            self.buffer.cursor_position.1 + 1
        );

        let mut spans = vec![
            Span::styled(mode_str, mode_style),
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
}
