use serde::Serialize;
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

    #[error("Path is not valid UTF-8: {0}")]
    InvalidPath(String),
}

/// What RAID found at a folder the user wants to open as a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RepoStatus {
    Ready {
        branch: String,
    },
    /// A git repository with no commit yet; worktrees are created from `HEAD`.
    NoCommits,
    NotARepo,
    /// The folder is inside a repository but is not its root.
    NotRepoRoot {
        root: String,
    },
    Missing,
}

impl RepoStatus {
    /// Plain explanation with the next step, or None when the repo is ready.
    pub fn problem(&self) -> Option<String> {
        match self {
            RepoStatus::Ready { .. } => None,
            RepoStatus::Missing => Some("This folder does not exist.".to_string()),
            RepoStatus::NotARepo => Some(
                "This folder is not a git repository. Quests run in git worktrees, so RAID needs one."
                    .to_string(),
            ),
            RepoStatus::NoCommits => Some(
                "This repository has no commits yet. Quests start from the latest commit, so make a first commit."
                    .to_string(),
            ),
            RepoStatus::NotRepoRoot { root } => Some(format!(
                "This folder is inside a repository. Open its root instead: {root}"
            )),
        }
    }
}

const GITIGNORE_DEFAULT: &str = "node_modules/\ntarget/\ndist/\n.DS_Store\n";

fn git_output(dir: &Path, args: &[&str]) -> Result<std::process::Output, GitError> {
    Ok(Command::new("git").current_dir(dir).args(args).output()?)
}

fn git_ok(dir: &Path, args: &[&str]) -> Result<(), GitError> {
    let out = git_output(dir, args)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(GitError::CommandFailed(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ))
    }
}

const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_UNTRACKED_FILES: usize = 500;
const MAX_UNTRACKED_BYTES: u64 = 200 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub status: ChangeKind,
    /// None for binary files or files too large to diff.
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeChanges {
    pub files: Vec<FileChange>,
    pub patch: String,
    pub truncated: bool,
}

pub struct GitService;

impl GitService {
    pub fn inspect(path: &Path) -> Result<RepoStatus, GitError> {
        if !path.is_dir() {
            return Ok(RepoStatus::Missing);
        }
        let top = git_output(path, &["rev-parse", "--show-toplevel"])?;
        if !top.status.success() {
            return Ok(RepoStatus::NotARepo);
        }
        let root = String::from_utf8_lossy(&top.stdout).trim().to_string();
        let same = match (Path::new(&root).canonicalize(), path.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if !same {
            return Ok(RepoStatus::NotRepoRoot { root });
        }
        if !git_output(path, &["rev-parse", "--verify", "HEAD"])?
            .status
            .success()
        {
            return Ok(RepoStatus::NoCommits);
        }
        let branch = git_output(path, &["rev-parse", "--abbrev-ref", "HEAD"])?;
        Ok(RepoStatus::Ready {
            branch: String::from_utf8_lossy(&branch.stdout).trim().to_string(),
        })
    }

    /// Keeps `.raid/` (worktrees) out of `git status` without touching any
    /// tracked file: it goes in the local-only `info/exclude`.
    pub fn exclude_raid_dir(repo: &Path) -> Result<(), GitError> {
        let out = git_output(repo, &["rev-parse", "--git-path", "info/exclude"])?;
        if !out.status.success() {
            return Ok(());
        }
        let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let file = repo.join(rel);
        let current = std::fs::read_to_string(&file).unwrap_or_default();
        if current.lines().any(|l| l.trim() == ".raid/") {
            return Ok(());
        }
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut text = current;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(".raid/\n");
        std::fs::write(&file, text)?;
        Ok(())
    }

    /// Makes the folder ready for quests: `git init` when needed, a default
    /// `.gitignore` only if none exists, and a first commit of the folder's
    /// files. The caller must have the user's explicit consent.
    pub fn initialize(path: &Path) -> Result<RepoStatus, GitError> {
        match Self::inspect(path)? {
            RepoStatus::NotARepo => {
                git_ok(path, &["init", "-b", "main"]).or_else(|_| git_ok(path, &["init"]))?
            }
            RepoStatus::NoCommits => {}
            RepoStatus::Ready { branch } => return Ok(RepoStatus::Ready { branch }),
            other => {
                return Err(GitError::CommandFailed(
                    other
                        .problem()
                        .unwrap_or_else(|| "Cannot initialize here".to_string()),
                ));
            }
        }

        let ignore = path.join(".gitignore");
        if !ignore.exists() {
            std::fs::write(&ignore, GITIGNORE_DEFAULT)?;
        }
        Self::exclude_raid_dir(path)?;
        git_ok(path, &["add", "-A"])?;

        // Use the user's git identity; fall back only when none is configured.
        let has_identity = |key: &str| {
            git_output(path, &["config", key])
                .map(|o| o.status.success() && !o.stdout.is_empty())
                .unwrap_or(false)
        };
        let mut args: Vec<&str> = Vec::new();
        if !has_identity("user.name") {
            args.extend(["-c", "user.name=RAID"]);
        }
        if !has_identity("user.email") {
            args.extend(["-c", "user.email=raid@localhost"]);
        }
        args.extend([
            "commit",
            "--allow-empty",
            "-m",
            "Initial commit (created by RAID)",
        ]);
        git_ok(path, &args)?;

        Self::inspect(path)
    }

    /// True if the worktree has uncommitted changes. If git cannot tell, this
    /// returns true so that work is reviewed rather than silently discarded.
    pub fn has_changes(worktree: &Path) -> bool {
        if !worktree.is_dir() {
            return false;
        }
        match Command::new("git")
            .current_dir(worktree)
            .args(["status", "--porcelain"])
            .output()
        {
            Ok(out) if out.status.success() => !out.stdout.is_empty(),
            _ => true,
        }
    }

    /// Location of the quest worktree (it may not exist yet).
    pub fn quest_worktree_path(repo_path: &Path, quest_id: &str) -> PathBuf {
        repo_path
            .join(".raid")
            .join("worktrees")
            .join(format!("quest_{}", quest_id))
    }

    /// Creates an isolated Git worktree at `.raid/worktrees/quest_<quest_id>` on branch `raid/quest-<quest_id>`
    pub fn create_quest_worktree(repo_path: &Path, quest_id: &str) -> Result<PathBuf, GitError> {
        let worktree_dir = repo_path
            .join(".raid")
            .join("worktrees")
            .join(format!("quest_{}", quest_id));
        let branch_name = format!("raid/quest-{}", quest_id);

        if worktree_dir.exists() {
            info!("Worktree directory already exists at {:?}", worktree_dir);
            return Ok(worktree_dir);
        }

        if let Some(parent) = worktree_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        info!(
            "Creating git worktree at {:?} on branch {}",
            worktree_dir, branch_name
        );
        let worktree_str = worktree_dir
            .to_str()
            .ok_or_else(|| GitError::InvalidPath(worktree_dir.display().to_string()))?;
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "add", "-b", &branch_name, worktree_str, "HEAD"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // If branch already exists, try adding without -b
            if stderr.contains("already exists") {
                let fallback = Command::new("git")
                    .current_dir(repo_path)
                    .args(["worktree", "add", worktree_str, &branch_name])
                    .output()?;
                if !fallback.status.success() {
                    return Err(GitError::CommandFailed(
                        String::from_utf8_lossy(&fallback.stderr).to_string(),
                    ));
                }
            } else {
                return Err(GitError::CommandFailed(stderr.to_string()));
            }
        }

        Ok(worktree_dir)
    }

    /// Removes a git worktree safely
    pub fn remove_quest_worktree(repo_path: &Path, quest_id: &str) -> Result<(), GitError> {
        let worktree_dir = repo_path
            .join(".raid")
            .join("worktrees")
            .join(format!("quest_{}", quest_id));
        if !worktree_dir.exists() {
            return Ok(());
        }

        info!("Removing git worktree at {:?}", worktree_dir);
        let worktree_str = worktree_dir
            .to_str()
            .ok_or_else(|| GitError::InvalidPath(worktree_dir.display().to_string()))?;
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(["worktree", "remove", "--force", worktree_str])
            .output()?;

        if !output.status.success() {
            warn!(
                "Failed to remove git worktree via command: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            // Fallback manual cleanup
            let _ = std::fs::remove_dir_all(&worktree_dir);
        }

        Ok(())
    }

    /// Everything the agent changed in a quest worktree since it branched off
    /// `base_branch`: commits on the quest branch, uncommitted edits and new files.
    pub fn worktree_changes(
        worktree: &Path,
        base_branch: &str,
    ) -> Result<WorktreeChanges, GitError> {
        let base = git_output(worktree, &["merge-base", "HEAD", base_branch])
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "HEAD".to_string());

        let mut files: Vec<FileChange> = Vec::new();
        let name_status = git_output(worktree, &["diff", "--name-status", "-M", &base])?;
        let numstat = git_output(worktree, &["diff", "--numstat", "-M", &base])?;
        let counts: Vec<(Option<u32>, Option<u32>)> = String::from_utf8_lossy(&numstat.stdout)
            .lines()
            .map(|l| {
                let mut parts = l.split('\t');
                (
                    parts.next().and_then(|n| n.parse().ok()),
                    parts.next().and_then(|n| n.parse().ok()),
                )
            })
            .collect();
        for (i, line) in String::from_utf8_lossy(&name_status.stdout)
            .lines()
            .enumerate()
        {
            let mut parts = line.split('\t');
            let code = parts.next().unwrap_or("");
            let path = parts.next_back().unwrap_or("").to_string();
            let status = match code.chars().next() {
                Some('A') => ChangeKind::Added,
                Some('D') => ChangeKind::Deleted,
                Some('R') => ChangeKind::Renamed,
                _ => ChangeKind::Modified,
            };
            let (additions, deletions) = counts.get(i).copied().unwrap_or((None, None));
            files.push(FileChange {
                path,
                status,
                additions,
                deletions,
            });
        }

        let mut patch =
            String::from_utf8_lossy(&git_output(worktree, &["diff", "-M", &base])?.stdout)
                .to_string();

        let untracked = git_output(worktree, &["ls-files", "--others", "--exclude-standard"])?;
        let untracked: Vec<String> = String::from_utf8_lossy(&untracked.stdout)
            .lines()
            .map(str::to_string)
            .collect();
        let untracked_total = untracked.len();
        for path in untracked.into_iter().take(MAX_UNTRACKED_FILES) {
            let size = std::fs::metadata(worktree.join(&path))
                .map(|m| m.len())
                .unwrap_or(0);
            let mut additions = None;
            if size <= MAX_UNTRACKED_BYTES && patch.len() < MAX_PATCH_BYTES {
                // Exit code 1 only means "files differ".
                let out = git_output(worktree, &["diff", "--no-index", "--", "/dev/null", &path])?;
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                additions = Some(
                    text.lines()
                        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                        .count() as u32,
                );
                patch.push_str(&text);
            }
            files.push(FileChange {
                path,
                status: ChangeKind::Added,
                additions,
                deletions: Some(0),
            });
        }

        let truncated = patch.len() > MAX_PATCH_BYTES || untracked_total > MAX_UNTRACKED_FILES;
        if patch.len() > MAX_PATCH_BYTES {
            let mut cut = MAX_PATCH_BYTES;
            while !patch.is_char_boundary(cut) {
                cut -= 1;
            }
            patch.truncate(cut);
        }

        Ok(WorktreeChanges {
            files,
            patch,
            truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("raid-repo-{}", Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    #[test]
    fn missing_and_plain_folders_are_reported() {
        if !git_available() {
            return;
        }
        assert_eq!(
            GitService::inspect(Path::new("/no/such/folder")).ok(),
            Some(RepoStatus::Missing)
        );
        let dir = temp_dir();
        assert_eq!(GitService::inspect(&dir).ok(), Some(RepoStatus::NotARepo));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn initialize_makes_a_plain_folder_ready_without_overwriting_gitignore() {
        if !git_available() {
            return;
        }
        let dir = temp_dir();
        std::fs::write(dir.join("a.txt"), "hello").expect("write");
        std::fs::write(dir.join(".gitignore"), "secret.txt\n").expect("ignore");
        std::fs::write(dir.join("secret.txt"), "x").expect("secret");

        let status = GitService::initialize(&dir).expect("initialize");
        assert!(matches!(status, RepoStatus::Ready { .. }), "{status:?}");
        assert_eq!(
            std::fs::read_to_string(dir.join(".gitignore")).unwrap_or_default(),
            "secret.txt\n"
        );
        let tracked = git_output(&dir, &["ls-files"]).expect("ls-files");
        let tracked = String::from_utf8_lossy(&tracked.stdout).to_string();
        assert!(tracked.contains("a.txt"));
        assert!(!tracked.contains("secret.txt"));

        // Idempotent on a ready repo.
        assert!(matches!(
            GitService::initialize(&dir),
            Ok(RepoStatus::Ready { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_without_commits_and_subfolders_are_detected() {
        if !git_available() {
            return;
        }
        let dir = temp_dir();
        git_ok(&dir, &["init"]).expect("init");
        assert_eq!(GitService::inspect(&dir).ok(), Some(RepoStatus::NoCommits));

        std::fs::create_dir_all(dir.join("sub")).expect("sub");
        assert!(matches!(
            GitService::inspect(&dir.join("sub")),
            Ok(RepoStatus::NotRepoRoot { .. })
        ));
        assert!(matches!(
            GitService::initialize(&dir.join("sub")),
            Err(GitError::CommandFailed(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn worktree_changes_include_commits_edits_and_new_files() {
        if !git_available() {
            return;
        }
        let dir = temp_dir();
        std::fs::write(dir.join("keep.txt"), "one\n").expect("write");
        std::fs::write(dir.join("gone.txt"), "bye\n").expect("write");
        GitService::initialize(&dir).expect("init");
        let base = String::from_utf8_lossy(
            &git_output(&dir, &["rev-parse", "--abbrev-ref", "HEAD"])
                .expect("branch")
                .stdout,
        )
        .trim()
        .to_string();

        let wt = GitService::create_quest_worktree(&dir, "q1").expect("worktree");
        std::fs::write(wt.join("committed.txt"), "c\n").expect("write");
        git_ok(&wt, &["add", "committed.txt"]).expect("add");
        git_ok(
            &wt,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-m",
                "agent commit",
            ],
        )
        .expect("commit");
        std::fs::write(wt.join("keep.txt"), "one\ntwo\n").expect("edit");
        std::fs::remove_file(wt.join("gone.txt")).expect("delete");
        std::fs::write(wt.join("new file.txt"), "a\nb\n").expect("new");

        let changes = GitService::worktree_changes(&wt, &base).expect("changes");
        let find = |p: &str| changes.files.iter().find(|f| f.path == p).cloned();
        assert_eq!(
            find("committed.txt").map(|f| f.status),
            Some(ChangeKind::Added)
        );
        let keep = find("keep.txt").expect("keep");
        assert_eq!(
            (keep.status, keep.additions),
            (ChangeKind::Modified, Some(1))
        );
        assert_eq!(
            find("gone.txt").map(|f| f.status),
            Some(ChangeKind::Deleted)
        );
        let new = find("new file.txt").expect("untracked");
        assert_eq!((new.status, new.additions), (ChangeKind::Added, Some(2)));
        assert!(changes.patch.contains("+two"));
        assert!(changes.patch.contains("new file.txt"));
        assert!(!changes.truncated);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raid_dir_is_excluded_once() {
        if !git_available() {
            return;
        }
        let dir = temp_dir();
        git_ok(&dir, &["init"]).expect("init");
        GitService::exclude_raid_dir(&dir).expect("first");
        GitService::exclude_raid_dir(&dir).expect("second");
        let text = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap_or_default();
        assert_eq!(text.matches(".raid/").count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
