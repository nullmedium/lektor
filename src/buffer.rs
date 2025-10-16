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
    pub start: (usize, usize),
    pub end: (usize, usize),
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
}
