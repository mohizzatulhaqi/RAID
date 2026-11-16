use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresentationState {
    Idle,
    Entering,
    Exploring,
    Planning,
    Fighting,
    Investigating,
    NeedsApproval,
    Verification,
    Returning,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum EventPayload {
    SessionStarted {
        session_id: String,
        task_id: String,
    },
    SessionOutput {
        text: String,
    },
    FileRead {
        path: String,
    },
    FileChanged {
        path: String,
    },
    CommandStarted {
        command: String,
    },
    CommandCompleted {
        command: String,
        exit_code: i32,
    },
    ApprovalRequired {
        action_description: String,
        is_risky: bool,
    },
    VerificationStarted {
        verification_type: String,
        command: String,
    },
    VerificationCompleted {
        verification_type: String,
        success: bool,
        exit_code: i32,
        summary: String,
    },
    SessionSucceeded {
        summary: String,
    },
    SessionFailed {
        error: String,
    },
    SessionCancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub presentation_state: PresentationState,
    pub payload: EventPayload,
}
