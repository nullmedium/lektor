use crate::buffer::TextBuffer;
use anyhow::Result;
use arboard::Clipboard;

impl super::App {
    pub(super) fn copy(&mut self) -> Result<()> {
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

    pub(super) fn cut(&mut self) -> Result<()> {
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

    pub(super) fn paste(&mut self) -> Result<()> {
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

    pub(super) fn select_all(&mut self) {
        self.buffer_manager.current_mut().cursor_position = (0, 0);
        self.buffer_manager.current_mut().start_selection();
        // Move to end of document
        let last_line = self.buffer_manager.current().line_count().saturating_sub(1);
        let last_col = self.buffer_manager.current().get_line(last_line).len().saturating_sub(1);
        self.buffer_manager.current_mut().cursor_position = (last_line, last_col);
        self.buffer_manager.current_mut().update_selection();
        self.status_message = String::from("Selected all");
    }

    pub(super) fn get_smart_indent(&self, buffer: &TextBuffer) -> usize {
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

    pub(super) fn toggle_comment(&mut self) {
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

            buffer.modified = true;
        }
    }
}
