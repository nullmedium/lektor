use crate::split::SplitDirection;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use super::{Mode, App};

impl super::App {
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Visual => self.handle_visual_mode(key),
            Mode::Command => self.handle_command_mode(key),
            Mode::Search => self.handle_search_mode(key),
            Mode::Replace => self.handle_replace_mode(key),
            Mode::QuitConfirm => self.handle_quit_confirm_mode(key),
        }
    }

    pub(super) fn try_quit(&mut self) {
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

    pub(super) fn update_viewport(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            if let Some(pane) = split_manager.get_active_pane() {
                let buffer = &self.buffer_manager.buffers[pane.buffer_index];
                let cursor_row = buffer.cursor_position.0;
                let viewport_height = 20; // This should be calculated based on actual pane height

                if cursor_row < pane.viewport_offset {
                    pane.viewport_offset = cursor_row;
                } else if cursor_row >= pane.viewport_offset + viewport_height {
                    pane.viewport_offset = cursor_row.saturating_sub(viewport_height - 1);
                }
            }
        } else {
            let buffer = self.buffer_manager.current();
            let cursor_row = buffer.cursor_position.0;
            let viewport_height = 20; // This should be calculated based on actual terminal height

            if cursor_row < self.viewport_offset {
                self.viewport_offset = cursor_row;
            } else if cursor_row >= self.viewport_offset + viewport_height {
                self.viewport_offset = cursor_row.saturating_sub(viewport_height - 1);
            }
        }
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<()> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Handle mouse click in split view
                if let Some(split_manager) = &mut self.split_manager {
                    if split_manager.handle_click(mouse.column, mouse.row) {
                        self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);
                        return Ok(());
                    }
                }

                // Check if click is in sidebar area
                if self.show_sidebar && mouse.column < self.config.sidebar.width {
                    if let Some(sidebar) = &mut self.sidebar {
                        sidebar.handle_click(mouse.row as usize);
                    }
                    return Ok(());
                }

                // Handle click in editor area
                let editor_start_x = if self.show_sidebar { self.config.sidebar.width } else { 0 };
                if mouse.column >= editor_start_x {
                    let relative_x = mouse.column - editor_start_x;
                    let relative_y = mouse.row;

                    // Calculate buffer position from mouse coordinates
                    let viewport_offset = if let Some(split_manager) = &self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            pane.viewport_offset
                        } else {
                            self.viewport_offset
                        }
                    } else {
                        self.viewport_offset
                    };

                    let line_offset = if self.config.editor.show_line_numbers { 5 } else { 0 };
                    let target_row = viewport_offset + relative_y as usize;
                    let target_col = if relative_x >= line_offset {
                        (relative_x - line_offset) as usize
                    } else {
                        0
                    };

                    // Get the appropriate buffer
                    let buffer = if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            &mut self.buffer_manager.buffers[pane.buffer_index]
                        } else {
                            self.buffer_manager.current_mut()
                        }
                    } else {
                        self.buffer_manager.current_mut()
                    };

                    // Ensure we don't go beyond buffer bounds
                    let max_row = buffer.line_count().saturating_sub(1);
                    let actual_row = target_row.min(max_row);
                    let line_len = buffer.get_line(actual_row).len();
                    let actual_col = target_col.min(line_len);

                    buffer.cursor_position = (actual_row, actual_col);
                    buffer.clear_selection();
                    self.update_viewport();
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                // Handle text selection during drag
                let editor_start_x = if self.show_sidebar { self.config.sidebar.width } else { 0 };
                if mouse.column >= editor_start_x {
                    let relative_x = mouse.column - editor_start_x;
                    let relative_y = mouse.row;

                    let viewport_offset = if let Some(split_manager) = &self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            pane.viewport_offset
                        } else {
                            self.viewport_offset
                        }
                    } else {
                        self.viewport_offset
                    };

                    let line_offset = if self.config.editor.show_line_numbers { 5 } else { 0 };
                    let target_row = viewport_offset + relative_y as usize;
                    let target_col = if relative_x >= line_offset {
                        (relative_x - line_offset) as usize
                    } else {
                        0
                    };

                    let buffer = if let Some(split_manager) = &mut self.split_manager {
                        if let Some(pane) = split_manager.get_active_pane() {
                            &mut self.buffer_manager.buffers[pane.buffer_index]
                        } else {
                            self.buffer_manager.current_mut()
                        }
                    } else {
                        self.buffer_manager.current_mut()
                    };

                    let max_row = buffer.line_count().saturating_sub(1);
                    let actual_row = target_row.min(max_row);
                    let line_len = buffer.get_line(actual_row).len();
                    let actual_col = target_col.min(line_len);

                    // Start selection if not already started
                    if buffer.selection.is_none() {
                        buffer.start_selection();
                    }

                    buffer.cursor_position = (actual_row, actual_col);
                    buffer.update_selection();
                    self.update_viewport();
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(split_manager) = &mut self.split_manager {
                    // Handle scrolling in split panes
                    if let Some(pane) = split_manager.get_active_pane() {
                        pane.viewport_offset = pane.viewport_offset.saturating_sub(3);
                    }
                } else {
                    // Handle scrolling in single editor
                    self.viewport_offset = self.viewport_offset.saturating_sub(3);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(split_manager) = &mut self.split_manager {
                    // Handle scrolling in split panes
                    if let Some(pane) = split_manager.get_active_pane() {
                        let buffer = &self.buffer_manager.buffers[pane.buffer_index];
                        let max_offset = buffer.content.len_lines().saturating_sub(10);
                        if pane.viewport_offset < max_offset {
                            pane.viewport_offset += 3;
                        }
                    }
                } else {
                    // Handle scrolling in single editor
                    let buffer = self.buffer_manager.current();
                    let max_offset = buffer.content.len_lines().saturating_sub(10);
                    if self.viewport_offset < max_offset {
                        self.viewport_offset += 3;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    // Note: The individual mode handlers (handle_normal_mode, handle_insert_mode, etc.)
    // are quite large and would be extracted in a separate refactoring step.
    // For now, they remain in the main app.rs file.
}
