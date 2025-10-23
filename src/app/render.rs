use crate::split::{Pane, SplitDirection, SplitNode};
use crate::buffer::TextBuffer;
use crate::sidebar::{SidebarMode, GitStatus};
use crate::theme::{ThemeManager, get_ui_style, hex_to_color};
use super::{App, Mode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use anyhow::Result;

impl super::App {
    pub fn draw(&mut self, frame: &mut Frame) {
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

    fn draw_split_node_static(app: &mut App, frame: &mut Frame, area: Rect, node: &mut SplitNode, active_index: usize) {
        match node {
            SplitNode::Leaf(pane) => {
                // Draw the pane
                app.draw_pane(frame, area, pane, active_index == 0);
            }
            SplitNode::Split { direction, ratio, first, second } => {
                let (first_area, second_area) = match direction {
                    SplitDirection::Horizontal => {
                        let first_height = ((area.height.saturating_sub(1)) as f32 * *ratio) as u16;
                        (
                            Rect::new(area.x, area.y, area.width, first_height),
                            Rect::new(area.x, area.y + first_height + 1, area.width, area.height.saturating_sub(first_height + 1)),
                        )
                    }
                    SplitDirection::Vertical => {
                        let first_width = ((area.width.saturating_sub(1)) as f32 * *ratio) as u16;
                        (
                            Rect::new(area.x, area.y, first_width, area.height),
                            Rect::new(area.x + first_width + 1, area.y, area.width.saturating_sub(first_width + 1), area.height),
                        )
                    }
                };

                // Draw separator line between panes
                let _theme = app.theme_manager.get_current_theme();
                let separator_style = Style::default().fg(Color::Rgb(60, 60, 60));

                match direction {
                    SplitDirection::Horizontal => {
                        // Draw horizontal separator line
                        let separator_y = area.y + first_area.height;
                        if separator_y < area.y + area.height {
                            let line = "─".repeat(area.width as usize);
                            frame.render_widget(
                                Paragraph::new(line)
                                    .style(separator_style),
                                Rect::new(area.x, separator_y, area.width, 1),
                            );
                        }
                    }
                    SplitDirection::Vertical => {
                        // Draw vertical separator line
                        let separator_x = area.x + first_area.width;
                        if separator_x < area.x + area.width {
                            // Create a vertical line by rendering multiple single-line paragraphs
                            for y in 0..area.height {
                                frame.render_widget(
                                    Paragraph::new("│")
                                        .style(separator_style),
                                    Rect::new(separator_x, area.y + y, 1, 1),
                                );
                            }
                        }
                    }
                }

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

    fn count_panes(node: &SplitNode) -> usize {
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

        // Update pane dimensions (no borders)
        pane.x = area.x;
        pane.y = area.y;
        pane.width = area.width;
        pane.height = area.height;

        // Draw the buffer content with syntax highlighting
        let viewport_height = area.height as usize;
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

            // Check if this line should be highlighted for diff
            let mut line_bg_color: Option<Color> = None;
            if let Some(diff_info) = &self.diff_info {
                let actual_line_index = pane.viewport_offset + i;

                // Only highlight the working buffer (right side)
                if pane.buffer_index == diff_info.working_buffer_index {
                    if diff_info.added_lines.contains(&actual_line_index) {
                        // Green background for added lines
                        line_bg_color = Some(Color::Rgb(20, 50, 20));
                    } else if diff_info.modified_lines.contains(&actual_line_index) {
                        // Yellow/amber background for modified lines
                        line_bg_color = Some(Color::Rgb(50, 50, 20));
                    }
                } else if pane.buffer_index == diff_info.head_buffer_index {
                    if diff_info.deleted_lines.contains(&actual_line_index) {
                        // Red background for deleted lines (in HEAD buffer)
                        line_bg_color = Some(Color::Rgb(50, 20, 20));
                    } else if diff_info.modified_lines.contains(&actual_line_index) {
                        // Yellow/amber background for modified lines (original version)
                        line_bg_color = Some(Color::Rgb(50, 50, 20));
                    }
                }
            }

            if self.config.editor.show_line_numbers {
                let line_number_style = if let Some(bg) = line_bg_color {
                    get_ui_style(theme, "line_numbers").bg(bg)
                } else {
                    get_ui_style(theme, "line_numbers")
                };
                spans.push(Span::styled(
                    format!("{:4} ", line_number),
                    line_number_style,
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
                                        ratatui_style = ratatui_style.bg(Color::Rgb(80, 80, 80))
                                            .add_modifier(Modifier::BOLD);
                                    }
                                } else if is_trailing_whitespace {
                                    // Highlight trailing whitespace
                                    ratatui_style = ratatui_style.fg(Color::Red)
                                        .bg(Color::Rgb(60, 20, 20));
                                } else if is_indent_guide {
                                    // Draw indent guide
                                    ratatui_style = ratatui_style.fg(Color::Rgb(60, 60, 60));
                                    spans.push(Span::styled("│", ratatui_style));
                                    current_col += 1;
                                    continue;
                                } else if is_column_ruler {
                                    // Highlight column ruler position
                                    ratatui_style = ratatui_style.bg(Color::Rgb(40, 40, 40));
                                } else if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                    // Dim whitespace characters
                                    ratatui_style = ratatui_style.fg(Color::Rgb(80, 80, 80));
                                } else {
                                    // Apply syntax highlighting color
                                    let fg = style.foreground;
                                    ratatui_style = ratatui_style.fg(Color::Rgb(
                                        fg.r,
                                        fg.g,
                                        fg.b,
                                    ));

                                    // Apply style modifiers
                                    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                                        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                                    }
                                    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                                        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                                    }
                                    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                                        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                                    }
                                }

                                // Apply diff background if present
                                if let Some(bg) = line_bg_color {
                                    if !is_matching_bracket && !is_cursor_bracket && !is_trailing_whitespace {
                                        ratatui_style = ratatui_style.bg(bg);
                                    }
                                }

                                // Apply current line highlighting (only if no diff background)
                                if is_current_line && self.config.editor.highlight_current_line && line_bg_color.is_none() {
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
                                            .fg(Color::Rgb(60, 60, 60))));
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback to simple rendering
                        let span_style = if let Some(bg) = line_bg_color {
                            Style::default().bg(bg)
                        } else {
                            Style::default()
                        };
                        spans.push(Span::styled(line.to_string(), span_style));
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
                        style = style.fg(Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        if is_selected {
                            style = style.bg(hex_to_color(&theme.ui.selection));
                        } else if let Some(bg) = line_bg_color {
                            // Apply diff background
                            style = style.bg(bg);
                        } else if is_current_line && self.config.editor.highlight_current_line {
                            style = style.bg(hex_to_color(&theme.ui.current_line));
                        }

                        if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                            style = style.fg(Color::Rgb(80, 80, 80));
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
                        style = style.fg(Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        // Apply diff background if present
                        if let Some(bg) = line_bg_color {
                            style = style.bg(bg);
                        } else if is_current_line && self.config.editor.highlight_current_line {
                            style = style.bg(hex_to_color(&theme.ui.current_line));
                        }

                        if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                            style = style.fg(Color::Rgb(80, 80, 80));
                        }

                        spans.push(Span::styled(display_char.to_string(), style));
                    }
                    col += 1;
                }
            }

            paragraph_lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(paragraph_lines)
            .style(Style::default().bg(hex_to_color(&theme.ui.background)));
        frame.render_widget(paragraph, area);

        // Draw cursor if this is the active pane
        if is_active {
            let cursor_pos = buffer.cursor_position;
            let screen_row = cursor_pos.0.saturating_sub(pane.viewport_offset);
            let screen_col = cursor_pos.1 + if self.config.editor.show_line_numbers { 5 } else { 0 };

            if screen_row < viewport_height {
                frame.set_cursor_position((area.x + screen_col as u16, area.y + screen_row as u16));
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

    fn get_rainbow_color(&self, depth: usize) -> Color {
        // Rainbow colors for different bracket depths
        let colors = [
            Color::Rgb(255, 100, 100),  // Red
            Color::Rgb(255, 200, 100),  // Orange
            Color::Rgb(255, 255, 100),  // Yellow
            Color::Rgb(100, 255, 100),  // Green
            Color::Rgb(100, 200, 255),  // Blue
            Color::Rgb(200, 100, 255),  // Purple
            Color::Rgb(255, 100, 200),  // Pink
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
                        style = style.fg(Color::Rgb(60, 60, 60));
                        spans.push(Span::styled("│", style));
                    } else {
                        if is_bracket && self.config.editor.rainbow_brackets && !is_selected {
                            // Get bracket depth for rainbow coloring
                            let depth = self.buffer_manager.current().get_bracket_depth_at((row, col));
                            style = style.fg(self.get_rainbow_color(depth));

                            // Highlight matching brackets
                            if is_matching_bracket || is_cursor_bracket {
                                style = style.bg(Color::Rgb(80, 80, 80))
                                    .add_modifier(Modifier::BOLD);
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
                            style = style.fg(Color::Rgb(80, 80, 80));
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
                                        ratatui_style = ratatui_style.bg(Color::Rgb(80, 80, 80))
                                            .add_modifier(Modifier::BOLD);
                                    }
                                } else if is_trailing_whitespace {
                                    // Highlight trailing whitespace
                                    ratatui_style = ratatui_style.fg(Color::Red)
                                        .bg(Color::Rgb(60, 20, 20));
                                } else if is_indent_guide {
                                    // Draw indent guide
                                    ratatui_style = ratatui_style.fg(Color::Rgb(60, 60, 60));
                                    spans.push(Span::styled("│", ratatui_style));
                                    current_col += 1;
                                    continue;
                                } else if is_column_ruler {
                                    // Highlight column ruler position
                                    ratatui_style = ratatui_style.bg(Color::Rgb(40, 40, 40));
                                } else if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                    // Dim whitespace characters
                                    ratatui_style = ratatui_style.fg(Color::Rgb(80, 80, 80));
                                } else {
                                    // Normal syntax highlighting
                                    ratatui_style = ratatui_style.fg(Color::Rgb(
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
                                            .fg(Color::Rgb(60, 60, 60))));
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
                            style = style.fg(Color::Rgb(60, 60, 60));
                            spans.push(Span::styled("│", style));
                        } else {
                            if is_bracket && self.config.editor.rainbow_brackets {
                                // Get bracket depth for rainbow coloring
                                let depth = self.buffer_manager.current().get_bracket_depth_at((row, col));
                                style = style.fg(self.get_rainbow_color(depth));

                                // Highlight matching brackets
                                if is_matching_bracket || is_cursor_bracket {
                                    style = style.bg(Color::Rgb(80, 80, 80))
                                        .add_modifier(Modifier::BOLD);
                                }
                            }

                            if is_current_line && self.config.editor.highlight_current_line {
                                if !is_matching_bracket && !is_cursor_bracket {
                                    style = style.bg(hex_to_color(&theme.ui.current_line));
                                }
                            }

                            if self.config.editor.show_whitespace && (ch == ' ' || ch == '\t') {
                                style = style.fg(Color::Rgb(80, 80, 80));
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
                .fg(Color::White)
                .bg(Color::Rgb(200, 50, 50))
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
}
