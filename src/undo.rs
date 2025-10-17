use ropey::Rope;
use std::collections::VecDeque;

const MAX_UNDO_HISTORY: usize = 1000;

#[derive(Debug, Clone)]
pub struct EditorState {
    pub content: Rope,
    pub cursor_position: (usize, usize),
    pub selection: Option<crate::buffer::Selection>,
}

#[derive(Debug)]
pub struct UndoManager {
    undo_stack: VecDeque<EditorState>,
    redo_stack: VecDeque<EditorState>,
    last_saved_state: Option<EditorState>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            redo_stack: VecDeque::new(),
            last_saved_state: None,
        }
    }

    pub fn save_state(&mut self, state: EditorState) {
        // Clear redo stack when new action is performed
        self.redo_stack.clear();

        // Add to undo stack
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(state);
    }

    pub fn undo(&mut self, current_state: EditorState) -> Option<EditorState> {
        if let Some(previous_state) = self.undo_stack.pop_back() {
            // Save current state to redo stack
            self.redo_stack.push_back(current_state);
            Some(previous_state)
        } else {
            None
        }
    }

    pub fn redo(&mut self, current_state: EditorState) -> Option<EditorState> {
        if let Some(next_state) = self.redo_stack.pop_back() {
            // Save current state to undo stack
            self.undo_stack.push_back(current_state);
            Some(next_state)
        } else {
            None
        }
    }

    pub fn mark_saved(&mut self, state: EditorState) {
        self.last_saved_state = Some(state);
    }

    pub fn is_modified(&self, current_state: &EditorState) -> bool {
        if let Some(ref saved) = self.last_saved_state {
            // Simple comparison - could be enhanced
            saved.content != current_state.content
        } else {
            !current_state.content.to_string().is_empty()
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
