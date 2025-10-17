use crate::buffer::Selection;
use ropey::Rope;

#[derive(Debug, Clone)]
pub struct Cursor {
    pub position: (usize, usize),  // (row, col)
    pub selection: Option<Selection>,
    pub is_primary: bool,
}

impl Cursor {
    pub fn new(position: (usize, usize), is_primary: bool) -> Self {
        Self {
            position,
            selection: None,
            is_primary,
        }
    }
}

pub struct CursorManager {
    pub cursors: Vec<Cursor>,
    pub primary_index: usize,
}

impl CursorManager {
    pub fn new() -> Self {
        Self {
            cursors: vec![Cursor::new((0, 0), true)],
            primary_index: 0,
        }
    }

    pub fn primary(&self) -> &Cursor {
        &self.cursors[self.primary_index]
    }

    pub fn primary_mut(&mut self) -> &mut Cursor {
        &mut self.cursors[self.primary_index]
    }

    pub fn add_cursor(&mut self, position: (usize, usize)) {
        // Check if cursor already exists at this position
        for cursor in &self.cursors {
            if cursor.position == position {
                return;
            }
        }

        self.cursors.push(Cursor::new(position, false));
    }

    pub fn remove_cursor(&mut self, index: usize) {
        if index != self.primary_index && index < self.cursors.len() {
            self.cursors.remove(index);
            // Adjust primary index if needed
            if index < self.primary_index {
                self.primary_index -= 1;
            }
        }
    }

    pub fn clear_secondary_cursors(&mut self) {
        self.cursors.retain(|c| c.is_primary);
        self.primary_index = 0;
    }

    pub fn merge_overlapping_cursors(&mut self) {
        let mut i = 0;
        while i < self.cursors.len() {
            let mut j = i + 1;
            while j < self.cursors.len() {
                if self.cursors[i].position == self.cursors[j].position {
                    if j == self.primary_index {
                        self.primary_index = i;
                        self.cursors[i].is_primary = true;
                    } else if j < self.primary_index {
                        self.primary_index -= 1;
                    }
                    self.cursors.remove(j);
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    pub fn move_all_cursors<F>(&mut self, content: &Rope, mut move_fn: F)
    where
        F: FnMut(&mut Cursor, &Rope),
    {
        for cursor in &mut self.cursors {
            move_fn(cursor, content);
        }
        self.merge_overlapping_cursors();
    }

    pub fn insert_at_all_cursors(&mut self, content: &mut Rope, text: &str) -> Vec<(usize, usize)> {
        // Sort cursor indices by position (bottom to top, right to left) to avoid offset issues
        let mut sorted_indices: Vec<(usize, (usize, usize))> = self.cursors
            .iter()
            .enumerate()
            .map(|(idx, cursor)| (idx, cursor.position))
            .collect();
        sorted_indices.sort_by(|a, b| {
            let pos_a = a.1;
            let pos_b = b.1;
            pos_b.0.cmp(&pos_a.0).then(pos_b.1.cmp(&pos_a.1))
        });

        let mut insertions = Vec::new();
        let text_lines: Vec<&str> = text.lines().collect();
        let lines_added = if text.contains('\n') { text_lines.len() - 1 } else { 0 };

        for (idx, original_pos) in sorted_indices {
            let (row, col) = original_pos;
            let line_idx = content.line_to_char(row);
            let pos = line_idx + col;

            // Insert text
            content.insert(pos, text);

            // Update this cursor's position
            if text_lines.len() > 1 {
                self.cursors[idx].position = (row + text_lines.len() - 1, text_lines.last().unwrap().len());
            } else {
                self.cursors[idx].position.1 += text.len();
            }

            insertions.push((row, col));

            // Update other cursors that come after this position
            for i in 0..self.cursors.len() {
                if i != idx {
                    let other_pos = &mut self.cursors[i].position;
                    if other_pos.0 == row && other_pos.1 > col {
                        if lines_added > 0 {
                            other_pos.0 += lines_added;
                        } else {
                            other_pos.1 += text.len();
                        }
                    } else if other_pos.0 > row && lines_added > 0 {
                        other_pos.0 += lines_added;
                    }
                }
            }
        }

        insertions
    }

    pub fn delete_at_all_cursors(&mut self, content: &mut Rope) {
        // Sort cursor indices by position (bottom to top, right to left)
        let mut sorted_indices: Vec<(usize, (usize, usize))> = self.cursors
            .iter()
            .enumerate()
            .map(|(idx, cursor)| (idx, cursor.position))
            .collect();
        sorted_indices.sort_by(|a, b| {
            let pos_a = a.1;
            let pos_b = b.1;
            pos_b.0.cmp(&pos_a.0).then(pos_b.1.cmp(&pos_a.1))
        });

        for (idx, original_pos) in sorted_indices {
            let (row, col) = original_pos;

            if col > 0 {
                let line_idx = content.line_to_char(row);
                let pos = line_idx + col - 1;

                if pos < content.len_chars() {
                    content.remove(pos..pos + 1);
                    self.cursors[idx].position.1 -= 1;
                }
            } else if row > 0 {
                let prev_line = content.line(row - 1);
                let prev_line_len = prev_line.len_chars().saturating_sub(1);

                let line_idx = content.line_to_char(row);
                let pos = line_idx - 1;

                if pos < content.len_chars() {
                    content.remove(pos..pos + 1);
                    self.cursors[idx].position = (row - 1, prev_line_len);
                }
            }
        }
    }

    pub fn set_primary(&mut self, index: usize) {
        if index < self.cursors.len() {
            self.cursors[self.primary_index].is_primary = false;
            self.primary_index = index;
            self.cursors[self.primary_index].is_primary = true;
        }
    }

    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    pub fn has_multiple_cursors(&self) -> bool {
        self.cursors.len() > 1
    }
}
