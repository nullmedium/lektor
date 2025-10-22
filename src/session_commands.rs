use anyhow::Result;
use once_cell::sync::OnceCell;
use std::sync::Mutex;
use crate::app::App;
use crate::session_handler::SessionHandler;

// Global session handler instance
static SESSION_HANDLER: OnceCell<Mutex<SessionHandler>> = OnceCell::new();

/// Initialize the global session handler
pub fn init_session_handler(handler: SessionHandler) {
    SESSION_HANDLER.set(Mutex::new(handler)).ok();
}

/// Execute session-related commands
pub fn execute_session_command(app: &mut App, command: &str, args: &[&str]) -> Result<bool> {
    let handler = SESSION_HANDLER.get()
        .ok_or_else(|| anyhow::anyhow!("Session handler not initialized"))?;

    let mut handler = handler.lock()
        .map_err(|_| anyhow::anyhow!("Failed to lock session handler"))?;

    match command {
        "session" if !args.is_empty() => {
            match args[0] {
                "save" => {
                    if args.len() > 1 {
                        handler.save_session_as(app, args[1].to_string())?;
                        app.status_message = format!("Session saved as '{}'", args[1]);
                    } else {
                        handler.save_session(app)?;
                        app.status_message = String::from("Session saved");
                    }
                    Ok(true)
                }
                "load" if args.len() > 1 => {
                    handler.load_session(app, args[1])?;
                    Ok(true)
                }
                "delete" if args.len() > 1 => {
                    handler.delete_session(args[1])?;
                    app.status_message = format!("Session '{}' deleted", args[1]);
                    Ok(true)
                }
                "list" => {
                    let sessions = handler.list_sessions()?;
                    if sessions.is_empty() {
                        app.status_message = String::from("No saved sessions");
                    } else {
                        app.status_message = format!("Sessions: {}", sessions.join(", "));
                    }
                    Ok(true)
                }
                _ => {
                    app.status_message = String::from("Unknown session command. Use: save, load, delete, list");
                    Ok(true)
                }
            }
        }
        "mksession" => {
            // Vim-like session save command
            if !args.is_empty() {
                handler.save_session_as(app, args[0].to_string())?;
                app.status_message = format!("Session saved as '{}'", args[0]);
            } else {
                handler.save_session(app)?;
                app.status_message = String::from("Session saved");
            }
            Ok(true)
        }
        "source" if !args.is_empty() => {
            // Vim-like session restore command
            handler.load_session(app, args[0])?;
            Ok(true)
        }
        _ => Ok(false)  // Command not handled
    }
}

/// Save session on app exit
pub fn save_session_on_exit(app: &App) -> Result<()> {
    if let Some(handler) = SESSION_HANDLER.get() {
        let mut handler = handler.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock session handler"))?;

        if handler.should_auto_save() {
            handler.save_session(app)?;
        }
    }
    Ok(())
}
