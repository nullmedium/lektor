use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use super::{App, Mode};

impl App {
    pub fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Char('i'), KeyModifiers::NONE) => {
                self.mode = Mode::Insert;
                self.status_message = String::from("-- INSERT --");
            }
            (KeyCode::Char('v'), KeyModifiers::NONE) => {
                self.mode = Mode::Visual;
                self.buffer_manager.current_mut().start_selection();
                self.status_message = String::from("-- VISUAL --");
            }
            (KeyCode::Char(':'), KeyModifiers::NONE) => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.status_message = String::from(":");
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                self.mode = Mode::Search;
                self.search_query.clear();
                self.status_message = String::from("Search: ");
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Replace;
                self.search_query.clear();
                self.replace_text.clear();
                self.status_message = String::from("Replace: ");
            }
            // Basic movement
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                self.buffer_manager.current_mut().move_cursor_left();
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                self.buffer_manager.current_mut().move_cursor_right();
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                self.buffer_manager.current_mut().move_cursor_down();
                self.update_viewport();
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                self.buffer_manager.current_mut().move_cursor_up();
                self.update_viewport();
            }
            // Word movement
            (KeyCode::Char('w'), KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_word_right();
            }
            (KeyCode::Char('b'), KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().move_cursor_word_left();
            }
            // Line movement
            (KeyCode::Char('0'), KeyModifiers::NONE) | (KeyCode::Home, _) => {
                self.buffer_manager.current_mut().move_to_line_start();
            }
            (KeyCode::Char('$'), KeyModifiers::NONE) | (KeyCode::End, _) => {
                self.buffer_manager.current_mut().move_to_line_end();
            }
            // Page movement
            (KeyCode::Char('G'), KeyModifiers::SHIFT) => {
                let buffer = self.buffer_manager.current_mut();
                let last_line = buffer.line_count().saturating_sub(1);
                buffer.cursor_position = (last_line, 0);
                self.update_viewport();
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                if self.last_key == Some('g') {
                    self.buffer_manager.current_mut().cursor_position = (0, 0);
                    self.update_viewport();
                    self.last_key = None;
                } else {
                    self.last_key = Some('g');
                }
            }
            // Delete operations
            (KeyCode::Char('x'), KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().delete_char();
            }
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                if self.last_key == Some('d') {
                    self.buffer_manager.current_mut().delete_line();
                    self.last_key = None;
                    self.status_message = String::from("Line deleted");
                } else {
                    self.last_key = Some('d');
                    self.status_message = String::from("d");
                }
            }
            // Undo/Redo
            (KeyCode::Char('u'), KeyModifiers::NONE) => {
                if self.buffer_manager.current_mut().undo() {
                    self.status_message = String::from("Undo");
                } else {
                    self.status_message = String::from("Already at oldest change");
                }
            }
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                if self.buffer_manager.current_mut().redo() {
                    self.status_message = String::from("Redo");
                } else {
                    self.status_message = String::from("Already at newest change");
                }
            }
            // Save and quit
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.save_current_buffer()?;
            }
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.try_quit();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_insert_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.status_message.clear();
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.buffer_manager.current_mut().insert_char(c);
            }
            (KeyCode::Enter, _) => {
                // Get smart indentation
                let indent = self.get_smart_indent(self.buffer_manager.current());
                self.buffer_manager.current_mut().insert_char('\n');

                // Apply indentation
                if indent > 0 {
                    let indent_str = " ".repeat(indent);
                    self.buffer_manager.current_mut().insert_str(&indent_str);
                }
                self.update_viewport();
            }
            (KeyCode::Backspace, _) => {
                let buffer = self.buffer_manager.current_mut();
                if buffer.cursor_position.1 > 0 {
                    buffer.move_cursor_left();
                    buffer.delete_char();
                } else if buffer.cursor_position.0 > 0 {
                    // Move to end of previous line
                    buffer.move_cursor_up();
                    buffer.move_to_line_end();
                    buffer.delete_char();
                }
            }
            (KeyCode::Delete, _) => {
                self.buffer_manager.current_mut().delete_char();
            }
            (KeyCode::Tab, _) => {
                let tab_str = " ".repeat(self.config.editor.tab_width);
                self.buffer_manager.current_mut().insert_str(&tab_str);
            }
            // Movement in insert mode
            (KeyCode::Left, _) => {
                self.buffer_manager.current_mut().move_cursor_left();
            }
            (KeyCode::Right, _) => {
                self.buffer_manager.current_mut().move_cursor_right();
            }
            (KeyCode::Up, _) => {
                self.buffer_manager.current_mut().move_cursor_up();
                self.update_viewport();
            }
            (KeyCode::Down, _) => {
                self.buffer_manager.current_mut().move_cursor_down();
                self.update_viewport();
            }
            (KeyCode::Home, _) => {
                self.buffer_manager.current_mut().move_to_line_start();
            }
            (KeyCode::End, _) => {
                self.buffer_manager.current_mut().move_to_line_end();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_visual_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.mode = Mode::Normal;
                self.buffer_manager.current_mut().clear_selection();
                self.status_message.clear();
            }
            // Movement updates selection
            (KeyCode::Char('h'), KeyModifiers::NONE) | (KeyCode::Left, _) => {
                self.buffer_manager.current_mut().move_cursor_left();
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Char('l'), KeyModifiers::NONE) | (KeyCode::Right, _) => {
                self.buffer_manager.current_mut().move_cursor_right();
                self.buffer_manager.current_mut().update_selection();
            }
            (KeyCode::Char('j'), KeyModifiers::NONE) | (KeyCode::Down, _) => {
                self.buffer_manager.current_mut().move_cursor_down();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            (KeyCode::Char('k'), KeyModifiers::NONE) | (KeyCode::Up, _) => {
                self.buffer_manager.current_mut().move_cursor_up();
                self.buffer_manager.current_mut().update_selection();
                self.update_viewport();
            }
            // Operations on selection
            (KeyCode::Char('d'), KeyModifiers::NONE) => {
                self.buffer_manager.current_mut().delete_selection();
                self.mode = Mode::Normal;
                self.status_message = String::from("Selection deleted");
            }
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                if let Some(text) = self.buffer_manager.current().get_selected_text() {
                    // TODO: Copy to clipboard
                    self.status_message = format!("Yanked {} characters", text.len());
                }
                self.buffer_manager.current_mut().clear_selection();
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_command_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.status_message.clear();
            }
            KeyCode::Enter => {
                let command = self.command_buffer.clone();
                self.mode = Mode::Normal;
                self.command_buffer.clear();
                self.execute_command(&command)?;
            }
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_search_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.search_matches.clear();
                self.status_message.clear();
            }
            KeyCode::Enter => {
                if !self.search_query.is_empty() {
                    self.find_and_highlight_matches();
                    if !self.search_matches.is_empty() {
                        let (row, col, _) = self.search_matches[0];
                        self.buffer_manager.current_mut().cursor_position = (row, col);
                        self.update_viewport();
                        self.status_message = format!("Found {} matches", self.search_matches.len());
                    } else {
                        self.status_message = String::from("Pattern not found");
                    }
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.status_message = format!("Search: {}", self.search_query);
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.status_message = format!("Search: {}", self.search_query);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_replace_mode(&mut self, key: KeyEvent) -> Result<()> {
        let entering_replacement = self.search_query.contains('\0');

        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.search_query.clear();
                self.replace_text.clear();
                self.search_matches.clear();
                self.status_message.clear();
            }
            KeyCode::Enter => {
                if !entering_replacement && !self.search_query.is_empty() {
                    // Move to replacement text entry
                    self.search_query.push('\0');
                    self.find_and_highlight_matches();
                    if !self.search_matches.is_empty() {
                        self.status_message = format!(
                            "Replace '{}' with ({} matches): ",
                            self.search_query.trim_end_matches('\0'),
                            self.search_matches.len()
                        );
                    } else {
                        self.status_message = String::from("Pattern not found");
                        self.mode = Mode::Normal;
                    }
                } else if entering_replacement && !self.replace_text.is_empty() {
                    // Start interactive replacement
                    if !self.search_matches.is_empty() {
                        let (row, col, _) = self.search_matches[0];
                        self.buffer_manager.current_mut().cursor_position = (row, col);
                        self.update_viewport();
                        self.search_query.push('\0'); // Second null marks confirmation mode
                        self.status_message = format!(
                            "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit",
                            self.search_query.trim_end_matches('\0'),
                            self.replace_text
                        );
                    }
                }
            }
            KeyCode::Char(c) if self.search_query.matches('\0').count() >= 2 => {
                // In confirmation mode
                match c {
                    'y' | 'Y' => {
                        // Replace current match
                        if !self.search_matches.is_empty() {
                            let search_text = self.search_query.trim_end_matches('\0').to_string();
                            self.buffer_manager.current_mut().replace(&search_text, &self.replace_text, false);
                            self.search_matches.remove(0);

                            if !self.search_matches.is_empty() {
                                let (row, col, _) = self.search_matches[0];
                                self.buffer_manager.current_mut().cursor_position = (row, col);
                                self.update_viewport();
                                self.status_message = format!(
                                    "Replace '{}' → '{}': (y)es / (n)o / (a)ll / (q)uit ({} left)",
                                    search_text,
                                    self.replace_text,
                                    self.search_matches.len()
                                );
                            } else {
                                self.status_message = String::from("All replacements done");
                                self.mode = Mode::Normal;
                            }
                        }
                    }
                    'n' | 'N' => {
                        // Skip current match
                        if !self.search_matches.is_empty() {
                            self.search_matches.remove(0);
                            if !self.search_matches.is_empty() {
                                let (row, col, _) = self.search_matches[0];
                                self.buffer_manager.current_mut().cursor_position = (row, col);
                                self.update_viewport();
                            } else {
                                self.status_message = String::from("No more matches");
                                self.mode = Mode::Normal;
                            }
                        }
                    }
                    'a' | 'A' => {
                        // Replace all
                        let search_text = self.search_query.trim_end_matches('\0').to_string();
                        let count = self.search_matches.len();
                        for _ in 0..count {
                            self.buffer_manager.current_mut().replace(&search_text, &self.replace_text, false);
                        }
                        self.status_message = format!("Replaced {} occurrences", count);
                        self.mode = Mode::Normal;
                    }
                    'q' | 'Q' => {
                        // Quit replacement
                        self.mode = Mode::Normal;
                        self.status_message = String::from("Replace cancelled");
                    }
                    _ => {}
                }
            }
            KeyCode::Backspace => {
                if !entering_replacement {
                    self.search_query.pop();
                    self.status_message = format!("Replace: {}", self.search_query);
                } else {
                    self.replace_text.pop();
                    self.status_message = format!(
                        "Replace '{}' with: {}",
                        self.search_query.trim_end_matches('\0'),
                        self.replace_text
                    );
                }
            }
            KeyCode::Char(c) if self.search_query.matches('\0').count() < 2 => {
                if !entering_replacement {
                    self.search_query.push(c);
                    self.status_message = format!("Replace: {}", self.search_query);
                } else {
                    self.replace_text.push(c);
                    self.status_message = format!(
                        "Replace '{}' with: {}",
                        self.search_query.trim_end_matches('\0'),
                        self.replace_text
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn handle_quit_confirm_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Save all unsaved buffers
                for &idx in &self.unsaved_buffers_to_check {
                    let buffer = &mut self.buffer_manager.buffers[idx];
                    if buffer.file_path.is_some() {
                        buffer.save()?;
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
            _ => {}
        }
        Ok(())
    }
}
