# Lektor - Terminal Text Editor

A modern TUI text editor built with Rust, featuring syntax highlighting, Git integration, and customizable themes.

## Features

- **Undo/Redo Support**
  - Full undo/redo history with Ctrl+Z/Ctrl+Y
  - Maintains cursor position and selection state
  - Works in both Normal and Insert modes
- **Mouse Support**
  - Click to position cursor
  - Drag to select text
  - Scroll wheel support
  - Sidebar navigation with mouse
- **Multiple Buffer Support**
  - Work with multiple files simultaneously
  - Buffer indicator in status bar shows [current/total]
  - Switch between buffers without losing changes
  - Each buffer maintains its own state and cursor position
- **File Navigation Sidebar** with directory tree view
  - Shows current directory tree by default
  - Expandable/collapsible directories
  - Git status indicators for modified files
- **Search and Replace**
  - Case-sensitive/insensitive search
  - Interactive replace with preview
  - Replace all or one-by-one
  - Preserves syntax highlighting during search
- **Git Integration** showing file status in sidebar
- **Syntax Highlighting** for multiple languages including:
  - Rust, C, C++
  - Python, JavaScript, TypeScript
  - HTML, CSS, JSON, TOML
  - QML for Qt development
  - And many more via syntect
- **Rainbow Brackets** with matching bracket highlighting
- **Theme Support** with dark and light themes
- **Configurable** via TOML configuration file
- **Modal Editing** with Normal, Insert, Visual, and Command modes

## Installation

```bash
cargo build --release
```

## Usage

```bash
# Open editor in current directory
./target/release/lektor

# Open specific file
./target/release/lektor file.rs

# Open editor in specific directory
./target/release/lektor /path/to/directory

# Open with directory sidebar
./target/release/lektor src/
```

## Key Bindings

### Normal Mode
- `i` - Enter insert mode
- `v` - Enter visual mode
- `:` - Enter command mode
- `h/j/k/l` or Arrow keys - Navigate in editor
- `0` - Move to line start
- `$` - Move to line end
- `Ctrl+S` - Save file
- `Ctrl+Q` - Quit
- `Ctrl+Z` - Undo last change
- `Ctrl+Y` - Redo last undone change

### Sidebar Navigation (Normal Mode)
- `Ctrl+B` - Toggle sidebar visibility
- `Ctrl+T` - Toggle between file tree and buffer list view
- `Ctrl+R` - Refresh sidebar (updates file list and Git status)
- Arrow keys - Navigate files/folders/buffers when sidebar is focused
- `Enter` - Open file/buffer, expand/collapse directory, or navigate to parent

#### File Tree Mode
- Shows the current directory tree with:
  - `↑ ..` for parent directory navigation
  - `▶` for collapsed directories
  - `▼` for expanded directories
  - Git status colors for modified/added/deleted files
  - Automatically refreshes when saving files

#### Buffer List Mode
- Shows all open buffers with:
  - Buffer number (1, 2, 3, etc.)
  - Filename or [No Name] for unnamed buffers
  - [+] indicator for modified buffers
  - Press Enter to switch to selected buffer

### Selection & Editing (Normal & Insert modes)
- `Shift+Arrow` - Select text character by character
- `Ctrl+Shift+Left/Right` - Select word by word
- `Shift+Up/Down` - Select line by line
- `Ctrl+A` - Select all
- `Ctrl+C` - Copy selected text
- `Ctrl+X` - Cut selected text
- `Ctrl+V` - Paste from clipboard
- `Ctrl+Left/Right` - Move cursor word by word

### Indentation
- `Tab` - Indent selected lines (or current line if no selection)
- `Shift+Tab` - Unindent selected lines (or current line if no selection)
- Works in both Normal and Insert modes
- Respects the `use_spaces` and `tab_width` settings from config

### Insert Mode
- `Esc` - Return to normal mode
- `Tab` - Insert tab/spaces
- Regular typing for text input
- All selection/clipboard shortcuts work in insert mode
- `Ctrl+Z` - Undo last change
- `Ctrl+Y` - Redo last undone change

### Command Mode
- `:w` - Save file (for existing files)
- `:w <filename>` - Save as (saves with a new filename)
- `:q` - Quit
- `:wq` - Save and quit
- `:wq <filename>` - Save as and quit
- `:e <file>` - Open file in new buffer

#### Buffer Commands
- `:bn` or `:bnext` - Switch to next buffer
- `:bp` or `:bprevious` - Switch to previous buffer
- `:bd` or `:bdelete` - Close current buffer
- `:ls` or `:buffers` - List all open buffers

Note: When saving an unnamed buffer with Ctrl+S, it will prompt for a filename in command mode.

### Search Mode (Ctrl+F)
- Type search query
- `Enter` - Jump to next match
- `Ctrl+G` - Toggle case-sensitive search
- `Esc` - Exit search mode

### Replace Mode (Ctrl+H)
- Type search query, press `Enter`
- Type replacement text, press `Enter`
- Choose replace option:
  - `y` - Replace current and continue
  - `n` - Skip current and continue
  - `a` - Replace all occurrences
  - `q` or `Esc` - Quit replace mode
- `Ctrl+G` - Toggle case-sensitive search (before confirming)

### Mouse Support
- **Left Click** - Position cursor at click location
- **Left Click in Sidebar** - Select file/directory or buffer
- **Left Drag** - Select text
- **Scroll Wheel** - Scroll up/down in the editor

## Configuration

Copy `config.example.toml` to `~/.config/lektor/config.toml` and customize settings.

## License

MIT
