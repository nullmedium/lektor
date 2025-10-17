use anyhow::Result;
use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum SidebarItem {
    File(PathBuf),
    Directory(PathBuf, bool),  // path, is_expanded
    Parent,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SidebarMode {
    Files,
    Buffers,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub git_status: Option<GitStatus>,
    pub level: usize,
    pub is_buffer: bool,  // True if this is a buffer entry, not a file entry
    pub buffer_index: Option<usize>,  // Index in the buffer manager
    pub is_modified: bool,  // For buffer entries
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GitStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Ignored,
    Conflicted,
}

pub struct Sidebar {
    pub entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub root_path: PathBuf,
    pub show_hidden: bool,
    pub git_repo: Option<Repository>,
    pub git_statuses: HashMap<PathBuf, GitStatus>,
    pub mode: SidebarMode,
}

impl Sidebar {
    pub fn new(root_path: PathBuf) -> Result<Self> {
        let git_repo = Repository::open(&root_path).ok();
        let mut sidebar = Self {
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            root_path: root_path.clone(),
            show_hidden: false,
            git_repo,
            git_statuses: HashMap::new(),
            mode: SidebarMode::Files,
        };

        sidebar.update_git_statuses()?;
        sidebar.load_entries(&root_path, 0)?;

        Ok(sidebar)
    }

    pub fn update_git_statuses(&mut self) -> Result<()> {
        self.git_statuses.clear();

        if let Some(repo) = &self.git_repo {
            let mut opts = StatusOptions::new();
            opts.include_untracked(true)
                .recurse_untracked_dirs(true)
                .include_ignored(false);

            let statuses = repo.statuses(Some(&mut opts))?;

            for entry in statuses.iter() {
                if let Some(path_str) = entry.path() {
                    let path = self.root_path.join(path_str);
                    let status = entry.status();

                    let git_status = if status.contains(Status::WT_NEW) {
                        Some(GitStatus::Added)
                    } else if status.contains(Status::WT_MODIFIED) {
                        Some(GitStatus::Modified)
                    } else if status.contains(Status::WT_DELETED) {
                        Some(GitStatus::Deleted)
                    } else if status.contains(Status::WT_RENAMED) {
                        Some(GitStatus::Renamed)
                    } else if status.contains(Status::CONFLICTED) {
                        Some(GitStatus::Conflicted)
                    } else if status.contains(Status::IGNORED) {
                        Some(GitStatus::Ignored)
                    } else if status.contains(Status::INDEX_NEW) {
                        Some(GitStatus::Added)
                    } else {
                        None
                    };

                    if let Some(git_status) = git_status {
                        self.git_statuses.insert(path, git_status);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn load_entries(&mut self, path: &Path, level: usize) -> Result<()> {
        let mut entries = Vec::new();

        // Add parent directory entry ".." if we're at root level and not at filesystem root
        if level == 0 && path.parent().is_some() {
            if let Some(parent_path) = path.parent() {
                entries.push(FileEntry {
                    path: parent_path.to_path_buf(),
                    name: "..".to_string(),
                    is_dir: true,
                    is_expanded: false,
                    git_status: None,
                    level: 0,
                    is_buffer: false,
                    buffer_index: None,
                    is_modified: false,
                });
            }
        }

        if path.is_dir() {
            let mut dir_entries: Vec<_> = fs::read_dir(path)?
                .filter_map(|entry| entry.ok())
                .collect();

            dir_entries.sort_by_key(|entry| {
                let is_dir = entry.path().is_dir();
                let name = entry.file_name().to_string_lossy().to_lowercase();
                (!is_dir, name)
            });

            for entry in dir_entries {
                let entry_path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                if !self.show_hidden && name.starts_with('.') {
                    continue;
                }

                let git_status = self.git_statuses.get(&entry_path).copied();

                entries.push(FileEntry {
                    path: entry_path.clone(),
                    name,
                    is_dir: entry_path.is_dir(),
                    is_expanded: false,
                    git_status,
                    level,
                    is_buffer: false,
                    buffer_index: None,
                    is_modified: false,
                });
            }
        }

        if level == 0 {
            self.entries = entries;
        } else {
            let insert_pos = self.find_insert_position(path)?;
            for (i, entry) in entries.into_iter().enumerate() {
                self.entries.insert(insert_pos + i, entry);
            }
        }

        Ok(())
    }

    fn find_insert_position(&self, parent_path: &Path) -> Result<usize> {
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.path == parent_path {
                return Ok(i + 1);
            }
        }
        Ok(self.entries.len())
    }

    pub fn toggle_expanded(&mut self) -> Result<()> {
        if let Some(entry) = self.entries.get_mut(self.selected_index) {
            // Don't try to expand ".." parent directory entry
            if entry.name == ".." {
                return Ok(());
            }

            if entry.is_dir {
                entry.is_expanded = !entry.is_expanded;

                if entry.is_expanded {
                    let path = entry.path.clone();
                    let level = entry.level + 1;
                    self.load_entries(&path, level)?;
                } else {
                    self.collapse_dir(self.selected_index);
                }
            }
        }
        Ok(())
    }

    fn collapse_dir(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let level = entry.level;
            let mut i = index + 1;

            while i < self.entries.len() {
                if self.entries[i].level > level {
                    self.entries.remove(i);
                } else {
                    break;
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected_index < self.entries.len().saturating_sub(1) {
            self.selected_index += 1;
        }
    }

    pub fn get_selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected_index).map(|e| &e.path)
    }

    pub fn is_parent_selected(&self) -> bool {
        self.entries.get(self.selected_index)
            .map(|e| e.name == "..")
            .unwrap_or(false)
    }

    pub fn toggle_hidden_files(&mut self) -> Result<()> {
        self.show_hidden = !self.show_hidden;
        self.refresh()?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        if self.mode == SidebarMode::Files {
            self.entries.clear();
            self.update_git_statuses()?;
            let root_path = self.root_path.clone();
            self.load_entries(&root_path, 0)?;
            self.selected_index = 0;
        }
        // For buffer mode, refresh should be called explicitly with load_buffer_list
        Ok(())
    }

    pub fn navigate_to_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.root_path.parent() {
            self.root_path = parent.to_path_buf();
            self.refresh()?;
        }
        Ok(())
    }

    pub fn get_visible_entries(&self, height: usize) -> &[FileEntry] {
        let end = (self.scroll_offset + height).min(self.entries.len());
        &self.entries[self.scroll_offset..end]
    }

    pub fn update_scroll(&mut self, viewport_height: usize) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.selected_index.saturating_sub(viewport_height - 1);
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            SidebarMode::Files => SidebarMode::Buffers,
            SidebarMode::Buffers => SidebarMode::Files,
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    pub fn load_buffer_list(&mut self, buffers: Vec<(usize, String, PathBuf, bool)>) {
        self.entries.clear();

        for (index, name, path, is_modified) in buffers {
            self.entries.push(FileEntry {
                path: path.clone(),
                name: format!("{}. {}{}", index + 1, name, if is_modified { " [+]" } else { "" }),
                is_dir: false,
                is_expanded: false,
                git_status: None,
                level: 0,
                is_buffer: true,
                buffer_index: Some(index),
                is_modified,
            });
        }

        if self.selected_index >= self.entries.len() && !self.entries.is_empty() {
            self.selected_index = self.entries.len() - 1;
        }
    }

    pub fn get_selected_buffer_index(&self) -> Option<usize> {
        if self.mode == SidebarMode::Buffers {
            self.entries.get(self.selected_index)
                .and_then(|e| e.buffer_index)
        } else {
            None
        }
    }

    pub fn handle_click(&mut self, row: usize) {
        // Don't adjust the row - mouse coordinates are already correct
        // Check if the click is within the visible range
        let visible_index = row + self.scroll_offset;

        if visible_index < self.entries.len() {
            self.selected_index = visible_index;
        }
    }

    pub fn get_selected_item(&self) -> Option<SidebarItem> {
        if let Some(entry) = self.entries.get(self.selected_index) {
            if entry.name == ".." {
                Some(SidebarItem::Parent)
            } else if entry.is_dir {
                Some(SidebarItem::Directory(entry.path.clone(), entry.is_expanded))
            } else {
                Some(SidebarItem::File(entry.path.clone()))
            }
        } else {
            None
        }
    }

    pub fn toggle_directory(&mut self, path: &PathBuf) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|e| &e.path == path && e.is_dir) {
            entry.is_expanded = !entry.is_expanded;
            // Rebuild the entries to show/hide directory contents
            let root = self.root_path.clone();
            self.load_entries(&root, 0)?;
        }
        Ok(())
    }
}
