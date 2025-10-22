use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub version: String,
    pub timestamp: SystemTime,
    pub name: String,
    pub buffers: Vec<BufferState>,
    pub active_buffer: usize,
    pub split_layout: Option<SplitLayout>,
    pub window_state: WindowState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferState {
    pub file_path: Option<PathBuf>,
    pub cursor_position: (usize, usize),
    pub viewport_offset: usize,
    pub selection: Option<SelectionState>,
    pub is_modified: bool,
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionState {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitLayout {
    pub direction: SplitDirection,
    pub panes: Vec<PaneState>,
    pub active_pane: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneState {
    pub buffer_index: usize,
    pub viewport_offset: usize,
    pub cursor_position: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub width: u16,
    pub height: u16,
    pub sidebar_visible: bool,
    pub sidebar_width: u16,
}

impl Session {
    pub fn new(name: String) -> Self {
        Session {
            version: String::from("1.0.0"),
            timestamp: SystemTime::now(),
            name,
            buffers: Vec::new(),
            active_buffer: 0,
            split_layout: None,
            window_state: WindowState {
                width: 80,
                height: 24,
                sidebar_visible: false,
                sidebar_width: 25,
            },
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let session_dir = path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid session path"))?;

        if !session_dir.exists() {
            fs::create_dir_all(session_dir)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let session: Session = serde_json::from_str(&contents)?;
        Ok(session)
    }

    pub fn get_session_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        let session_dir = config_dir.join("lektor").join("sessions");
        Ok(session_dir)
    }

    pub fn get_default_session_path() -> Result<PathBuf> {
        let session_dir = Self::get_session_dir()?;
        Ok(session_dir.join("default.json"))
    }

    pub fn get_workspace_session_path() -> Result<PathBuf> {
        let current_dir = std::env::current_dir()?;
        Ok(current_dir.join(".lektor").join("session.json"))
    }

    pub fn list_sessions() -> Result<Vec<String>> {
        let session_dir = Self::get_session_dir()?;
        let mut sessions = Vec::new();

        if session_dir.exists() {
            for entry in fs::read_dir(session_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        sessions.push(stem.to_string());
                    }
                }
            }
        }

        Ok(sessions)
    }

    pub fn delete(name: &str) -> Result<()> {
        let session_dir = Self::get_session_dir()?;
        let session_path = session_dir.join(format!("{}.json", name));

        if session_path.exists() {
            fs::remove_file(session_path)?;
        }

        Ok(())
    }
}

pub struct SessionManager {
    current_session: Option<Session>,
    auto_save: bool,
    workspace_session: bool,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            current_session: None,
            auto_save: false,
            workspace_session: false,
        }
    }

    pub fn set_auto_save(&mut self, enabled: bool) {
        self.auto_save = enabled;
    }

    pub fn set_workspace_session(&mut self, enabled: bool) {
        self.workspace_session = enabled;
    }

    pub fn create_session(&mut self, name: String) -> &mut Session {
        let session = Session::new(name);
        self.current_session = Some(session);
        self.current_session.as_mut().unwrap()
    }

    pub fn save_current(&self) -> Result<()> {
        if let Some(session) = &self.current_session {
            let path = if self.workspace_session {
                Session::get_workspace_session_path()?
            } else {
                let session_dir = Session::get_session_dir()?;
                session_dir.join(format!("{}.json", session.name))
            };

            session.save(&path)?;
        }
        Ok(())
    }

    pub fn load_session(&mut self, name: &str) -> Result<Session> {
        let session_dir = Session::get_session_dir()?;
        let path = session_dir.join(format!("{}.json", name));

        let session = Session::load(&path)?;
        self.current_session = Some(session.clone());
        Ok(session)
    }

    pub fn load_workspace_session(&mut self) -> Result<Option<Session>> {
        let path = Session::get_workspace_session_path()?;

        if path.exists() {
            let session = Session::load(&path)?;
            self.current_session = Some(session.clone());
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }

    pub fn get_current(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    pub fn get_current_mut(&mut self) -> Option<&mut Session> {
        self.current_session.as_mut()
    }

    pub fn should_auto_save(&self) -> bool {
        self.auto_save && self.current_session.is_some()
    }
}

pub fn calculate_content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
