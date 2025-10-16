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
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();

        // Use a theme with better bracket/brace highlighting
        let theme_name = if ts.themes.contains_key("InspiredGitHub") {
            "InspiredGitHub".to_string()
        } else if ts.themes.contains_key("base16-ocean.dark") {
            "base16-ocean.dark".to_string()
        } else {
            ts.themes.keys().next().unwrap().to_string()
        };

        Self {
            syntax_set: ss,
            theme_set: ts,
            current_theme: theme_name,
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
            // QML files - use JavaScript syntax as fallback since QML is JavaScript-based
            "qml" => self.syntax_set.find_syntax_by_name("QML")
                .or_else(|| self.syntax_set.find_syntax_by_name("JavaScript")),
            // C++ header files
            "hpp" | "hxx" | "h++" | "hh" | "h" => self.syntax_set.find_syntax_by_name("C++"),
            // C++ source files
            "cpp" | "cxx" | "c++" | "cc" => self.syntax_set.find_syntax_by_name("C++"),
            // C files (not .h, which is handled above for C++)
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

    #[allow(dead_code)]
    pub fn list_syntaxes(&self) -> Vec<String> {
        self.syntax_set.syntaxes()
            .iter()
            .map(|s| format!("{}: {:?}", s.name, s.file_extensions))
            .collect()
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
