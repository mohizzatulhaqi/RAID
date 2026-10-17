use crate::git::GitService;
use crate::models::{Character, Project, Quest, Task};
use crate::providers::{AgentProvider, ClaudeCodeAdapter, SessionConfig};
use chrono::Utc;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

pub struct AppState {
    pub db: SqlitePool,
    pub claude_adapter: Arc<ClaudeCodeAdapter>,
}

#[tauri::command]
pub async fn open_project(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let path = Path::new(&repo_path);
    if !path.exists() {
        return Err("Repository path does not exist".to_string());
    }

    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Project".to_string());

    let existing: Option<Project> = sqlx::query_as(
        "SELECT * FROM projects WHERE repository_path = $1"
    )
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
            default_branch: "main".to_string(),
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

#[tauri::command]
pub async fn get_tasks(
    quest_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, String> {
    sqlx::query_as("SELECT * FROM tasks WHERE quest_id = $1 ORDER BY created_at ASC")
        .bind(quest_id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())
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
pub async fn get_characters(
    state: State<'_, AppState>,
) -> Result<Vec<Character>, String> {
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

#[tauri::command]
pub async fn start_quest_task(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // 1. Fetch task and quest details
    let task: Task = sqlx::query_as("SELECT * FROM tasks WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Task not found: {}", e))?;

    let quest: Quest = sqlx::query_as("SELECT * FROM quests WHERE id = $1")
        .bind(&task.quest_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Quest not found: {}", e))?;

    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(&quest.project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("Project not found: {}", e))?;

    // 2. Create isolated Git worktree for this quest
    let worktree_dir = GitService::create_quest_worktree(
        Path::new(&project.repository_path),
        &quest.id,
    )
    .map_err(|e| format!("Failed to create git worktree: {}", e))?;

    let session_id = Uuid::new_v4().to_string();

    // 3. Record session in SQLite
    let _ = sqlx::query(
        "INSERT INTO agent_sessions (id, task_id, provider, model, worktree_path, branch_name, status)
         VALUES ($1, $2, 'claude-code', 'claude-3-7-sonnet', $3, $4, 'running')"
    )
    .bind(&session_id)
    .bind(&task.id)
    .bind(worktree_dir.to_string_lossy().to_string())
    .bind(format!("raid/quest-{}", quest.id))
    .execute(&state.db)
    .await;

    // 4. Update task status to 'running'
    let _ = sqlx::query("UPDATE tasks SET status = 'running', updated_at = CURRENT_TIMESTAMP WHERE id = $1")
        .bind(&task.id)
        .execute(&state.db)
        .await;

    // 5. Start Claude Code session in the worktree
    let config = SessionConfig {
        session_id: session_id.clone(),
        task_id: task.id,
        quest_id: quest.id,
        task_title: task.title,
        task_description: task.description,
        worktree_path: worktree_dir,
        model: "claude-3-7-sonnet".to_string(),
    };

    state
        .claude_adapter
        .start_session(config)
        .await
        .map_err(|e| format!("Failed to start agent session: {}", e))?;

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
        .map_err(|e| e.to_string())
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

#[tauri::command]
pub async fn get_quest_diff(
    project_id: String,
    quest_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let project: Project = sqlx::query_as("SELECT * FROM projects WHERE id = $1")
        .bind(&project_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    GitService::get_quest_diff(Path::new(&project.repository_path), &quest_id)
        .map_err(|e| e.to_string())
}
