use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct GitService;

impl GitService {
    /// Creates an isolated Git worktree at `.raid/worktrees/quest_<quest_id>` on branch `raid/quest-<quest_id>`
    pub fn create_quest_worktree(repo_path: &Path, quest_id: &str) -> Result<PathBuf, GitError> {
        let worktree_dir = repo_path.join(".raid").join("worktrees").join(format!("quest_{}", quest_id));
        let branch_name = format!("raid/quest-{}", quest_id);

        if worktree_dir.exists() {
            info!("Worktree directory already exists at {:?}", worktree_dir);
            return Ok(worktree_dir);
        }

        if let Some(parent) = worktree_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        info!("Creating git worktree at {:?} on branch {}", worktree_dir, branch_name);
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "add", "-b", &branch_name, worktree_dir.to_str().unwrap(), "HEAD"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If branch already exists, try adding without -b
            if stderr.contains("already exists") {
                let fallback = Command::new("git")
                    .current_dir(repo_path)
                    .args(["worktree", "add", worktree_dir.to_str().unwrap(), &branch_name])
                    .output()?;
                if !fallback.status.success() {
                    return Err(GitError::CommandFailed(String::from_utf8_lossy(&fallback.stderr).to_string()));
                }
            } else {
                return Err(GitError::CommandFailed(stderr.to_string()));
            }
        }

        Ok(worktree_dir)
    }

    /// Removes a git worktree safely
    pub fn remove_quest_worktree(repo_path: &Path, quest_id: &str) -> Result<(), GitError> {
        let worktree_dir = repo_path.join(".raid").join("worktrees").join(format!("quest_{}", quest_id));
        if !worktree_dir.exists() {
            return Ok(());
        }

        info!("Removing git worktree at {:?}", worktree_dir);
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "remove", "--force", worktree_dir.to_str().unwrap()])
            .output()?;

        if !output.status.success() {
            warn!("Failed to remove git worktree via command: {}", String::from_utf8_lossy(&output.stderr));
            // Fallback manual cleanup
            let _ = std::fs::remove_dir_all(&worktree_dir);
        }

        Ok(())
    }

    /// Gets diff of changes in the quest branch compared to current main/HEAD
    pub fn get_quest_diff(repo_path: &Path, quest_id: &str) -> Result<String, GitError> {
        let branch_name = format!("raid/quest-{}", quest_id);
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["diff", "HEAD", &branch_name])
            .output()?;

        if !output.status.success() {
            return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).to_string()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
