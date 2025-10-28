use std::collections::HashMap;
use ratatui::text::Span;

/// Cached syntax highlighting segment - stores color and text for a portion of a line
#[derive(Debug, Clone)]
pub struct CachedSyntaxSegment {
    pub foreground: (u8, u8, u8),
    pub text: String,
}

/// Cache for expensive rendering calculations
#[derive(Debug, Default)]
pub struct RenderCache {
    /// Syntax highlighting cache: (buffer_index, line_num, version) -> syntax segments
    pub syntax_cache: HashMap<(usize, usize, u64), Vec<CachedSyntaxSegment>>,

    /// Bracket depth cache: (buffer_index, line_num, version) -> vec of depths per char position
    pub bracket_depth_cache: HashMap<(usize, usize, u64), Vec<usize>>,

    /// Matching bracket cache: (buffer_index, cursor_pos, version) -> matching position
    pub matching_bracket_cache: HashMap<(usize, (usize, usize), u64), Option<(usize, usize)>>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Invalidate cache entries for a specific buffer
    pub fn invalidate_buffer(&mut self, buffer_index: usize) {
        self.syntax_cache.retain(|(buf_idx, _, _), _| *buf_idx != buffer_index);
        self.bracket_depth_cache.retain(|(buf_idx, _, _), _| *buf_idx != buffer_index);
        self.matching_bracket_cache.retain(|(buf_idx, _, _), _| *buf_idx != buffer_index);
    }

    /// Invalidate cache entries for a specific buffer and line
    pub fn invalidate_line(&mut self, buffer_index: usize, line_num: usize) {
        self.syntax_cache.retain(|(buf_idx, line, _), _| {
            *buf_idx != buffer_index || *line != line_num
        });
        self.bracket_depth_cache.retain(|(buf_idx, line, _), _| {
            *buf_idx != buffer_index || *line != line_num
        });
    }

    /// Clear all caches (e.g., on buffer switch or major changes)
    pub fn clear(&mut self) {
        self.syntax_cache.clear();
        self.bracket_depth_cache.clear();
        self.matching_bracket_cache.clear();
    }

    /// Get cache statistics for debugging/monitoring
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            syntax_entries: self.syntax_cache.len(),
            bracket_depth_entries: self.bracket_depth_cache.len(),
            matching_bracket_entries: self.matching_bracket_cache.len(),
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub syntax_entries: usize,
    pub bracket_depth_entries: usize,
    pub matching_bracket_entries: usize,
}
