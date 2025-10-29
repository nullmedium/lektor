
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
pub struct Pane {
    pub buffer_index: usize,  // Index into BufferManager's buffers
    pub viewport_offset: usize,
    pub horizontal_offset: usize, // Horizontal scrolling for long lines
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Pane {
    pub fn new(buffer_index: usize, x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            buffer_index,
            viewport_offset: 0,
            horizontal_offset: 0,
            cursor_x: 0,
            cursor_y: 0,
            x,
            y,
            width,
            height,
        }
    }

    pub fn resize(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }

    pub fn adjust_viewport(&mut self, cursor_row: usize) {
        let visible_lines = self.height.saturating_sub(2) as usize;

        if cursor_row < self.viewport_offset {
            self.viewport_offset = cursor_row;
        } else if cursor_row >= self.viewport_offset + visible_lines {
            self.viewport_offset = cursor_row.saturating_sub(visible_lines - 1);
        }
    }

    pub fn adjust_horizontal_offset(&mut self, cursor_col: usize, visible_width: usize) {
        let scroll_margin = 5; // Columns from left/right before scrolling starts

        // Scroll left if cursor is too far left
        if cursor_col < self.horizontal_offset + scroll_margin {
            self.horizontal_offset = cursor_col.saturating_sub(scroll_margin);
        }
        // Scroll right if cursor is too far right
        else if cursor_col >= self.horizontal_offset + visible_width.saturating_sub(scroll_margin) {
            self.horizontal_offset = cursor_col + scroll_margin + 1 - visible_width.min(cursor_col + scroll_margin + 1);
        }
    }
}

#[derive(Debug)]
pub enum SplitNode {
    Leaf(Pane),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

impl SplitNode {
    pub fn new_leaf(pane: Pane) -> Self {
        SplitNode::Leaf(pane)
    }

    pub fn split(&mut self, direction: SplitDirection, new_buffer_index: usize) -> bool {
        match self {
            SplitNode::Leaf(pane) => {
                let (first_pane, second_pane) = match direction {
                    SplitDirection::Horizontal => {
                        let half_height = pane.height / 2;
                        let first = Pane::new(
                            pane.buffer_index,
                            pane.x,
                            pane.y,
                            pane.width,
                            half_height,
                        );
                        let second = Pane::new(
                            new_buffer_index,
                            pane.x,
                            pane.y + half_height,
                            pane.width,
                            pane.height - half_height,
                        );
                        (first, second)
                    }
                    SplitDirection::Vertical => {
                        let half_width = pane.width / 2;
                        let first = Pane::new(
                            pane.buffer_index,
                            pane.x,
                            pane.y,
                            half_width,
                            pane.height,
                        );
                        let second = Pane::new(
                            new_buffer_index,
                            pane.x + half_width,
                            pane.y,
                            pane.width - half_width,
                            pane.height,
                        );
                        (first, second)
                    }
                };

                *self = SplitNode::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(SplitNode::Leaf(first_pane)),
                    second: Box::new(SplitNode::Leaf(second_pane)),
                };
                true
            }
            _ => false,
        }
    }

    pub fn resize(&mut self, x: u16, y: u16, width: u16, height: u16) {
        match self {
            SplitNode::Leaf(pane) => {
                pane.resize(x, y, width, height);
            }
            SplitNode::Split { direction, ratio, first, second } => {
                match direction {
                    SplitDirection::Horizontal => {
                        let first_height = (height as f32 * *ratio) as u16;
                        let second_height = height - first_height;
                        first.resize(x, y, width, first_height);
                        second.resize(x, y + first_height, width, second_height);
                    }
                    SplitDirection::Vertical => {
                        let first_width = (width as f32 * *ratio) as u16;
                        let second_width = width - first_width;
                        first.resize(x, y, first_width, height);
                        second.resize(x + first_width, y, second_width, height);
                    }
                }
            }
        }
    }

    pub fn find_pane_at(&mut self, target_x: u16, target_y: u16) -> Option<&mut Pane> {
        match self {
            SplitNode::Leaf(pane) => {
                if target_x >= pane.x && target_x < pane.x + pane.width
                    && target_y >= pane.y && target_y < pane.y + pane.height
                {
                    Some(pane)
                } else {
                    None
                }
            }
            SplitNode::Split { first, second, .. } => {
                first.find_pane_at(target_x, target_y)
                    .or_else(|| second.find_pane_at(target_x, target_y))
            }
        }
    }

    pub fn get_all_panes(&mut self) -> Vec<&mut Pane> {
        match self {
            SplitNode::Leaf(pane) => vec![pane],
            SplitNode::Split { first, second, .. } => {
                let mut panes = first.get_all_panes();
                panes.extend(second.get_all_panes());
                panes
            }
        }
    }

    pub fn get_all_panes_immutable(&self) -> Vec<&Pane> {
        match self {
            SplitNode::Leaf(pane) => vec![pane],
            SplitNode::Split { first, second, .. } => {
                let mut panes = first.get_all_panes_immutable();
                panes.extend(second.get_all_panes_immutable());
                panes
            }
        }
    }
}

pub struct SplitManager {
    pub root: SplitNode,
    pub active_pane_index: usize,
}

impl SplitManager {
    pub fn new(buffer_index: usize, width: u16, height: u16, sidebar_width: u16) -> Self {
        let pane = Pane::new(buffer_index, sidebar_width, 0, width - sidebar_width, height);
        Self {
            root: SplitNode::new_leaf(pane),
            active_pane_index: 0,
        }
    }

    pub fn split_current(&mut self, direction: SplitDirection, new_buffer_index: usize) -> bool {
        if let Some(pane) = self.get_active_pane() {
            let x = pane.x;
            let y = pane.y;
            let width = pane.width;
            let height = pane.height;

            self.root.split(direction, new_buffer_index);
            self.root.resize(x, y, width, height);
            true
        } else {
            false
        }
    }

    pub fn get_active_pane(&mut self) -> Option<&mut Pane> {
        let mut panes = self.root.get_all_panes();
        if self.active_pane_index < panes.len() {
            Some(panes.swap_remove(self.active_pane_index))
        } else {
            None
        }
    }

    pub fn get_active_buffer_index(&self) -> Option<usize> {
        let panes = self.root.get_all_panes_immutable();
        if self.active_pane_index < panes.len() {
            Some(panes[self.active_pane_index].buffer_index)
        } else {
            None
        }
    }

    pub fn next_pane(&mut self) {
        let pane_count = self.count_panes();
        if pane_count > 0 {
            self.active_pane_index = (self.active_pane_index + 1) % pane_count;
        }
    }

    pub fn previous_pane(&mut self) {
        let pane_count = self.count_panes();
        if pane_count > 0 {
            self.active_pane_index = if self.active_pane_index == 0 {
                pane_count - 1
            } else {
                self.active_pane_index - 1
            };
        }
    }

    pub fn resize(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.root.resize(x, y, width, height);
    }

    pub fn count_panes(&mut self) -> usize {
        self.root.get_all_panes().len()
    }

    pub fn get_pane_count(&self) -> usize {
        self.root.get_all_panes_immutable().len()
    }

    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let panes = self.root.get_all_panes();
        for (index, pane) in panes.into_iter().enumerate() {
            if x >= pane.x && x < pane.x + pane.width
                && y >= pane.y && y < pane.y + pane.height
            {
                self.active_pane_index = index;
                return true;
            }
        }
        false
    }
}
