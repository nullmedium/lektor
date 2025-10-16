# Lektor Text Editor - Feature Suggestions & Improvements

After analyzing the codebase, here are my suggestions organized by category:

## 🚀 High-Impact Features

### 1. Undo/Redo System
- Currently missing, essential for any text editor
- Implement command pattern with history stack
- Ctrl+Z/Ctrl+Y keybindings

### 2. Find & Replace Enhancements
- Add regex support (already have regex crate via syntect)
- Find in files (project-wide search)
- Replace in selection only
- Search history

### 3. Auto-completion & LSP Support
- Basic keyword/snippet completion
- Language Server Protocol integration for intelligent completions
- Function signatures, documentation on hover

### 4. Split View/Panes
- Vertical/horizontal splits
- Navigate between panes with Ctrl+W shortcuts
- Compare files side-by-side

## 🎨 UI/UX Improvements

### 5. Status Line Enhancements
- File encoding indicator (UTF-8, etc.)
- Git branch name
- Language/syntax mode indicator
- Character under cursor info

### 6. Line Operations
- Duplicate line (Ctrl+D)
- Move line up/down (Alt+Up/Down)
- Join lines
- Sort selected lines

### 7. Better Visual Feedback
- Column ruler at 80/100/120 chars
- Whitespace visualization (tabs, spaces, trailing)
- Indent guides
- Minimap for navigation

### 8. Command Palette
- Ctrl+Shift+P for command search
- Fuzzy finder for commands
- Recently used commands

## 📝 Editor Features

### 9. Multi-cursor Support
- Ctrl+Click to add cursors
- Ctrl+Alt+Up/Down for column selection
- Edit multiple locations simultaneously

### 10. Code Folding
- Fold/unfold code blocks
- Fold at indentation levels
- Persistent fold states

### 11. Smart Editing
- Auto-close brackets/quotes
- Smart indentation based on language
- Comment toggling (Ctrl+/)
- Block commenting

### 12. Bookmarks & Navigation
- Set/jump to bookmarks (Ctrl+F2, F2)
- Go to definition/declaration
- Navigate back/forward through edit locations

## 🔧 Performance & Architecture

### 13. Async File Operations
- Already have tokio, use it for non-blocking I/O
- Background file saving
- Lazy loading for large files

### 14. Plugin System
- Simple Lua/WASM plugin support
- Hook into editor events
- Custom commands/keybindings

### 15. Session Management
- Save/restore open buffers
- Persist cursor positions
- Project workspace files

## 🐛 Bug Fixes & Polish

### 16. Mouse Support
- Click to position cursor
- Drag to select text
- Scroll with mouse wheel
- Already have EnableMouseCapture but not handling events

### 17. Improved Error Handling
- Better error messages in status line
- Graceful recovery from errors
- File permission checks

### 18. Testing Infrastructure
- Add unit tests for buffer operations
- Integration tests for key sequences
- Performance benchmarks

## 💡 Quick Wins (Easy to Implement)

### 19. Recent Files
- Track and display recently opened files
- Quick access via :recent command

### 20. Line/Column Jump
- :goto line:column syntax
- Ctrl+G for goto line dialog

### 21. Word Count
- Display in status bar or via command
- Selection word count

### 22. File Templates
- New file templates based on extension
- Configurable templates directory

## 🔍 Code Quality Improvements

### 23. Refactoring Suggestions
- Extract large methods in `app.rs` (2000+ lines)
- Create separate modules for modes/commands
- Better separation of concerns

### 24. Configuration Enhancements
- Hot-reload config changes
- Per-project .lektor files
- Export/import config profiles

### 25. Documentation
- Add inline documentation (rustdoc)
- Create user manual
- Add examples directory

## 🎯 Priority Recommendations

### Immediate (Essential)
1. **Undo/Redo** - Critical for usability
2. **Mouse support** - Already partially setup
3. **Multi-cursor** - Modern editor expectation

### Short-term (High Value)
4. **Find in files**
5. **Auto-completion** (basic)
6. **Split views**
7. **Line operations**

### Long-term (Nice to Have)
8. **LSP integration**
9. **Plugin system**
10. **Session management**

## Technical Notes

The codebase is well-structured with good separation between modules. The use of Ratatui and Ropey provides a solid foundation. The main areas for improvement are:

- Adding essential editing features that users expect from a modern editor
- Reducing the size of `app.rs` by extracting mode handlers into separate modules
- Leveraging the async runtime (tokio) that's already included for better performance
- Adding comprehensive testing to ensure reliability

## Implementation Complexity

### Low Complexity
- Line operations (duplicate, move)
- Recent files tracking
- Word count
- Better status line info

### Medium Complexity
- Undo/redo system
- Mouse support
- Find in files
- Basic auto-completion
- Code folding

### High Complexity
- LSP integration
- Plugin system
- Multi-cursor support
- Split views with proper focus management
