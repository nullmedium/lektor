use anyhow::Result;
use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TextBuffer {
    pub content: Rope,
    pub file_path: Option<PathBuf>,
    pub modified: bool,
    pub cursor_position: (usize, usize),
    pub selection: Option<Selection>,
    pub syntax_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub start: (usize, usize),  // (row, col)
    pub end: (usize, usize),    // (row, col)
    pub anchor: (usize, usize), // Original position when selection started
}

impl Selection {
    pub fn new(pos: (usize, usize)) -> Self {
        Self {
            start: pos,
            end: pos,
            anchor: pos,
        }
    }

    pub fn update_end(&mut self, new_end: (usize, usize)) {
        self.end = new_end;
        // Ensure start is always before end
        if (new_end.0 < self.anchor.0) ||
           (new_end.0 == self.anchor.0 && new_end.1 < self.anchor.1) {
            self.start = new_end;
            self.end = self.anchor;
        } else {
            self.start = self.anchor;
            self.end = new_end;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            content: Rope::new(),
            file_path: None,
            modified: false,
            cursor_position: (0, 0),
            selection: None,
            syntax_name: None,
        }
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let rope = Rope::from_str(&contents);

        Ok(Self {
            content: rope,
            file_path: Some(path.to_path_buf()),
            modified: false,
            cursor_position: (0, 0),
            selection: None,
            syntax_name: None,
        })
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(path) = &self.file_path {
            fs::write(path, self.content.to_string())?;
            self.modified = false;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        fs::write(path, self.content.to_string())?;
        self.file_path = Some(path.to_path_buf());
        self.modified = false;
        Ok(())
    }

    pub fn insert_char(&mut self, ch: char) {
        let (row, col) = self.cursor_position;
        let line_idx = self.content.line_to_char(row);
        let pos = line_idx + col;

        self.content.insert_char(pos, ch);

        if ch == '\n' {
            self.cursor_position = (row + 1, 0);
        } else {
            self.cursor_position.1 += 1;
        }

        self.modified = true;
    }

    pub fn insert_str(&mut self, text: &str) {
        for ch in text.chars() {
            self.insert_char(ch);
        }
    }

    pub fn delete_char(&mut self) {
        let (row, col) = self.cursor_position;

        if col > 0 {
            let line_idx = self.content.line_to_char(row);
            let pos = line_idx + col - 1;

            self.content.remove(pos..pos + 1);
            self.cursor_position.1 -= 1;
            self.modified = true;
        } else if row > 0 {
            let prev_line = self.content.line(row - 1);
            let prev_line_len = prev_line.len_chars().saturating_sub(1);

            let line_idx = self.content.line_to_char(row);
            let pos = line_idx - 1;

            self.content.remove(pos..pos + 1);
            self.cursor_position = (row - 1, prev_line_len);
            self.modified = true;
        }
    }

    pub fn delete_forward(&mut self) {
        let (row, col) = self.cursor_position;
        let line = self.content.line(row);
        let line_len = line.len_chars();

        if col < line_len {
            let line_idx = self.content.line_to_char(row);
            let pos = line_idx + col;

            if pos < self.content.len_chars() {
                self.content.remove(pos..pos + 1);
                self.modified = true;
            }
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_position.0 > 0 {
            self.cursor_position.0 -= 1;
            let line = self.content.line(self.cursor_position.0);
            let line_len = line.len_chars().saturating_sub(1);
            self.cursor_position.1 = self.cursor_position.1.min(line_len);
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_position.0 < self.content.len_lines().saturating_sub(1) {
            self.cursor_position.0 += 1;
            let line = self.content.line(self.cursor_position.0);
            let line_len = line.len_chars().saturating_sub(1);
            self.cursor_position.1 = self.cursor_position.1.min(line_len);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position.1 > 0 {
            self.cursor_position.1 -= 1;
        } else if self.cursor_position.0 > 0 {
            self.cursor_position.0 -= 1;
            let line = self.content.line(self.cursor_position.0);
            self.cursor_position.1 = line.len_chars().saturating_sub(1);
        }
    }

    pub fn move_cursor_right(&mut self) {
        let line = self.content.line(self.cursor_position.0);
        let line_len = line.len_chars().saturating_sub(1);

        if self.cursor_position.1 < line_len {
            self.cursor_position.1 += 1;
        } else if self.cursor_position.0 < self.content.len_lines().saturating_sub(1) {
            self.cursor_position.0 += 1;
            self.cursor_position.1 = 0;
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_position.1 = 0;
    }

    pub fn move_to_line_end(&mut self) {
        let line = self.content.line(self.cursor_position.0);
        self.cursor_position.1 = line.len_chars().saturating_sub(1);
    }

    pub fn get_line(&self, row: usize) -> String {
        if row < self.content.len_lines() {
            self.content.line(row).to_string()
        } else {
            String::new()
        }
    }

    pub fn line_count(&self) -> usize {
        self.content.len_lines()
    }

    pub fn get_visible_lines(&self, start_row: usize, height: usize) -> Vec<String> {
        let mut lines = Vec::new();
        let end_row = (start_row + height).min(self.content.len_lines());

        for i in start_row..end_row {
            lines.push(self.get_line(i));
        }

        lines
    }

    pub fn start_selection(&mut self) {
        self.selection = Some(Selection::new(self.cursor_position));
    }

    pub fn update_selection(&mut self) {
        if let Some(ref mut selection) = self.selection {
            selection.update_end(self.cursor_position);
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn get_selected_text(&self) -> Option<String> {
        if let Some(ref selection) = self.selection {
            if selection.is_empty() {
                return None;
            }

            let start_idx = self.content.line_to_char(selection.start.0) + selection.start.1;
            let end_idx = self.content.line_to_char(selection.end.0) + selection.end.1;

            Some(self.content.slice(start_idx..end_idx).to_string())
        } else {
            None
        }
    }

    pub fn delete_selection(&mut self) -> Option<String> {
        if let Some(ref selection) = self.selection {
            if selection.is_empty() {
                return None;
            }

            let start_idx = self.content.line_to_char(selection.start.0) + selection.start.1;
            let end_idx = self.content.line_to_char(selection.end.0) + selection.end.1;

            let deleted_text = self.content.slice(start_idx..end_idx).to_string();
            self.content.remove(start_idx..end_idx);

            self.cursor_position = selection.start;
            self.selection = None;
            self.modified = true;

            Some(deleted_text)
        } else {
            None
        }
    }

    pub fn move_cursor_word_left(&mut self) {
        let line = self.get_line(self.cursor_position.0);
        let mut col = self.cursor_position.1;

        // Skip current whitespace
        while col > 0 && line.chars().nth(col - 1).map_or(false, |c| c.is_whitespace()) {
            col -= 1;
        }

        // Skip word characters
        while col > 0 && line.chars().nth(col - 1).map_or(false, |c| !c.is_whitespace()) {
            col -= 1;
        }

        self.cursor_position.1 = col;
    }

    pub fn move_cursor_word_right(&mut self) {
        let line = self.get_line(self.cursor_position.0);
        let mut col = self.cursor_position.1;
        let line_len = line.len();

        // Skip current word
        while col < line_len && line.chars().nth(col).map_or(false, |c| !c.is_whitespace()) {
            col += 1;
        }

        // Skip whitespace
        while col < line_len && line.chars().nth(col).map_or(false, |c| c.is_whitespace()) {
            col += 1;
        }

        self.cursor_position.1 = col.min(line.len().saturating_sub(1));
    }

    pub fn is_position_selected(&self, row: usize, col: usize) -> bool {
        if let Some(ref selection) = self.selection {
            if selection.is_empty() {
                return false;
            }

            // Check if position is between start and end
            if row < selection.start.0 || row > selection.end.0 {
                return false;
            }

            if row == selection.start.0 && row == selection.end.0 {
                // Selection is on a single line
                col >= selection.start.1 && col < selection.end.1
            } else if row == selection.start.0 {
                // First line of selection
                col >= selection.start.1
            } else if row == selection.end.0 {
                // Last line of selection
                col < selection.end.1
            } else {
                // Middle lines are fully selected
                true
            }
        } else {
            false
        }
    }

    pub fn indent_selection(&mut self, use_spaces: bool, tab_width: usize) {
        if let Some(ref selection) = self.selection.clone() {
            let start_line = selection.start.0;
            let end_line = selection.end.0;

            let indent_str = if use_spaces {
                " ".repeat(tab_width)
            } else {
                "\t".to_string()
            };

            // Indent each line in the selection
            for line_num in (start_line..=end_line).rev() {
                let line_start_idx = self.content.line_to_char(line_num);
                self.content.insert(line_start_idx, &indent_str);
            }

            // Adjust cursor and selection positions
            if self.cursor_position.0 >= start_line && self.cursor_position.0 <= end_line {
                self.cursor_position.1 += indent_str.len();
            }

            if let Some(ref mut selection) = self.selection {
                if selection.start.0 == start_line {
                    selection.start.1 += indent_str.len();
                }
                if selection.end.0 == end_line && selection.end.1 > 0 {
                    selection.end.1 += indent_str.len();
                }
                selection.anchor.1 += indent_str.len();
            }

            self.modified = true;
        }
    }

    pub fn unindent_selection(&mut self, use_spaces: bool, tab_width: usize) -> usize {
        let mut max_removed = 0;

        if let Some(ref selection) = self.selection.clone() {
            let start_line = selection.start.0;
            let end_line = selection.end.0;

            // Unindent each line in the selection
            for line_num in (start_line..=end_line).rev() {
                let line = self.get_line(line_num);
                let line_start_idx = self.content.line_to_char(line_num);

                // Calculate how much to unindent
                let mut chars_to_remove = 0;
                let mut space_count = 0;

                for ch in line.chars() {
                    if ch == '\t' {
                        chars_to_remove = 1;
                        break;
                    } else if ch == ' ' {
                        space_count += 1;
                        if space_count >= tab_width {
                            chars_to_remove = tab_width;
                            break;
                        }
                    } else {
                        // Hit non-whitespace, use what we found
                        if space_count > 0 {
                            chars_to_remove = space_count.min(tab_width);
                        }
                        break;
                    }
                }

                if chars_to_remove > 0 {
                    self.content.remove(line_start_idx..line_start_idx + chars_to_remove);

                    // Track max removed for single line case
                    if start_line == end_line {
                        max_removed = chars_to_remove;
                    }

                    // Adjust cursor and selection if on this line
                    if self.cursor_position.0 == line_num {
                        self.cursor_position.1 = self.cursor_position.1.saturating_sub(chars_to_remove);
                    }

                    if let Some(ref mut selection) = self.selection {
                        if selection.start.0 == line_num {
                            selection.start.1 = selection.start.1.saturating_sub(chars_to_remove);
                        }
                        if selection.end.0 == line_num {
                            selection.end.1 = selection.end.1.saturating_sub(chars_to_remove);
                        }
                        if selection.anchor.0 == line_num {
                            selection.anchor.1 = selection.anchor.1.saturating_sub(chars_to_remove);
                        }
                    }
                }
            }

            self.modified = true;
        }

        max_removed
    }

    pub fn has_selection(&self) -> bool {
        self.selection.as_ref().map_or(false, |s| !s.is_empty())
    }

    pub fn get_selection_lines(&self) -> Option<(usize, usize)> {
        self.selection.as_ref().map(|s| (s.start.0, s.end.0))
    }
}
