use crate::orchestrator::boss_hp::{CheckInput, CheckState};
use sqlx::{SqliteConnection, SqlitePool};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use thiserror::Error;
use tokio::process::Command;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
const SUMMARY_MAX_CHARS: usize = 2_000;
const SUMMARY_MAX_LINES: usize = 20;

#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("Could not start the command: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub passed: bool,
    pub exit_code: Option<i32>,
    /// Tail of the command output (agent/tool output is untrusted data).
    pub summary: String,
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(SUMMARY_MAX_LINES);
    let joined = lines[start..].join("\n");
    let count = joined.chars().count();
    if count > SUMMARY_MAX_CHARS {
        joined.chars().skip(count - SUMMARY_MAX_CHARS).collect()
    } else {
        joined
    }
}

/// Runs `command` through the platform shell in `cwd`. The child is killed if
/// the timeout elapses or this future is dropped.
pub async fn run_check(
    command: &str,
    cwd: &Path,
    timeout: Duration,
) -> Result<CheckOutcome, VerificationError> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn()?;
    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(output) => {
            let output = output?;
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            let exit_code = output.status.code();
            Ok(CheckOutcome {
                passed: exit_code == Some(0),
                exit_code,
                summary: tail(&text),
            })
        }
        Err(_) => Ok(CheckOutcome {
            passed: false,
            exit_code: None,
            summary: format!("Timed out after {} seconds", timeout.as_secs()),
        }),
    }
}

/// Check commands found in a project's files. Only things that exist are
/// returned; nothing is guessed.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DetectedChecks {
    pub build_cmd: Option<String>,
    pub test_cmd: Option<String>,
    pub lint_cmd: Option<String>,
    /// Files the commands were read from, e.g. `package.json`.
    pub sources: Vec<String>,
}

impl DetectedChecks {
    pub fn is_empty(&self) -> bool {
        self.build_cmd.is_none() && self.test_cmd.is_none() && self.lint_cmd.is_none()
    }
}

/// `npm init` writes this placeholder as the test script; it always fails.
fn is_placeholder_test(script: &str) -> bool {
    script.contains("no test specified")
}

fn detect_node(dir: &Path, out: &mut DetectedChecks) {
    let Ok(text) = std::fs::read_to_string(dir.join("package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else {
        return;
    };
    let runner = if dir.join("pnpm-lock.yaml").exists() {
        "pnpm run"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm run"
    };
    let script = |name: &str| scripts.get(name).and_then(|v| v.as_str());
    let mut found = false;
    if script("build").is_some() {
        out.build_cmd = Some(format!("{runner} build"));
        found = true;
    }
    if script("test").is_some_and(|s| !is_placeholder_test(s)) {
        out.test_cmd = Some(format!("{runner} test"));
        found = true;
    }
    if script("lint").is_some() {
        out.lint_cmd = Some(format!("{runner} lint"));
        found = true;
    }
    if found {
        out.sources.push("package.json".to_string());
    }
}

fn detect_cargo(dir: &Path, out: &mut DetectedChecks) {
    if !dir.join("Cargo.toml").is_file() {
        return;
    }
    let mut used = false;
    if out.build_cmd.is_none() {
        out.build_cmd = Some("cargo build".to_string());
        used = true;
    }
    if out.test_cmd.is_none() {
        out.test_cmd = Some("cargo test".to_string());
        used = true;
    }
    if out.lint_cmd.is_none() {
        out.lint_cmd = Some("cargo clippy -- -D warnings".to_string());
        used = true;
    }
    if used {
        out.sources.push("Cargo.toml".to_string());
    }
}

/// Reads build/test/lint commands from `package.json` and `Cargo.toml`.
pub fn detect_checks(dir: &Path) -> DetectedChecks {
    let mut out = DetectedChecks::default();
    detect_node(dir, &mut out);
    detect_cargo(dir, &mut out);
    out
}

/// Latest recorded evidence for one check across all tasks of a quest.
/// `configured` is the project's command for this check, if any.
pub async fn latest_check_input(
    conn: &mut SqliteConnection,
    quest_id: &str,
    kind: &str,
    configured: Option<&str>,
) -> Result<CheckInput, sqlx::Error> {
    if configured.map(str::trim).unwrap_or("").is_empty() {
        return Ok(CheckInput::NotSet);
    }
    let row: Option<(String, Option<i64>)> = sqlx::query_as(
        "SELECT status, exit_code FROM verification_runs
         WHERE verification_type = $1
           AND task_id IN (SELECT id FROM tasks WHERE quest_id = $2)
           AND status IN ('passed', 'failed')
         ORDER BY started_at DESC, rowid DESC LIMIT 1",
    )
    .bind(kind)
    .bind(quest_id)
    .fetch_optional(conn)
    .await?;

    Ok(match row {
        None => CheckInput::NotRun,
        Some((status, exit_code)) => CheckInput::Ran {
            passed: status == "passed",
            exit_code: exit_code.and_then(|c| i32::try_from(c).ok()),
        },
    })
}

/// True when at least one check is configured and every configured check's
/// latest run passed with exit code 0.
pub async fn all_configured_checks_passed(
    conn: &mut SqliteConnection,
    quest_id: &str,
    build: Option<&str>,
    test: Option<&str>,
    lint: Option<&str>,
) -> Result<bool, sqlx::Error> {
    let mut configured = 0;
    for (kind, cmd) in [("build", build), ("test", test), ("lint", lint)] {
        let input = latest_check_input(&mut *conn, quest_id, kind, cmd).await?;
        match CheckState::from(input) {
            CheckState::NotSet => {}
            CheckState::Passed => configured += 1,
            _ => return Ok(false),
        }
    }
    Ok(configured > 0)
}

/// Marks runs left `pending`/`running` by a crash or restart as failed, so no
/// check stays stuck in progress (CLAUDE.md Definition of Done #4).
pub async fn recover_interrupted(pool: &SqlitePool) -> Result<u64, VerificationError> {
    let result = sqlx::query(
        "UPDATE verification_runs
         SET status = 'failed', ended_at = CURRENT_TIMESTAMP,
             result_summary = 'Interrupted: the app stopped before this check finished'
         WHERE status IN ('pending', 'running')",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod detect_tests {
    use super::*;

    fn dir_with(files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("raid-detect-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        for (name, text) in files {
            std::fs::write(dir.join(name), text).expect("write");
        }
        dir
    }

    #[test]
    fn only_existing_scripts_are_detected() {
        let dir = dir_with(&[(
            "package.json",
            r#"{"scripts":{"dev":"vite","build":"vite build"}}"#,
        )]);
        let found = detect_checks(&dir);
        assert_eq!(found.build_cmd.as_deref(), Some("npm run build"));
        assert_eq!(found.test_cmd, None);
        assert_eq!(found.lint_cmd, None);
        assert_eq!(found.sources, vec!["package.json"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn npm_placeholder_test_script_is_ignored() {
        let dir = dir_with(&[(
            "package.json",
            r#"{"scripts":{"test":"echo \"Error: no test specified\" && exit 1","lint":"eslint ."}}"#,
        )]);
        let found = detect_checks(&dir);
        assert_eq!(found.test_cmd, None);
        assert_eq!(found.lint_cmd.as_deref(), Some("npm run lint"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_manager_follows_the_lockfile() {
        let dir = dir_with(&[
            ("package.json", r#"{"scripts":{"build":"x"}}"#),
            ("pnpm-lock.yaml", ""),
        ]);
        assert_eq!(
            detect_checks(&dir).build_cmd.as_deref(),
            Some("pnpm run build")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cargo_projects_get_cargo_commands() {
        let dir = dir_with(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
        let found = detect_checks(&dir);
        assert_eq!(found.build_cmd.as_deref(), Some("cargo build"));
        assert_eq!(found.test_cmd.as_deref(), Some("cargo test"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_or_broken_projects_detect_nothing() {
        let dir = dir_with(&[("package.json", "not json")]);
        assert!(detect_checks(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn tmp() -> &'static Path {
        Path::new("/tmp")
    }

    #[tokio::test]
    async fn exit_zero_passes() {
        let out = run_check("true", tmp(), Duration::from_secs(5)).await;
        assert!(matches!(out, Ok(ref o) if o.passed && o.exit_code == Some(0)));
    }

    #[tokio::test]
    async fn nonzero_exit_fails_with_code_and_output() {
        let out = run_check("echo broken; exit 3", tmp(), Duration::from_secs(5)).await;
        match out {
            Ok(o) => {
                assert!(!o.passed);
                assert_eq!(o.exit_code, Some(3));
                assert!(o.summary.contains("broken"));
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn timeout_fails_without_exit_code() {
        let out = run_check("sleep 5", tmp(), Duration::from_millis(100)).await;
        match out {
            Ok(o) => {
                assert!(!o.passed);
                assert_eq!(o.exit_code, None);
                assert!(o.summary.contains("Timed out"));
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn missing_directory_is_an_error_not_a_pass() {
        let out = run_check("true", Path::new("/no/such/dir"), Duration::from_secs(5)).await;
        assert!(out.is_err());
    }

    #[test]
    fn tail_keeps_only_the_last_lines() {
        let text = (0..100)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let t = tail(&text);
        assert!(t.ends_with("99"));
        assert!(!t.contains("\n10\n"));
    }
}
