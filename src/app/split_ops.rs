use crate::split::{SplitManager, SplitDirection, Pane};
use crate::buffer::TextBuffer;
use anyhow::Result;

impl super::App {
    pub fn split_horizontal(&mut self) -> Result<()> {
        let current_buffer_index = self.buffer_manager.current_index;

        if self.split_manager.is_none() {
            let terminal_size = crossterm::terminal::size()?;
            let sidebar_width = if self.show_sidebar { self.config.sidebar.width } else { 0 };
            self.split_manager = Some(SplitManager::new(
                current_buffer_index,
                terminal_size.0,
                terminal_size.1.saturating_sub(2),
                sidebar_width,
            ));
        }

        if let Some(split_manager) = &mut self.split_manager {
            // Create a new buffer with the same content as current
            let current_buffer = &self.buffer_manager.buffers[current_buffer_index];
            let mut new_buffer = TextBuffer::new();
            new_buffer.content = current_buffer.content.clone();
            new_buffer.file_path = current_buffer.file_path.clone();
            new_buffer.modified = current_buffer.modified;
            new_buffer.cursor_position = current_buffer.cursor_position;

            self.buffer_manager.buffers.push(new_buffer);
            let new_buffer_index = self.buffer_manager.buffers.len() - 1;

            split_manager.split_current(SplitDirection::Horizontal, new_buffer_index);
            self.status_message = String::from("Split horizontal");
        }

        Ok(())
    }

    pub fn split_vertical(&mut self) -> Result<()> {
        let current_buffer_index = self.buffer_manager.current_index;

        if self.split_manager.is_none() {
            let terminal_size = crossterm::terminal::size()?;
            let sidebar_width = if self.show_sidebar { self.config.sidebar.width } else { 0 };
            self.split_manager = Some(SplitManager::new(
                current_buffer_index,
                terminal_size.0,
                terminal_size.1.saturating_sub(2),
                sidebar_width,
            ));
        }

        if let Some(split_manager) = &mut self.split_manager {
            // Create a new buffer with the same content as current
            let current_buffer = &self.buffer_manager.buffers[current_buffer_index];
            let mut new_buffer = TextBuffer::new();
            new_buffer.content = current_buffer.content.clone();
            new_buffer.file_path = current_buffer.file_path.clone();
            new_buffer.modified = current_buffer.modified;
            new_buffer.cursor_position = current_buffer.cursor_position;

            self.buffer_manager.buffers.push(new_buffer);
            let new_buffer_index = self.buffer_manager.buffers.len() - 1;

            split_manager.split_current(SplitDirection::Vertical, new_buffer_index);
            self.status_message = String::from("Split vertical");
        }

        Ok(())
    }

    pub fn next_pane(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            split_manager.next_pane();
            self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);
        }
    }

    fn get_pane_buffer(&self, pane: &Pane) -> &TextBuffer {
        &self.buffer_manager.buffers[pane.buffer_index]
    }

    fn get_pane_buffer_mut(&mut self, pane: &Pane) -> &mut TextBuffer {
        &mut self.buffer_manager.buffers[pane.buffer_index]
    }

    fn get_active_buffer_mut(&mut self) -> Option<&mut TextBuffer> {
        if let Some(split_manager) = &mut self.split_manager {
            if let Some(pane) = split_manager.get_active_pane() {
                let buffer_index = pane.buffer_index;
                return Some(&mut self.buffer_manager.buffers[buffer_index]);
            }
        }
        None
    }

    fn update_active_pane_cursor(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            if let Some(pane) = split_manager.get_active_pane() {
                let buffer = &self.buffer_manager.buffers[pane.buffer_index];
                pane.viewport_offset = self.calculate_viewport_offset(buffer);
            }
        }
    }

    pub fn previous_pane(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            split_manager.previous_pane();
            self.status_message = format!("Switched to pane {}", split_manager.active_pane_index + 1);
        }
    }

    pub fn close_current_pane(&mut self) {
        if let Some(split_manager) = &mut self.split_manager {
            // If there's only one pane, close the entire split view
            if split_manager.get_pane_count() <= 1 {
                self.split_manager = None;
                self.diff_info = None;  // Clear diff info when closing split
                self.status_message = String::from("Split view closed");
            } else {
                // Close just the current pane (not implemented yet in split_manager)
                // For now, just close all splits
                self.split_manager = None;
                self.diff_info = None;  // Clear diff info when closing split
                self.status_message = String::from("Split view closed");
            }
        } else {
            self.status_message = String::from("No split to close");
        }
    }

    fn calculate_viewport_offset(&self, buffer: &TextBuffer) -> usize {
        // This logic should match the viewport calculation in the main app
        // For now, just return 0 as a placeholder
        0
    }
}
