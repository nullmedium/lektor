use anyhow::Result;
use std::path::PathBuf;
use crate::buffer::TextBuffer;
use crate::syntax::SyntaxHighlighter;

pub struct BufferManager {
    buffers: Vec<TextBuffer>,
    current_index: usize,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: vec![TextBuffer::new()],
            current_index: 0,
        }
    }

    pub fn current(&self) -> &TextBuffer {
        &self.buffers[self.current_index]
    }

    pub fn current_mut(&mut self) -> &mut TextBuffer {
        &mut self.buffers[self.current_index]
    }

    pub fn open_file(&mut self, path: &PathBuf, syntax_highlighter: &SyntaxHighlighter) -> Result<()> {
        // Check if file is already open
        for (i, buffer) in self.buffers.iter().enumerate() {
            if let Some(buffer_path) = &buffer.file_path {
                if buffer_path == path {
                    self.current_index = i;
                    return Ok(());
                }
            }
        }

        // Open new file
        let mut buffer = TextBuffer::from_file(path)?;

        // Set syntax highlighting for the file
        if let Some(syntax) = syntax_highlighter.detect_syntax(path) {
            buffer.syntax_name = Some(syntax.name.clone());
        }

        self.buffers.push(buffer);
        self.current_index = self.buffers.len() - 1;

        Ok(())
    }

    pub fn new_buffer(&mut self) {
        self.buffers.push(TextBuffer::new());
        self.current_index = self.buffers.len() - 1;
    }

    pub fn close_current(&mut self) -> Result<bool> {
        if self.buffers.len() == 1 {
            // Don't close the last buffer, just clear it
            self.buffers[0] = TextBuffer::new();
            Ok(false)
        } else {
            self.buffers.remove(self.current_index);
            if self.current_index >= self.buffers.len() {
                self.current_index = self.buffers.len() - 1;
            }
            Ok(true)
        }
    }

    pub fn next_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.current_index = (self.current_index + 1) % self.buffers.len();
        }
    }

    pub fn previous_buffer(&mut self) {
        if !self.buffers.is_empty() {
            if self.current_index == 0 {
                self.current_index = self.buffers.len() - 1;
            } else {
                self.current_index -= 1;
            }
        }
    }

    pub fn go_to_buffer(&mut self, index: usize) -> bool {
        if index < self.buffers.len() {
            self.current_index = index;
            true
        } else {
            false
        }
    }

    pub fn get_buffer_list(&self) -> Vec<String> {
        self.buffers.iter().enumerate().map(|(i, buffer)| {
            let mut name = if let Some(path) = &buffer.file_path {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            } else {
                format!("[No Name]")
            };

            if buffer.modified {
                name.push_str(" [+]");
            }

            if i == self.current_index {
                format!("{} {}", i + 1, name)
            } else {
                format!("{} {}", i + 1, name)
            }
        }).collect()
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    pub fn current_buffer_index(&self) -> usize {
        self.current_index + 1 // 1-indexed for display
    }

    pub fn get_buffer_info(&self) -> String {
        let current = self.current();
        let name = if let Some(path) = &current.file_path {
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string()
        } else {
            "[No Name]".to_string()
        };

        let modified = if current.modified { " [+]" } else { "" };

        format!("[{}/{}] {}{}",
            self.current_index + 1,
            self.buffers.len(),
            name,
            modified
        )
    }

    pub fn has_unsaved_buffers(&self) -> Vec<usize> {
        self.buffers
            .iter()
            .enumerate()
            .filter_map(|(i, b)| if b.modified { Some(i) } else { None })
            .collect()
    }

    pub fn get_buffer_info_list(&self) -> Vec<(usize, String, PathBuf, bool)> {
        self.buffers
            .iter()
            .enumerate()
            .map(|(i, buffer)| {
                let name = if let Some(path) = &buffer.file_path {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unnamed")
                        .to_string()
                } else {
                    "[No Name]".to_string()
                };
                let path = buffer.file_path.clone().unwrap_or_default();
                (i, name, path, buffer.modified)
            })
            .collect()
    }
}
