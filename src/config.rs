use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub editor: EditorConfig,
    pub theme: ThemeConfig,
    pub keybindings: KeybindingsConfig,
    pub sidebar: SidebarConfig,
    pub session: SessionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    pub tab_width: usize,
    pub use_spaces: bool,
    pub auto_indent: bool,
    pub show_line_numbers: bool,
    pub highlight_current_line: bool,
    pub word_wrap: bool,
    pub auto_save: bool,
    pub auto_save_interval: u64,
    pub rainbow_brackets: bool,
    pub highlight_matching_bracket: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub name: String,
    pub syntax_theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    pub save: String,
    pub quit: String,
    pub open: String,
    pub find: String,
    pub replace: String,
    pub goto_line: String,
    pub toggle_sidebar: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarConfig {
    pub show_hidden_files: bool,
    pub show_git_status: bool,
    pub width: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub auto_save: bool,
    pub auto_restore: bool,
    pub workspace_sessions: bool,
    pub restore_cursor_position: bool,
    pub restore_open_buffers: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            editor: EditorConfig {
                tab_width: 4,
                use_spaces: true,
                auto_indent: true,
                show_line_numbers: true,
                highlight_current_line: true,
                word_wrap: false,
                auto_save: false,
                auto_save_interval: 30,
                rainbow_brackets: true,
                highlight_matching_bracket: true,
            },
            theme: ThemeConfig {
                name: String::from("default"),
                syntax_theme: String::from("base16-ocean.dark"),
            },
            keybindings: KeybindingsConfig {
                save: String::from("Ctrl+S"),
                quit: String::from("Ctrl+Q"),
                open: String::from("Ctrl+O"),
                find: String::from("Ctrl+F"),
                replace: String::from("Ctrl+H"),
                goto_line: String::from("Ctrl+G"),
                toggle_sidebar: String::from("Ctrl+B"),
            },
            sidebar: SidebarConfig {
                show_hidden_files: false,
                show_git_status: true,
                width: 25,
            },
            session: SessionConfig {
                auto_save: true,
                auto_restore: true,
                workspace_sessions: true,
                restore_cursor_position: true,
                restore_open_buffers: true,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        let config_path = config_dir.join("lektor").join("config.toml");

        if config_path.exists() {
            let contents = fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        let config_path = config_dir.join("lektor");

        fs::create_dir_all(&config_path)?;

        let config_file = config_path.join("config.toml");
        let contents = toml::to_string_pretty(self)?;
        fs::write(config_file, contents)?;

        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        Ok(config_dir.join("lektor").join("config.toml"))
    }
}
