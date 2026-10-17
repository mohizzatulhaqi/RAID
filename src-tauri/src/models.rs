use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repository_path: String,
    pub default_branch: String,
    pub build_cmd: Option<String>,
    pub test_cmd: Option<String>,
    pub lint_cmd: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Quest {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub status: String, // 'created', 'ready', 'active', 'completed', 'failed'
    pub priority: String, // 'low', 'medium', 'high', 'urgent'
    pub boss_name: Option<String>,
    pub boss_max_hp: i64,
    pub boss_current_hp: i64,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub quest_id: String,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: String,
    pub status: String, // 'created', 'ready', 'assigned', 'running', 'verification', 'completed', 'blocked', 'awaiting_approval', 'failed', 'cancelled'
    pub assigned_character_id: Option<String>,
    pub priority: String,
    pub weight_percent: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskDependency {
    pub task_id: String,
    pub depends_on_task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Character {
    pub id: String,
    pub display_name: String,
    pub class: String, // 'warrior', 'mage', 'rogue', 'cleric', 'ranger', 'paladin', 'engineer', 'bard', 'summoner'
    pub party_role: String, // 'tank', 'dps', 'healer', 'support', 'controller', 'scout', 'utility'
    pub level: i64,
    pub experience: i64,
    pub appearance_config: String, // JSON
    pub state: String, // 'idle', 'assigned', 'preparing', 'running', 'waiting', 'needs_approval', 'succeeded', 'failed', 'cancelled'
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentSession {
    pub id: String,
    pub task_id: String,
    pub provider: String, // 'claude-code'
    pub model: String,
    pub process_identifier: Option<i64>,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub status: String, // 'preparing', 'running', 'waiting_approval', 'succeeded', 'failed', 'cancelled'
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AgentEvent {
    pub id: String,
    pub session_id: String,
    pub event_type: String, // 'session_started', 'file_read', 'file_changed', 'command_started', etc.
    pub timestamp: DateTime<Utc>,
    pub payload: String, // JSON
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct VerificationRun {
    pub id: String,
    pub task_id: String,
    pub verification_type: String, // 'build', 'test', 'lint', 'custom'
    pub command: String,
    pub status: String, // 'pending', 'running', 'passed', 'failed'
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i64>,
    pub result_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WorldState {
    pub project_id: String,
    pub scene_id: String,
    pub camera_state: String, // JSON {"x":0,"y":0,"zoom":1.0}
    pub character_positions: String, // JSON
    pub visual_preferences: String, // JSON
    pub updated_at: DateTime<Utc>,
}
