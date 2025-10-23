use crate::buffer::TextBuffer;
use crate::split::{SplitManager, SplitDirection};
use anyhow::Result;
use git2::Repository;
use ropey::Rope;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiffInfo {
    pub head_buffer_index: usize,
    pub working_buffer_index: usize,
    pub added_lines: Vec<usize>,    // Lines added in working version
    pub deleted_lines: Vec<usize>,  // Lines deleted from HEAD version
    pub modified_lines: Vec<usize>, // Lines modified in working version
}

impl super::App {
    pub(super) fn compute_diff(&self, head_lines: Vec<&str>, working_lines: Vec<&str>) -> DiffInfo {
        let mut added_lines = Vec::new();
        let mut deleted_lines = Vec::new();
        let mut modified_lines = Vec::new();

        // Use a simple LCS-based approach
        let mut i = 0;
        let mut j = 0;

        while i < head_lines.len() || j < working_lines.len() {
            if i >= head_lines.len() {
                // All remaining lines in working are additions
                added_lines.push(j);
                j += 1;
            } else if j >= working_lines.len() {
                // All remaining lines in HEAD are deletions
                deleted_lines.push(i);
                i += 1;
            } else if head_lines[i] == working_lines[j] {
                // Lines match, move both pointers
                i += 1;
                j += 1;
            } else {
                // Look ahead to determine if this is add/delete or modification
                let mut found_in_working = None;
                let mut found_in_head = None;

                // Look for current head line in next few working lines
                for k in (j + 1)..((j + 5).min(working_lines.len())) {
                    if head_lines[i] == working_lines[k] {
                        found_in_working = Some(k);
                        break;
                    }
                }

                // Look for current working line in next few head lines
                for k in (i + 1)..((i + 5).min(head_lines.len())) {
                    if head_lines[k] == working_lines[j] {
                        found_in_head = Some(k);
                        break;
                    }
                }

                match (found_in_working, found_in_head) {
                    (Some(k), _) => {
                        // Current head line found ahead in working
                        // Lines j to k-1 in working are additions
                        for add_idx in j..k {
                            added_lines.push(add_idx);
                        }
                        j = k;
                    }
                    (None, Some(k)) => {
                        // Current working line found ahead in head
                        // Lines i to k-1 in head are deletions
                        for del_idx in i..k {
                            deleted_lines.push(del_idx);
                        }
                        i = k;
                    }
                    (None, None) => {
                        // No match found, treat as modification
                        modified_lines.push(j);
                        i += 1;
                        j += 1;
                    }
                }
            }
        }

        DiffInfo {
            head_buffer_index: 0, // Will be set later
            working_buffer_index: 0, // Will be set later
            added_lines,
            deleted_lines,
            modified_lines,
        }
    }

    pub fn show_diff_view(&mut self) -> Result<()> {
        // Get the current file path
        let current_buffer_index = if let Some(split_manager) = &self.split_manager {
            split_manager.get_active_buffer_index().unwrap_or(self.buffer_manager.current_index)
        } else {
            self.buffer_manager.current_index
        };

        let file_path = match &self.buffer_manager.buffers[current_buffer_index].file_path {
            Some(path) => path.clone(),
            None => {
                self.status_message = String::from("No file to diff - save the file first");
                return Ok(());
            }
        };

        // Get HEAD version of the file
        let head_content = self.get_file_from_head(&file_path)?;

        if head_content.is_none() {
            self.status_message = String::from("File not in git repository or not committed");
            return Ok(());
        }

        let head_content = head_content.unwrap();

        // Get working version content
        let working_content = self.buffer_manager.buffers[current_buffer_index].content.to_string();

        // Compute diff
        let head_lines: Vec<&str> = head_content.lines().collect();
        let working_lines: Vec<&str> = working_content.lines().collect();
        let mut diff_info = self.compute_diff(head_lines, working_lines);

        // Create a new buffer for HEAD version with special name
        let head_buffer_name = format!("{} (HEAD)", file_path.display());
        let mut head_buffer = TextBuffer::new();
        head_buffer.content = Rope::from_str(&head_content);
        head_buffer.file_path = Some(PathBuf::from(&head_buffer_name));
        head_buffer.modified = false;

        // Add the HEAD buffer
        self.buffer_manager.buffers.push(head_buffer);
        let head_buffer_index = self.buffer_manager.buffers.len() - 1;

        // Update diff info with buffer indices
        diff_info.head_buffer_index = head_buffer_index;
        diff_info.working_buffer_index = current_buffer_index;

        // Store diff info
        self.diff_info = Some(diff_info);

        // Initialize split manager if needed
        if self.split_manager.is_none() {
            let terminal_size = crossterm::terminal::size()?;
            let sidebar_width = if self.show_sidebar { self.config.sidebar.width } else { 0 };
            self.split_manager = Some(SplitManager::new(
                current_buffer_index,
                terminal_size.0,
                terminal_size.1.saturating_sub(2),
                sidebar_width,
            ));
        }

        // Create vertical split for side-by-side diff
        if let Some(split_manager) = &mut self.split_manager {
            split_manager.split_current(SplitDirection::Vertical, head_buffer_index);
            self.status_message = format!("Diff view: {} | {} (HEAD)", file_path.display(), file_path.display());
        }

        Ok(())
    }

    fn get_file_from_head(&self, file_path: &Path) -> Result<Option<String>> {
        // Get the absolute path of the file
        let abs_file_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            std::env::current_dir()?.join(file_path)
        };

        // Try to discover git repository from the file's location
        let repo = match Repository::discover(&abs_file_path) {
            Ok(repo) => repo,
            Err(_) => return Ok(None),
        };

        // Get HEAD commit
        let head = match repo.head() {
            Ok(head) => head,
            Err(_) => return Ok(None),
        };

        let oid = head.target().ok_or_else(|| anyhow::anyhow!("HEAD has no target"))?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        // Get relative path from repository root
        let repo_path = repo.workdir().ok_or_else(|| anyhow::anyhow!("No workdir"))?;
        let relative_path = match abs_file_path.strip_prefix(repo_path) {
            Ok(rel) => rel,
            Err(_) => {
                // File is outside repository
                return Ok(None);
            }
        };

        // Find the file in the tree
        let entry = match tree.get_path(relative_path) {
            Ok(entry) => entry,
            Err(_) => return Ok(None), // File not in HEAD
        };

        let object = repo.find_blob(entry.id())?;
        let content = std::str::from_utf8(object.content())?.to_string();

        Ok(Some(content))
    }
}
