use anyhow::Result;
use crate::app::App;
use crate::session::{Session, SessionManager, BufferState, SelectionState, WindowState};
use crate::buffer::Selection;

pub struct SessionHandler {
    manager: SessionManager,
}

impl SessionHandler {
    pub fn new(auto_save: bool, workspace_sessions: bool) -> Self {
        let mut manager = SessionManager::new();
        manager.set_auto_save(auto_save);
        manager.set_workspace_session(workspace_sessions);

        SessionHandler { manager }
    }

    /// Save the current app state to a session
    pub fn save_session(&mut self, app: &App) -> Result<()> {
        // Decide whether to use workspace or default session
        // If workspace_sessions is enabled but we can't create a workspace session,
        // fall back to default
        let use_workspace = app.config.session.workspace_sessions;

        let (session_name, actual_workspace) = if use_workspace {
            // Check if we can create a workspace session
            if let Ok(path) = crate::session::Session::get_workspace_session_path() {
                if let Some(parent) = path.parent() {
                    // Try to create the .lektor directory
                    if std::fs::create_dir_all(parent).is_ok() {
                        (String::from("workspace"), true)
                    } else {
                        // Silently fall back to default session if we can't create workspace dir
                        (String::from("default"), false)
                    }
                } else {
                    (String::from("default"), false)
                }
            } else {
                (String::from("default"), false)
            }
        } else {
            (String::from("default"), false)
        };

        // Update the manager to use the correct type
        self.manager.set_workspace_session(actual_workspace);

        let session = self.manager.create_session(session_name);

        // Clear existing buffers and save all open buffers
        session.buffers.clear();

        for buffer in &app.buffer_manager.buffers {
            let selection_state = buffer.selection.as_ref().map(|sel| SelectionState {
                start: sel.start,
                end: sel.end,
            });

            // Get buffer content as string
            let content = buffer.content.to_string();

            session.buffers.push(BufferState {
                file_path: buffer.file_path.clone(),
                cursor_position: buffer.cursor_position,
                viewport_offset: app.viewport_offset,
                selection: selection_state,
                is_modified: buffer.modified,
                content_hash: Some(crate::session::calculate_content_hash(&content)),
            });
        }

        // Save active buffer index
        session.active_buffer = app.buffer_manager.current_index;

        // Save window state
        session.window_state = WindowState {
            width: 0,  // Terminal size will be used on restore
            height: 0,
            sidebar_visible: app.show_sidebar,
            sidebar_width: app.config.sidebar.width,
        };

        // Save split layout if present
        if let Some(ref split_manager) = app.split_manager {
            // TODO: Implement split layout serialization
            // This would need to traverse the split tree and save the structure
        }

        self.manager.save_current()
    }

    /// Restore app state from a session
    pub fn restore_session(&mut self, app: &mut App) -> Result<()> {
        let session = if app.config.session.workspace_sessions {
            if let Some(s) = self.manager.load_workspace_session()? {
                s
            } else {
                // Try default session
                self.manager.load_session("default")
                    .unwrap_or_else(|_| Session::new(String::from("default")))
            }
        } else {
            self.manager.load_session("default")?
        };

        // Clear current buffers
        app.buffer_manager.buffers.clear();

        // Restore buffers
        if app.config.session.restore_open_buffers {
            for buffer_state in &session.buffers {
                if let Some(ref path) = buffer_state.file_path {
                    if path.exists() {
                        if let Ok(_) = app.buffer_manager.open_file(path, &app.syntax_highlighter) {
                            let buffer_index = app.buffer_manager.buffers.len() - 1;
                            let buffer = &mut app.buffer_manager.buffers[buffer_index];

                            // Restore cursor position if enabled
                            if app.config.session.restore_cursor_position {
                                buffer.cursor_position = buffer_state.cursor_position;
                                app.viewport_offset = buffer_state.viewport_offset;
                            }

                            // Restore selection
                            if let Some(ref sel) = buffer_state.selection {
                                buffer.selection = Some(Selection {
                                    start: sel.start,
                                    end: sel.end,
                                    anchor: sel.start,  // Use start as anchor
                                });
                            }
                        }
                    }
                }
            }
        }

        // Restore active buffer
        if session.active_buffer < app.buffer_manager.buffers.len() {
            app.buffer_manager.current_index = session.active_buffer;
        }

        // Restore window state
        app.show_sidebar = session.window_state.sidebar_visible;

        // If no buffers were restored, create a new empty one
        if app.buffer_manager.buffers.is_empty() {
            app.buffer_manager.new_buffer();
        }

        app.status_message = format!("Session restored: {} buffers", app.buffer_manager.buffers.len());
        Ok(())
    }

    /// Save session with a specific name
    pub fn save_session_as(&mut self, app: &App, name: String) -> Result<()> {
        self.manager.create_session(name.clone());
        self.save_session(app)?;
        Ok(())
    }

    /// Load a specific named session
    pub fn load_session(&mut self, app: &mut App, name: &str) -> Result<()> {
        self.manager.load_session(name)?;
        self.restore_session(app)?;
        app.status_message = format!("Loaded session '{}'", name);
        Ok(())
    }

    /// Delete a named session
    pub fn delete_session(&mut self, name: &str) -> Result<()> {
        Session::delete(name)
    }

    /// List all available sessions
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        Session::list_sessions()
    }

    /// Check if auto-save is enabled
    pub fn should_auto_save(&self) -> bool {
        self.manager.should_auto_save()
    }

    /// Initialize session on app startup
    pub fn init_session(app: &mut App) -> Result<Self> {
        let mut handler = SessionHandler::new(
            app.config.session.auto_save,
            app.config.session.workspace_sessions,
        );

        // Try to restore last session if enabled
        if app.config.session.auto_restore {
            if let Err(e) = handler.restore_session(app) {
                app.status_message = format!("Session restore failed: {}", e);
            }
        }

        Ok(handler)
    }
}

/// Session commands that can be executed from the command mode
pub fn handle_session_command(app: &mut App, handler: &mut SessionHandler, parts: &[&str]) -> Result<()> {
    match parts[0] {
        "session" if parts.len() > 1 => {
            match parts[1] {
                "save" => {
                    if parts.len() > 2 {
                        handler.save_session_as(app, parts[2].to_string())?;
                        app.status_message = format!("Session saved as '{}'", parts[2]);
                    } else {
                        handler.save_session(app)?;
                        app.status_message = String::from("Session saved");
                    }
                }
                "load" if parts.len() > 2 => {
                    handler.load_session(app, parts[2])?;
                }
                "delete" if parts.len() > 2 => {
                    handler.delete_session(parts[2])?;
                    app.status_message = format!("Session '{}' deleted", parts[2]);
                }
                "list" => {
                    let sessions = handler.list_sessions()?;
                    if sessions.is_empty() {
                        app.status_message = String::from("No saved sessions");
                    } else {
                        app.status_message = format!("Sessions: {}", sessions.join(", "));
                    }
                }
                _ => {
                    app.status_message = String::from("Unknown session command. Use: save, load, delete, list");
                }
            }
        }
        "mksession" => {
            // Vim-like session save command
            if parts.len() > 1 {
                handler.save_session_as(app, parts[1].to_string())?;
                app.status_message = format!("Session saved as '{}'", parts[1]);
            } else {
                handler.save_session(app)?;
                app.status_message = String::from("Session saved");
            }
        }
        "source" if parts.len() > 1 => {
            // Vim-like session restore command
            handler.load_session(app, parts[1])?;
        }
        _ => return Err(anyhow::anyhow!("Unknown session command")),
    }

    Ok(())
}
