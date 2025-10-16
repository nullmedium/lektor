use anyhow::Result;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use std::path::Path;

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    current_theme: String,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            current_theme: String::from("base16-ocean.dark"),
        }
    }

    pub fn set_theme(&mut self, theme_name: &str) {
        if self.theme_set.themes.contains_key(theme_name) {
            self.current_theme = theme_name.to_string();
        }
    }

    pub fn get_available_themes(&self) -> Vec<String> {
        self.theme_set.themes.keys().cloned().collect()
    }

    pub fn detect_syntax(&self, file_path: &Path) -> Option<&SyntaxReference> {
        let extension = file_path.extension()?.to_str()?;

        // Handle special cases for extensions that might not be auto-detected
        match extension.to_lowercase().as_str() {
            "qml" => self.syntax_set.find_syntax_by_name("QML"),
            "hpp" | "hxx" | "h++" | "hh" => self.syntax_set.find_syntax_by_name("C++"),
            "cpp" | "cxx" | "c++" | "cc" => self.syntax_set.find_syntax_by_name("C++"),
            "c" => self.syntax_set.find_syntax_by_name("C"),
            _ => self.syntax_set.find_syntax_by_extension(extension)
                .or_else(|| {
                    // Fallback: try to find by extension with different cases
                    self.syntax_set.find_syntax_by_extension(&extension.to_lowercase())
                })
        }
    }

    pub fn detect_syntax_by_first_line(&self, first_line: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_first_line(first_line)
    }

    pub fn highlight_line(
        &self,
        line: &str,
        syntax: &SyntaxReference,
    ) -> Result<Vec<(Style, String)>> {
        let theme = &self.theme_set.themes[&self.current_theme];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let highlighted = highlighter.highlight_line(line, &self.syntax_set)?;

        Ok(highlighted
            .into_iter()
            .map(|(style, text)| (style, text.to_string()))
            .collect())
    }

    pub fn highlight_lines(
        &self,
        text: &str,
        syntax: &SyntaxReference,
    ) -> Result<Vec<Vec<(Style, String)>>> {
        let theme = &self.theme_set.themes[&self.current_theme];
        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();

        for line in LinesWithEndings::from(text) {
            let highlighted = highlighter.highlight_line(line, &self.syntax_set)?;
            result.push(
                highlighted
                    .into_iter()
                    .map(|(style, text)| (style, text.to_string()))
                    .collect(),
            );
        }

        Ok(result)
    }

    pub fn get_background_color(&self) -> Option<(u8, u8, u8)> {
        let theme = &self.theme_set.themes[&self.current_theme];
        theme.settings.background.map(|c| (c.r, c.g, c.b))
    }

    pub fn get_foreground_color(&self) -> Option<(u8, u8, u8)> {
        let theme = &self.theme_set.themes[&self.current_theme];
        theme.settings.foreground.map(|c| (c.r, c.g, c.b))
    }

    pub fn find_syntax_by_name(&self, name: &str) -> Option<&SyntaxReference> {
        self.syntax_set.find_syntax_by_name(name)
    }
}

pub fn style_to_ratatui_style(style: &Style) -> ratatui::style::Style {
    let mut ratatui_style = ratatui::style::Style::default();

    ratatui_style = ratatui_style.fg(ratatui::style::Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    ratatui_style = ratatui_style.bg(ratatui::style::Color::Rgb(
        style.background.r,
        style.background.g,
        style.background.b,
    ));

    ratatui_style
}
