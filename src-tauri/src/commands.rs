use crate::orchestrator::boss_hp::{BossHp, LatestVerification, compute_boss_hp};
use crate::orchestrator::lifecycle;
use crate::orchestrator::state::{
    Evidence, TaskStatus, TransitionContext, allowed_actions, transition_task,
};
use crate::persistence::models::{Character, Project, Quest, Task};
use crate::providers::{AgentProvider, ClaudeCodeAdapter, SessionConfig};
use crate::repo::{GitService, RepoStatus, WorktreeChanges};
use crate::verification;
use chrono::Utc;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use tracing::warn;
use uuid::Uuid;

pub struct AppState {
    pub db: SqlitePool,
    pub claude_adapter: Arc<ClaudeCodeAdapter>,
}

async fn inspect_blocking(repo_path: &str) -> Result<RepoStatus, String> {
    let path = repo_path.to_string();
    tokio::task::spawn_blocking(move || GitService::inspect(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Could not run git. Is git installed? ({e})"))
}

/// Tells the UI whether a folder can be opened, and why not.
#[derive(Serialize)]
pub struct RepoReport {
    #[serde(flatten)]
    pub status: RepoStatus,
    pub problem: Option<String>,
}

#[tauri::command]
pub async fn inspect_repo(repo_path: String) -> Result<RepoReport, String> {
    let status = inspect_blocking(&repo_path).await?;
    let problem = status.problem();
    Ok(RepoReport { status, problem })
}

/// Runs `git init` (if needed), writes a default `.gitignore` only if none
/// exists, and makes a first commit. Call only after the user confirmed.
#[tauri::command]
pub async fn initialize_repo(repo_path: String) -> Result<RepoReport, String> {
    let path = repo_path.clone();
    let status = tokio::task::spawn_blocking(move || GitService::initialize(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Could not initialize git: {e}"))?;
    let problem = status.problem();
    Ok(RepoReport { status, problem })
}

#[tauri::command]
pub async fn open_project(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let path = Path::new(&repo_path);
    let status = inspect_blocking(&repo_path).await?;
    let default_branch = match &status {
        RepoStatus::Ready { branch } => branch.clone(),
        other => {
            return Err(other
                .problem()
                .unwrap_or_else(|| "This folder cannot be opened.".to_string()));
        }
    };
    if let Err(e) = GitService::exclude_raid_dir(path) {
        warn!("Could not exclude .raid/ from git status: {e}");
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Project".to_string());

    let existing: Option<Project> =
        sqlx::query_as("SELECT * FROM projects WHERE repository_path = $1")
            .bind(&repo_path)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(project) = existing {
        Ok(project)
    } else {
        let new_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let project = Project {
            id: new_id.clone(),
            name,
            repository_path: repo_path.clone(),
            default_branch,
            build_cmd: None,
            test_cmd: None,
            lint_cmd: None,
            created_at: now,
            updated_at: now,
        };

        sqlx::query(
            "INSERT INTO projects (id, name, repository_path, default_branch, created_at, updated_at) 
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&project.id)
        .bind(&project.name)
        .bind(&project.repository_path)
        .bind(&project.default_branch)
        .bind(project.created_at)
        .bind(project.updated_at)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        Ok(project)
    }
}

#[tauri::command]
pub async fn get_quests(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Quest>, String> {
    sqlx::query_as("SELECT * FROM quests WHERE project_id = $1 ORDER BY created_at DESC")
        .bind(project_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_quest(
    project_id: String,
    title: String,
    description: String,
    priority: String,
    boss_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<Quest, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let quest = Quest {
        id: id.clone(),
        project_id,
        title,
        description,
        status: "created".to_string(),
        priority,
        boss_name: boss_name.or_else(|| Some("Feature Golem".to_string())),
        boss_max_hp: 100,
        boss_current_hp: 100,
        created_at: now,
        completed_at: None,
    };

    sqlx::query(
        "INSERT INTO quests (id, project_id, title, description, status, priority, boss_name, boss_max_hp, boss_current_hp, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
    )
    .bind(&quest.id)
    .bind(&quest.project_id)
    .bind(&quest.title)
    .bind(&quest.description)
    .bind(&quest.status)
    .bind(&quest.priority)
    .bind(&quest.boss_name)
    .bind(quest.boss_max_hp)
    .bind(quest.boss_current_hp)
    .bind(quest.created_at)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(quest)
}

/// A task plus what the UI may do with it, all derived by the backend.
#[derive(Serialize)]
pub struct TaskView {
    #[serde(flatten)]
    pub task: Task,
    pub failure_reason: Option<String>,
    pub active_session_id: Option<String>,
    pub allowed_actions: Vec<&'static str>,
}

#[tauri::command]
pub async fn get_tasks(
    quest_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskView>, String> {
    let tasks: Vec<Task> =
        sqlx::query_as("SELECT * FROM tasks WHERE quest_id = $1 ORDER BY created_at ASC")
            .bind(&quest_id)
            .fetch_all(&state.db)
            .await
            .map_err(|e| e.to_string())?;

    let mut views = Vec::with_capacity(tasks.len());
    for task in tasks {
        let (failure_reason,): (Option<String>,) =
            sqlx::query_as("SELECT failure_reason FROM tasks WHERE id = $1")
                .bind(&task.id)
                .fetch_one(&state.db)
                .await
                .map_err(|e| e.to_string())?;
        let session: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM agent_sessions
             WHERE task_id = $1 AND status IN ('preparing', 'running', 'waiting', 'waiting_approval')
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(&task.id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?;
        let actions = TaskStatus::parse(&task.status)
            .map(|st| allowed_actions(st, session.is_some()))
            .unwrap_or_default();
        views.push(TaskView {
            task,
            failure_reason,
            active_session_id: session.map(|(id,)| id),
            allowed_actions: actions,
        });
    }
    Ok(views)
}

#[tauri::command]
pub async fn create_task(
    quest_id: String,
    title: String,
    description: String,
    priority: String,
    weight_percent: f64,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let task = Task {
        id: id.clone(),
        quest_id,
        parent_task_id: None,
        title,
        description,
        status: "ready".to_string(),
        assigned_character_id: None,
        priority,
        weight_percent,
        created_at: now,
        updated_at: now,
    };

    sqlx::query(
        "INSERT INTO tasks (id, quest_id, title, description, status, priority, weight_percent, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(&task.id)
    .bind(&task.quest_id)
    .bind(&task.title)
    .bind(&task.description)
    .bind(&task.status)
    .bind(&task.priority)
    .bind(task.weight_percent)
    .bind(task.created_at)
    .bind(task.updated_at)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(task)
}

#[tauri::command]
pub async fn get_characters(state: State<'_, AppState>) -> Result<Vec<Character>, String> {
    let chars: Vec<Character> = sqlx::query_as("SELECT * FROM characters")
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    // Seed default 3 characters if table is empty
    if chars.is_empty() {
        let defaults = vec![
            ("Aron", "warrior", "tank"),
            ("Lyra", "mage", "dps"),
            ("Kael", "rogue", "scout"),
        ];

        for (name, class, role) in defaults {
            let id = Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO characters (id, display_name, class, party_role, level, experience, state)
                 VALUES ($1, $2, $3, $4, 1, 0, 'idle')"
            )
            .bind(id)
            .bind(name)
            .bind(class)
            .bind(role)
            .execute(&state.db)
            .await;
        }

        return sqlx::query_as("SELECT * FROM characters")
            .fetch_all(&state.db)
            .await
            .map_err(|e| e.to_string());
    }

    Ok(chars)
}

const AGENT_MODEL: &str = "claude-3-7-sonnet";

#[tauri::command]
pub async fn start_quest_task(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Task not found: {}", e))?;

    let project: Project = sqlx::query_as(
        "SELECT p.* FROM projects p JOIN quests q ON q.project_id = p.id WHERE q.id = $1",
    )
    .bind(&task.quest_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Project not found: {}", e))?;

    let repo = Path::new(&project.repository_path);
    let worktree_dir = GitService::quest_worktree_path(repo, &task.quest_id);

    // Validated transition to Running first: a double click or a second
    // running task is rejected here, before any worktree or process exists.
    let session_id = lifecycle::begin_session(
        &state.db,
        &task.id,
        &worktree_dir.to_string_lossy(),
        &format!("raid/quest-{}", task.quest_id),
        AGENT_MODEL,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Isolation: the agent always runs in the quest worktree, never the main tree.
    if let Err(e) = GitService::create_quest_worktree(repo, &task.quest_id) {
        let reason = format!("Could not create the git worktree: {e}");
        let _ = lifecycle::fail_start(&state.db, &task.id, &session_id, &reason).await;
        return Err(reason);
    }

    let config = SessionConfig {
        session_id: session_id.clone(),
        task_id: task.id.clone(),
        quest_id: task.quest_id,
        task_title: task.title,
        task_description: task.description,
        worktree_path: worktree_dir,
        model: AGENT_MODEL.to_string(),
    };

    if let Err(e) = state.claude_adapter.start_session(config).await {
        let reason = format!("Could not start the agent session: {e}");
        let _ = lifecycle::fail_start(&state.db, &task.id, &session_id, &reason).await;
        return Err(reason);
    }

    Ok(session_id)
}

#[tauri::command]
pub async fn send_agent_input(
    session_id: String,
    input: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .claude_adapter
        .send_input(&session_id, &input)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = lifecycle::on_approval_answered(&state.db, &session_id).await {
        warn!("Could not record the approval answer for {session_id}: {e}");
    }
    Ok(())
}

#[tauri::command]
pub async fn cancel_agent_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .claude_adapter
        .cancel(&session_id)
        .await
        .map_err(|e| e.to_string())
}

/// Files the agent changed in the quest worktree, with the full patch.
#[tauri::command]
pub async fn get_quest_changes(
    quest_id: String,
    state: State<'_, AppState>,
) -> Result<Option<WorktreeChanges>, String> {
    let project: Project = sqlx::query_as(
        "SELECT p.* FROM projects p JOIN quests q ON q.project_id = p.id WHERE q.id = $1",
    )
    .bind(&quest_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Quest not found: {e}"))?;

    let worktree = GitService::quest_worktree_path(Path::new(&project.repository_path), &quest_id);
    if !worktree.is_dir() {
        return Ok(None);
    }
    let base = project.default_branch;
    tokio::task::spawn_blocking(move || GitService::worktree_changes(&worktree, &base))
        .await
        .map_err(|e| e.to_string())?
        .map(Some)
        .map_err(|e| format!("Could not read the changes: {e}"))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TransitionRow {
    pub from_status: String,
    pub to_status: String,
    pub reason: Option<String>,
    pub actor: String,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub provider: String,
    pub status: String,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub started_at: chrono::DateTime<Utc>,
    pub ended_at: Option<chrono::DateTime<Utc>>,
    pub exit_code: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RunRow {
    pub id: String,
    pub verification_type: String,
    pub command: String,
    pub status: String,
    pub exit_code: Option<i64>,
    pub result_summary: Option<String>,
    pub started_at: chrono::DateTime<Utc>,
    pub ended_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct TaskDetail {
    pub events: Vec<crate::events::NormalizedEvent>,
    pub transitions: Vec<TransitionRow>,
    pub sessions: Vec<SessionRow>,
    pub runs: Vec<RunRow>,
}

/// History of one task: status changes, agent sessions and check runs, newest first.
#[tauri::command]
pub async fn get_task_detail(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<TaskDetail, String> {
    let transitions = sqlx::query_as(
        "SELECT from_status, to_status, reason, actor, created_at FROM task_transitions
         WHERE task_id = $1 ORDER BY created_at DESC, rowid DESC LIMIT 200",
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let sessions = sqlx::query_as(
        "SELECT id, provider, status, worktree_path, branch_name, started_at, ended_at, exit_code
         FROM agent_sessions WHERE task_id = $1 ORDER BY started_at DESC, rowid DESC LIMIT 50",
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let runs = sqlx::query_as(
        "SELECT id, verification_type, command, status, exit_code, result_summary, started_at, ended_at
         FROM verification_runs WHERE task_id = $1 ORDER BY started_at DESC, rowid DESC LIMIT 60",
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;
    let events = crate::events::store::task_events(&state.db, &task_id, 300)
        .await
        .map_err(|e| e.to_string())?;
    Ok(TaskDetail {
        events,
        transitions,
        sessions,
        runs,
    })
}

/// Boss HP is computed here from tasks and verification runs; the frontend
/// only displays it.
#[tauri::command]
pub async fn get_boss_hp(quest_id: String, state: State<'_, AppState>) -> Result<BossHp, String> {
    let project: Project = sqlx::query_as(
        "SELECT p.* FROM projects p JOIN quests q ON q.project_id = p.id WHERE q.id = $1",
    )
    .bind(&quest_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Quest not found: {e}"))?;

    let (total, done): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(status = 'completed'), 0) FROM tasks WHERE quest_id = $1",
    )
    .bind(&quest_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let mut conn = state.db.acquire().await.map_err(|e| e.to_string())?;
    let latest = LatestVerification {
        build: verification::latest_check_input(
            &mut conn,
            &quest_id,
            "build",
            project.build_cmd.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?,
        test: verification::latest_check_input(
            &mut conn,
            &quest_id,
            "test",
            project.test_cmd.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?,
        lint: verification::latest_check_input(
            &mut conn,
            &quest_id,
            "lint",
            project.lint_cmd.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?,
    };

    Ok(compute_boss_hp(
        u32::try_from(done).unwrap_or(0),
        u32::try_from(total).unwrap_or(0),
        &latest,
    ))
}

/// Saves the verification commands for a project. Empty text clears a check.
#[tauri::command]
pub async fn set_project_checks(
    project_id: String,
    build_cmd: Option<String>,
    test_cmd: Option<String>,
    lint_cmd: Option<String>,
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    sqlx::query(
        "UPDATE projects SET build_cmd = $1, test_cmd = $2, lint_cmd = $3,
         updated_at = CURRENT_TIMESTAMP WHERE id = $4",
    )
    .bind(clean(build_cmd))
    .bind(clean(test_cmd))
    .bind(clean(lint_cmd))
    .bind(&project_id)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(&project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct CheckReport {
    pub verification_type: String,
    pub command: String,
    pub passed: bool,
    pub exit_code: Option<i32>,
    pub summary: String,
}

/// Runs the configured checks for a task's quest inside the quest worktree
/// (never the user's main working tree) and records each run.
#[tauri::command]
pub async fn run_checks(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<CheckReport>, String> {
    let task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Task not found: {e}"))?;
    let project: Project = sqlx::query_as(
        "SELECT p.* FROM projects p JOIN quests q ON q.project_id = p.id WHERE q.id = $1",
    )
    .bind(&task.quest_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Project not found: {e}"))?;

    let worktree =
        GitService::quest_worktree_path(Path::new(&project.repository_path), &task.quest_id);
    if !worktree.is_dir() {
        return Err(
            "The quest worktree does not exist yet. Start a task first; checks never run in your main working tree."
                .to_string(),
        );
    }

    let checks = [
        ("build", project.build_cmd.clone()),
        ("test", project.test_cmd.clone()),
        ("lint", project.lint_cmd.clone()),
    ];
    let mut reports = Vec::new();

    for (kind, command) in checks {
        let Some(command) = command.filter(|c| !c.trim().is_empty()) else {
            continue;
        };
        let run_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO verification_runs (id, task_id, verification_type, command, status)
             VALUES ($1, $2, $3, $4, 'running')",
        )
        .bind(&run_id)
        .bind(&task_id)
        .bind(kind)
        .bind(&command)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        // A spawn failure is recorded as a failed run with its reason.
        let outcome =
            match verification::run_check(&command, &worktree, verification::DEFAULT_TIMEOUT).await
            {
                Ok(o) => o,
                Err(e) => verification::CheckOutcome {
                    passed: false,
                    exit_code: None,
                    summary: e.to_string(),
                },
            };

        sqlx::query(
            "UPDATE verification_runs SET status = $1, exit_code = $2, result_summary = $3,
             ended_at = CURRENT_TIMESTAMP WHERE id = $4",
        )
        .bind(if outcome.passed { "passed" } else { "failed" })
        .bind(outcome.exit_code)
        .bind(&outcome.summary)
        .bind(&run_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        reports.push(CheckReport {
            verification_type: kind.to_string(),
            command,
            passed: outcome.passed,
            exit_code: outcome.exit_code,
            summary: outcome.summary,
        });
    }

    if reports.is_empty() {
        return Err(
            "No checks set. Add build, test or lint commands so this task can be verified."
                .to_string(),
        );
    }

    // The outcome decides the task state; Completed needs the passing runs
    // just recorded as evidence and is re-checked inside the transition.
    let all_passed = reports.iter().all(|r| r.passed);
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
    if all_passed {
        transition_task(
            &mut tx,
            &task_id,
            TaskStatus::Completed,
            &TransitionContext::new("verification").evidence(Evidence::VerificationRuns),
        )
        .await
        .map_err(|e| e.to_string())?;
    } else {
        let failed: Vec<&str> = reports
            .iter()
            .filter(|r| !r.passed)
            .map(|r| r.verification_type.as_str())
            .collect();
        transition_task(
            &mut tx,
            &task_id,
            TaskStatus::Failed,
            &TransitionContext::new("verification")
                .reason(format!("Checks failed: {}", failed.join(", "))),
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(reports)
}

/// The user reviewed a task that needs review and confirms it is done.
#[tauri::command]
pub async fn mark_task_verified(task_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;
    let current: (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if TaskStatus::parse(&current.0) != Some(TaskStatus::NeedsReview) {
        return Err("Only a task that needs review can be marked verified.".to_string());
    }
    transition_task(
        &mut tx,
        &task_id,
        TaskStatus::Completed,
        &TransitionContext::new("user")
            .reason("Reviewed by the user")
            .evidence(Evidence::UserReview),
    )
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Looks for build/test/lint commands in the quest worktree (where the agent's
/// files are) or, without a quest, in the repository root.
#[tauri::command]
pub async fn detect_checks(
    project_id: String,
    quest_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<verification::DetectedChecks, String> {
    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(&project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let repo = std::path::PathBuf::from(&project.repository_path);
    let dir = match quest_id {
        Some(q) => {
            let wt = GitService::quest_worktree_path(&repo, &q);
            if wt.is_dir() { wt } else { repo }
        }
        None => repo,
    };
    tokio::task::spawn_blocking(move || verification::detect_checks(&dir))
        .await
        .map_err(|e| e.to_string())
}

/// Recent activity of a project, oldest first, so the log survives a restart.
#[tauri::command]
pub async fn get_recent_events(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::events::NormalizedEvent>, String> {
    crate::events::store::project_events(&state.db, &project_id, 500)
        .await
        .map_err(|e| e.to_string())
}
