use anyhow::Result;
use git2::{Repository, Status, StatusOptions};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub git_status: Option<GitStatus>,
    pub level: usize,
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

    pub fn toggle_hidden_files(&mut self) -> Result<()> {
        self.show_hidden = !self.show_hidden;
        self.refresh()?;
        Ok(())
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.entries.clear();
        self.update_git_statuses()?;
        let root_path = self.root_path.clone();
        self.load_entries(&root_path, 0)?;
        self.selected_index = 0;
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
}
