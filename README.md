# Lektor - Terminal Text Editor

A modern TUI text editor built with Rust, featuring syntax highlighting, Git integration, and customizable themes.

## Features

- **File Navigation Sidebar** with tree view
- **Git Integration** showing file status in sidebar
- **Syntax Highlighting** for multiple languages including:
  - Rust, C, C++
  - Python, JavaScript, TypeScript
  - HTML, CSS, JSON, TOML
  - QML for Qt development
  - And many more via syntect
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
- `h/j/k/l` or Arrow keys - Navigate
- `0` - Move to line start
- `$` - Move to line end
- `Ctrl+S` - Save file
- `Ctrl+Q` - Quit
- `Ctrl+B` - Toggle sidebar
- `Enter` - Open file/folder in sidebar

### Insert Mode
- `Esc` - Return to normal mode
- `Tab` - Insert tab/spaces
- Regular typing for text input

### Command Mode
- `:w` - Save file
- `:q` - Quit
- `:wq` - Save and quit
- `:e <file>` - Open file

## Configuration

Copy `config.example.toml` to `~/.config/lektor/config.toml` and customize settings.

## License

MIT
