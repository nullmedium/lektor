use ratatui::style::{Color, Style};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub ui: UiTheme,
    pub syntax: SyntaxTheme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTheme {
    pub background: String,
    pub foreground: String,
    pub selection: String,
    pub cursor: String,
    pub current_line: String,
    pub line_numbers: String,
    pub status_bar: StatusBarTheme,
    pub sidebar: SidebarTheme,
    pub border: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarTheme {
    pub background: String,
    pub foreground: String,
    pub mode_normal: String,
    pub mode_insert: String,
    pub mode_visual: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarTheme {
    pub background: String,
    pub foreground: String,
    pub selected: String,
    pub git_modified: String,
    pub git_added: String,
    pub git_deleted: String,
    pub folder: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntaxTheme {
    pub keyword: String,
    pub string: String,
    pub comment: String,
    pub function: String,
    pub variable: String,
    pub number: String,
    pub operator: String,
    pub type_name: String,
}

impl Theme {
    pub fn default_dark() -> Self {
        Theme {
            name: String::from("Dark"),
            ui: UiTheme {
                background: "#1e1e2e".to_string(),
                foreground: "#cdd6f4".to_string(),
                selection: "#45475a".to_string(),
                cursor: "#f5e0dc".to_string(),
                current_line: "#313244".to_string(),
                line_numbers: "#585b70".to_string(),
                status_bar: StatusBarTheme {
                    background: "#181825".to_string(),
                    foreground: "#cdd6f4".to_string(),
                    mode_normal: "#89b4fa".to_string(),
                    mode_insert: "#a6e3a1".to_string(),
                    mode_visual: "#f38ba8".to_string(),
                },
                sidebar: SidebarTheme {
                    background: "#11111b".to_string(),
                    foreground: "#bac2de".to_string(),
                    selected: "#45475a".to_string(),
                    git_modified: "#f9e2af".to_string(),
                    git_added: "#a6e3a1".to_string(),
                    git_deleted: "#f38ba8".to_string(),
                    folder: "#89b4fa".to_string(),
                    file: "#cdd6f4".to_string(),
                },
                border: "#585b70".to_string(),
            },
            syntax: SyntaxTheme {
                keyword: "#cba6f7".to_string(),
                string: "#a6e3a1".to_string(),
                comment: "#6c7086".to_string(),
                function: "#89b4fa".to_string(),
                variable: "#f5e0dc".to_string(),
                number: "#fab387".to_string(),
                operator: "#94e2d5".to_string(),
                type_name: "#f9e2af".to_string(),
            },
        }
    }

    pub fn default_light() -> Self {
        Theme {
            name: String::from("Light"),
            ui: UiTheme {
                background: "#eff1f5".to_string(),
                foreground: "#4c4f69".to_string(),
                selection: "#acb0be".to_string(),
                cursor: "#dc8a78".to_string(),
                current_line: "#e6e9ef".to_string(),
                line_numbers: "#9ca0b0".to_string(),
                status_bar: StatusBarTheme {
                    background: "#dce0e8".to_string(),
                    foreground: "#4c4f69".to_string(),
                    mode_normal: "#1e66f5".to_string(),
                    mode_insert: "#40a02b".to_string(),
                    mode_visual: "#d20f39".to_string(),
                },
                sidebar: SidebarTheme {
                    background: "#e6e9ef".to_string(),
                    foreground: "#5c5f77".to_string(),
                    selected: "#ccd0da".to_string(),
                    git_modified: "#df8e1d".to_string(),
                    git_added: "#40a02b".to_string(),
                    git_deleted: "#d20f39".to_string(),
                    folder: "#1e66f5".to_string(),
                    file: "#4c4f69".to_string(),
                },
                border: "#9ca0b0".to_string(),
            },
            syntax: SyntaxTheme {
                keyword: "#8839ef".to_string(),
                string: "#40a02b".to_string(),
                comment: "#9ca0b0".to_string(),
                function: "#1e66f5".to_string(),
                variable: "#dc8a78".to_string(),
                number: "#fe640b".to_string(),
                operator: "#179299".to_string(),
                type_name: "#df8e1d".to_string(),
            },
        }
    }
}

pub struct ThemeManager {
    themes: HashMap<String, Theme>,
    current_theme: String,
}

impl ThemeManager {
    pub fn new() -> Self {
        let mut themes = HashMap::new();

        let dark = Theme::default_dark();
        let light = Theme::default_light();

        themes.insert(dark.name.clone(), dark);
        themes.insert(light.name.clone(), light);

        Self {
            themes,
            current_theme: String::from("Dark"),
        }
    }

    pub fn get_current_theme(&self) -> &Theme {
        self.themes.get(&self.current_theme).unwrap()
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        if self.themes.contains_key(name) {
            self.current_theme = name.to_string();
            true
        } else {
            false
        }
    }

    pub fn get_available_themes(&self) -> Vec<String> {
        self.themes.keys().cloned().collect()
    }

    pub fn add_theme(&mut self, theme: Theme) {
        self.themes.insert(theme.name.clone(), theme);
    }
}

pub fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');

    if hex.len() != 6 {
        return Color::Reset;
    }

    if let Ok(rgb) = u32::from_str_radix(hex, 16) {
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;
        Color::Rgb(r, g, b)
    } else {
        Color::Reset
    }
}

pub fn get_ui_style(theme: &Theme, element: &str) -> Style {
    let ui = &theme.ui;

    match element {
        "background" => Style::default().bg(hex_to_color(&ui.background)),
        "foreground" => Style::default().fg(hex_to_color(&ui.foreground)),
        "selection" => Style::default().bg(hex_to_color(&ui.selection)),
        "cursor" => Style::default().bg(hex_to_color(&ui.cursor)),
        "current_line" => Style::default().bg(hex_to_color(&ui.current_line)),
        "line_numbers" => Style::default().fg(hex_to_color(&ui.line_numbers)),
        "border" => Style::default().fg(hex_to_color(&ui.border)),
        _ => Style::default(),
    }
}
