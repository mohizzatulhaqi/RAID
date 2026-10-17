use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Execution error: {0}")]
    ExecutionError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub task_id: String,
    pub quest_id: String,
    pub task_title: String,
    pub task_description: String,
    pub worktree_path: PathBuf,
    pub model: String,
}

#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Launches an agent session in the specified repository/worktree directory
    async fn start_session(&self, config: SessionConfig) -> Result<(), ProviderError>;

    /// Sends user input or confirmation response (e.g., 'y', 'n', text) to the session's stdin
    async fn send_input(&self, session_id: &str, input: &str) -> Result<(), ProviderError>;

    /// Aborts / cancels a running session
    async fn cancel(&self, session_id: &str) -> Result<(), ProviderError>;
}
