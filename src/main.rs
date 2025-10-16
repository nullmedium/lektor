mod app;
mod buffer;
mod buffer_manager;
mod config;
mod sidebar;
mod syntax;
mod theme;

use anyhow::Result;
use app::App;
use config::Config;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{env, io, time::Duration};

fn main() -> Result<()> {
    let config = Config::load().unwrap_or_default();

    let args: Vec<String> = env::args().collect();
    let path_arg = if args.len() > 1 {
        Some(std::path::PathBuf::from(&args[1]))
    } else {
        None
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Determine the working directory and file to open
    let (working_dir, file_to_open) = if let Some(path) = path_arg {
        if path.is_dir() {
            // If it's a directory, use it as working directory
            (path, None)
        } else if path.exists() {
            // If it's a file, use its parent as working directory and open the file
            let parent = path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            (parent, Some(path))
        } else {
            // Path doesn't exist, use current directory
            (std::env::current_dir().unwrap_or_default(), None)
        }
    } else {
        // No argument provided, use current directory
        (std::env::current_dir().unwrap_or_default(), None)
    };

    let mut app = App::new_with_dir(config, working_dir)?;

    if let Some(file_path) = file_to_open {
        app.open_file(&file_path)?;
    }

    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
) -> Result<()> {
    loop {
        terminal.draw(|f| {
            app.draw(f);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key)?;
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
