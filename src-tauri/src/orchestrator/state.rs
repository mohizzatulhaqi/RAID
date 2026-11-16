use crate::verification;
use serde::Serialize;
use sqlx::SqliteConnection;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

/// Maximum sessions in `Running`/`AwaitingApproval` at once (PRD §18.3).
pub const MAX_ACTIVE_SESSIONS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Ready,
    Assigned,
    Running,
    AwaitingApproval,
    Verification,
    Completed,
    Blocked,
    Failed,
    Cancelled,
    NeedsReview,
}

impl TaskStatus {
    pub const ALL: [TaskStatus; 11] = [
        TaskStatus::Created,
        TaskStatus::Ready,
        TaskStatus::Assigned,
        TaskStatus::Running,
        TaskStatus::AwaitingApproval,
        TaskStatus::Verification,
        TaskStatus::Completed,
        TaskStatus::Blocked,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
        TaskStatus::NeedsReview,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Created => "created",
            TaskStatus::Ready => "ready",
            TaskStatus::Assigned => "assigned",
            TaskStatus::Running => "running",
            TaskStatus::AwaitingApproval => "awaiting_approval",
            TaskStatus::Verification => "verification",
            TaskStatus::Completed => "completed",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
            TaskStatus::NeedsReview => "needs_review",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|st| st.as_str() == s)
    }

    /// Reference table from the skill, extended for restart recovery:
    /// `Assigned -> Failed` and `Assigned/Running/AwaitingApproval -> NeedsReview`.
    pub fn can_transition(self, to: TaskStatus) -> bool {
        use TaskStatus::*;
        matches!(
            (self, to),
            (Created, Ready | Blocked | Cancelled)
                | (Blocked, Ready | Cancelled)
                | (Ready, Assigned | Cancelled)
                | (Assigned, Running | Ready | Cancelled | Failed | NeedsReview)
                | (
                    Running,
                    AwaitingApproval | Verification | Failed | Cancelled | NeedsReview
                )
                | (AwaitingApproval, Running | Cancelled | Failed | NeedsReview)
                | (Verification, Completed | Failed | NeedsReview)
                | (NeedsReview, Completed | Failed | Ready)
                | (Failed, Ready | Cancelled)
                | (Cancelled, Ready)
                | (Completed, Ready)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Preparing,
    Running,
    Waiting,
    WaitingApproval,
    Succeeded,
    Failed,
    Cancelled,
}

impl SessionStatus {
    pub const ALL: [SessionStatus; 7] = [
        SessionStatus::Preparing,
        SessionStatus::Running,
        SessionStatus::Waiting,
        SessionStatus::WaitingApproval,
        SessionStatus::Succeeded,
        SessionStatus::Failed,
        SessionStatus::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SessionStatus::Preparing => "preparing",
            SessionStatus::Running => "running",
            SessionStatus::Waiting => "waiting",
            SessionStatus::WaitingApproval => "waiting_approval",
            SessionStatus::Succeeded => "succeeded",
            SessionStatus::Failed => "failed",
            SessionStatus::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|st| st.as_str() == s)
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            SessionStatus::Succeeded | SessionStatus::Failed | SessionStatus::Cancelled
        )
    }

    pub fn can_transition(self, to: SessionStatus) -> bool {
        use SessionStatus::*;
        matches!(
            (self, to),
            (Preparing, Running | Failed | Cancelled)
                | (
                    Running,
                    Waiting | WaitingApproval | Succeeded | Failed | Cancelled
                )
                | (Waiting, Running | Succeeded | Failed | Cancelled)
                // The process can end on its own while an approval is pending.
                | (WaitingApproval, Running | Succeeded | Failed | Cancelled)
        )
    }
}

/// What justifies a transition. `Completed` requires evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    None,
    /// Every configured check's latest run passed with exit code 0.
    VerificationRuns,
    /// The user reviewed the work and confirmed it.
    UserReview,
}

#[derive(Debug, Clone)]
pub struct TransitionContext {
    pub actor: &'static str,
    pub reason: Option<String>,
    pub evidence: Evidence,
}

impl TransitionContext {
    pub fn new(actor: &'static str) -> Self {
        Self {
            actor,
            reason: None,
            evidence: Evidence::None,
        }
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn evidence(mut self, evidence: Evidence) -> Self {
        self.evidence = evidence;
        self
    }
}

#[derive(Error, Debug)]
pub enum TransitionError {
    #[error("Task not found")]
    NotFound,
    #[error("Unknown status value in the database: {0}")]
    UnknownStatus(String),
    #[error("A task cannot move from {from} to {to}")]
    Invalid {
        from: &'static str,
        to: &'static str,
    },
    #[error("The task changed while this action was running. Reload and try again.")]
    Conflict,
    #[error("Completing a task needs passing checks or a user review")]
    MissingEvidence,
    #[error("Waiting for other tasks to be verified first")]
    DependenciesIncomplete,
    #[error("Another session is already running (limit {0})")]
    ConcurrencyLimit(i64),
    #[error("This party member is already on another task")]
    CharacterBusy,
    #[error("Party member not found")]
    CharacterNotFound,
    #[error("This dependency would create a cycle")]
    DependencyCycle,
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

async fn dependencies_complete(
    conn: &mut SqliteConnection,
    task_id: &str,
) -> Result<bool, sqlx::Error> {
    let (open,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_dependencies d
         JOIN tasks t ON t.id = d.depends_on_task_id
         WHERE d.task_id = $1 AND t.status != 'completed'",
    )
    .bind(task_id)
    .fetch_one(conn)
    .await?;
    Ok(open == 0)
}

async fn checks_passed_for_task(
    conn: &mut SqliteConnection,
    task_id: &str,
) -> Result<bool, sqlx::Error> {
    type Row = (String, Option<String>, Option<String>, Option<String>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT q.id, p.build_cmd, p.test_cmd, p.lint_cmd
         FROM tasks t
         JOIN quests q ON q.id = t.quest_id
         JOIN projects p ON p.id = q.project_id
         WHERE t.id = $1",
    )
    .bind(task_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((quest_id, build, test, lint)) = row else {
        return Ok(false);
    };
    verification::all_configured_checks_passed(
        conn,
        &quest_id,
        build.as_deref(),
        test.as_deref(),
        lint.as_deref(),
    )
    .await
}

/// Moves a task to `to` if the transition is valid and its guards hold.
/// Returns the previous status. Run it inside the caller's transaction
/// together with any related writes.
pub async fn transition_task(
    conn: &mut SqliteConnection,
    task_id: &str,
    to: TaskStatus,
    ctx: &TransitionContext,
) -> Result<TaskStatus, TransitionError> {
    let current: Option<(String,)> = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_optional(&mut *conn)
        .await?;
    let (raw,) = current.ok_or(TransitionError::NotFound)?;
    let from = TaskStatus::parse(&raw).ok_or(TransitionError::UnknownStatus(raw))?;

    if !from.can_transition(to) {
        return Err(TransitionError::Invalid {
            from: from.as_str(),
            to: to.as_str(),
        });
    }

    match to {
        TaskStatus::Ready if !dependencies_complete(&mut *conn, task_id).await? => {
            return Err(TransitionError::DependenciesIncomplete);
        }
        TaskStatus::Running if from == TaskStatus::Assigned => {
            let (active,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM tasks
                 WHERE status IN ('running', 'awaiting_approval') AND id != $1",
            )
            .bind(task_id)
            .fetch_one(&mut *conn)
            .await?;
            if active >= MAX_ACTIVE_SESSIONS {
                return Err(TransitionError::ConcurrencyLimit(MAX_ACTIVE_SESSIONS));
            }
        }
        TaskStatus::Completed => {
            let proven = match ctx.evidence {
                Evidence::None => false,
                Evidence::UserReview => true,
                Evidence::VerificationRuns => checks_passed_for_task(&mut *conn, task_id).await?,
            };
            if !proven {
                return Err(TransitionError::MissingEvidence);
            }
        }
        _ => {}
    }

    let keep_reason = matches!(to, TaskStatus::Failed | TaskStatus::NeedsReview);
    let result = sqlx::query(
        "UPDATE tasks SET status = $1, failure_reason = $2, updated_at = CURRENT_TIMESTAMP
         WHERE id = $3 AND status = $4",
    )
    .bind(to.as_str())
    .bind(if keep_reason {
        ctx.reason.as_deref()
    } else {
        None
    })
    .bind(task_id)
    .bind(from.as_str())
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(TransitionError::Conflict);
    }

    sqlx::query(
        "INSERT INTO task_transitions (id, task_id, from_status, to_status, reason, actor)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(task_id)
    .bind(from.as_str())
    .bind(to.as_str())
    .bind(ctx.reason.as_deref())
    .bind(ctx.actor)
    .execute(&mut *conn)
    .await?;

    Ok(from)
}

/// Same contract as `transition_task`, for `agent_sessions`.
pub async fn transition_session(
    conn: &mut SqliteConnection,
    session_id: &str,
    to: SessionStatus,
    exit_code: Option<i32>,
) -> Result<SessionStatus, TransitionError> {
    let current: Option<(String,)> =
        sqlx::query_as("SELECT status FROM agent_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (raw,) = current.ok_or(TransitionError::NotFound)?;
    let from = SessionStatus::parse(&raw).ok_or(TransitionError::UnknownStatus(raw))?;

    if !from.can_transition(to) {
        return Err(TransitionError::Invalid {
            from: from.as_str(),
            to: to.as_str(),
        });
    }

    let result = sqlx::query(
        "UPDATE agent_sessions
         SET status = $1,
             ended_at = CASE WHEN $2 THEN CURRENT_TIMESTAMP ELSE ended_at END,
             exit_code = COALESCE($3, exit_code)
         WHERE id = $4 AND status = $5",
    )
    .bind(to.as_str())
    .bind(to.is_terminal())
    .bind(exit_code)
    .bind(session_id)
    .bind(from.as_str())
    .execute(&mut *conn)
    .await?;
    if result.rows_affected() == 0 {
        return Err(TransitionError::Conflict);
    }
    Ok(from)
}

/// Actions the UI may offer for a task, derived only from backend state.
/// Names are stable identifiers: start, retry, cancel, run_checks, mark_verified.
pub fn allowed_actions(status: TaskStatus, has_live_session: bool) -> Vec<&'static str> {
    match status {
        TaskStatus::Created | TaskStatus::Ready => vec!["start"],
        TaskStatus::Failed | TaskStatus::Cancelled => vec!["retry"],
        TaskStatus::Assigned | TaskStatus::Running | TaskStatus::AwaitingApproval => {
            if has_live_session {
                vec!["cancel"]
            } else {
                vec![]
            }
        }
        TaskStatus::Verification => vec!["run_checks"],
        TaskStatus::NeedsReview => vec!["run_checks", "mark_verified", "retry"],
        TaskStatus::Completed | TaskStatus::Blocked => vec![],
    }
}

/// True if making `task` depend on `depends_on` would close a cycle.
/// `edges` are `(task, depends_on)` pairs already stored.
pub fn would_create_cycle(edges: &[(String, String)], task: &str, depends_on: &str) -> bool {
    if task == depends_on {
        return true;
    }
    let mut next: HashMap<&str, Vec<&str>> = HashMap::new();
    for (t, d) in edges {
        next.entry(t.as_str()).or_default().push(d.as_str());
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stack = vec![depends_on];
    while let Some(node) = stack.pop() {
        if node == task {
            return true;
        }
        if seen.insert(node)
            && let Some(deps) = next.get(node)
        {
            stack.extend(deps.iter().copied());
        }
    }
    false
}

/// Records that `task_id` must wait for `depends_on`, rejecting cycles.
pub async fn add_dependency(
    conn: &mut SqliteConnection,
    task_id: &str,
    depends_on: &str,
) -> Result<(), TransitionError> {
    let edges: Vec<(String, String)> =
        sqlx::query_as("SELECT task_id, depends_on_task_id FROM task_dependencies")
            .fetch_all(&mut *conn)
            .await?;
    if would_create_cycle(&edges, task_id, depends_on) {
        return Err(TransitionError::DependencyCycle);
    }
    sqlx::query(
        "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_task_id) VALUES ($1, $2)",
    )
    .bind(task_id)
    .bind(depends_on)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use TaskStatus::*;
    use sqlx::SqlitePool;
    use sqlx::sqlite::SqlitePoolOptions;

    pub async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect(&format!(
                "sqlite:file:{}?mode=memory&cache=shared",
                Uuid::new_v4().simple()
            ))
            .await
            .expect("memory db");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        sqlx::query(
            "INSERT INTO projects (id, name, repository_path, build_cmd, test_cmd)
             VALUES ('p', 'p', '/tmp/p', 'make build', 'make test')",
        )
        .execute(&pool)
        .await
        .expect("project");
        sqlx::query(
            "INSERT INTO quests (id, project_id, title, description) VALUES ('q', 'p', 't', 'd')",
        )
        .execute(&pool)
        .await
        .expect("quest");
        pool
    }

    pub async fn seed_task(pool: &SqlitePool, id: &str, status: TaskStatus) {
        sqlx::query(
            "INSERT INTO tasks (id, quest_id, title, description, status)
             VALUES ($1, 'q', $1, 'd', $2)",
        )
        .bind(id)
        .bind(status.as_str())
        .execute(pool)
        .await
        .expect("task");
    }

    async fn seed_run(pool: &SqlitePool, task: &str, kind: &str, status: &str, code: i32) {
        sqlx::query(
            "INSERT INTO verification_runs (id, task_id, verification_type, command, status, exit_code)
             VALUES ($1, $2, $3, 'cmd', $4, $5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(task)
        .bind(kind)
        .bind(status)
        .bind(code)
        .execute(pool)
        .await
        .expect("run");
    }

    async fn status_of(pool: &SqlitePool, id: &str) -> String {
        let (s,): (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("status");
        s
    }

    fn ctx() -> TransitionContext {
        TransitionContext::new("test")
    }

    #[test]
    fn task_transition_table_matches_the_reference() {
        let allowed: &[(TaskStatus, TaskStatus)] = &[
            (Created, Ready),
            (Created, Blocked),
            (Created, Cancelled),
            (Blocked, Ready),
            (Blocked, Cancelled),
            (Ready, Assigned),
            (Ready, Cancelled),
            (Assigned, Running),
            (Assigned, Ready),
            (Assigned, Cancelled),
            (Assigned, Failed),
            (Assigned, NeedsReview),
            (Running, AwaitingApproval),
            (Running, Verification),
            (Running, Failed),
            (Running, Cancelled),
            (Running, NeedsReview),
            (AwaitingApproval, Running),
            (AwaitingApproval, Cancelled),
            (AwaitingApproval, Failed),
            (AwaitingApproval, NeedsReview),
            (Verification, Completed),
            (Verification, Failed),
            (Verification, NeedsReview),
            (NeedsReview, Completed),
            (NeedsReview, Failed),
            (NeedsReview, Ready),
            (Failed, Ready),
            (Failed, Cancelled),
            (Cancelled, Ready),
            (Completed, Ready),
        ];
        for from in TaskStatus::ALL {
            for to in TaskStatus::ALL {
                assert_eq!(
                    from.can_transition(to),
                    allowed.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn only_verification_and_review_lead_to_completed() {
        for from in TaskStatus::ALL {
            let expected = matches!(from, Verification | NeedsReview);
            assert_eq!(from.can_transition(Completed), expected, "{from:?}");
        }
    }

    #[test]
    fn session_terminal_states_have_no_exits() {
        for from in SessionStatus::ALL.into_iter().filter(|s| s.is_terminal()) {
            for to in SessionStatus::ALL {
                assert!(!from.can_transition(to), "{from:?} -> {to:?}");
            }
        }
        assert!(SessionStatus::Running.can_transition(SessionStatus::Succeeded));
        assert!(!SessionStatus::Preparing.can_transition(SessionStatus::Succeeded));
        assert!(SessionStatus::WaitingApproval.can_transition(SessionStatus::Succeeded));
    }

    #[test]
    fn actions_follow_the_state() {
        assert_eq!(allowed_actions(Ready, false), vec!["start"]);
        assert_eq!(allowed_actions(Failed, false), vec!["retry"]);
        assert_eq!(allowed_actions(Running, true), vec!["cancel"]);
        assert!(allowed_actions(Running, false).is_empty());
        assert_eq!(allowed_actions(Verification, false), vec!["run_checks"]);
        assert!(allowed_actions(Completed, false).is_empty());
        // Marking verified is only offered after review is needed, never from Running.
        for st in TaskStatus::ALL {
            assert_eq!(
                allowed_actions(st, true).contains(&"mark_verified"),
                st == NeedsReview
            );
        }
    }

    #[test]
    fn status_strings_round_trip() {
        for s in TaskStatus::ALL {
            assert_eq!(TaskStatus::parse(s.as_str()), Some(s));
        }
        for s in SessionStatus::ALL {
            assert_eq!(SessionStatus::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn cycle_detection() {
        let edges = vec![
            ("b".to_string(), "a".to_string()),
            ("c".to_string(), "b".to_string()),
        ];
        assert!(would_create_cycle(&edges, "x", "x"));
        assert!(would_create_cycle(&edges, "a", "c"));
        assert!(would_create_cycle(&edges, "a", "b"));
        assert!(!would_create_cycle(&edges, "c", "a"));
        assert!(!would_create_cycle(&edges, "d", "c"));
    }

    #[tokio::test]
    async fn running_cannot_jump_to_completed() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Running).await;
        let mut conn = pool.acquire().await.expect("conn");
        let err = transition_task(
            &mut conn,
            "t",
            Completed,
            &ctx().evidence(Evidence::UserReview),
        )
        .await;
        assert!(matches!(err, Err(TransitionError::Invalid { .. })));
        assert_eq!(status_of(&pool, "t").await, "running");
    }

    #[tokio::test]
    async fn completing_without_evidence_is_rejected() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Verification).await;
        let mut conn = pool.acquire().await.expect("conn");
        let err = transition_task(&mut conn, "t", Completed, &ctx()).await;
        assert!(matches!(err, Err(TransitionError::MissingEvidence)));
        let err = transition_task(
            &mut conn,
            "t",
            Completed,
            &ctx().evidence(Evidence::VerificationRuns),
        )
        .await;
        assert!(matches!(err, Err(TransitionError::MissingEvidence)));
        assert_eq!(status_of(&pool, "t").await, "verification");
    }

    #[tokio::test]
    async fn passing_runs_allow_completion_and_a_failure_blocks_it() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Verification).await;
        seed_run(&pool, "t", "build", "passed", 0).await;
        seed_run(&pool, "t", "test", "failed", 1).await;
        let mut conn = pool.acquire().await.expect("conn");
        let evidence = ctx().evidence(Evidence::VerificationRuns);
        assert!(matches!(
            transition_task(&mut conn, "t", Completed, &evidence).await,
            Err(TransitionError::MissingEvidence)
        ));

        seed_run(&pool, "t", "test", "passed", 0).await;
        transition_task(&mut conn, "t", Completed, &evidence)
            .await
            .expect("completed");
        assert_eq!(status_of(&pool, "t").await, "completed");
    }

    #[tokio::test]
    async fn passed_status_with_nonzero_exit_code_is_not_evidence() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Verification).await;
        seed_run(&pool, "t", "build", "passed", 0).await;
        seed_run(&pool, "t", "test", "passed", 2).await;
        let mut conn = pool.acquire().await.expect("conn");
        let err = transition_task(
            &mut conn,
            "t",
            Completed,
            &ctx().evidence(Evidence::VerificationRuns),
        )
        .await;
        assert!(matches!(err, Err(TransitionError::MissingEvidence)));
    }

    #[tokio::test]
    async fn user_review_allows_completion_from_needs_review() {
        let pool = test_pool().await;
        seed_task(&pool, "t", NeedsReview).await;
        let mut conn = pool.acquire().await.expect("conn");
        transition_task(
            &mut conn,
            "t",
            Completed,
            &ctx().evidence(Evidence::UserReview),
        )
        .await
        .expect("completed");
        assert_eq!(status_of(&pool, "t").await, "completed");
    }

    #[tokio::test]
    async fn task_with_open_prerequisite_cannot_become_ready() {
        let pool = test_pool().await;
        seed_task(&pool, "dep", Running).await;
        seed_task(&pool, "t", Created).await;
        let mut conn = pool.acquire().await.expect("conn");
        add_dependency(&mut conn, "t", "dep")
            .await
            .expect("dependency");
        let err = transition_task(&mut conn, "t", Ready, &ctx()).await;
        assert!(matches!(err, Err(TransitionError::DependenciesIncomplete)));

        sqlx::query("UPDATE tasks SET status = 'completed' WHERE id = 'dep'")
            .execute(&pool)
            .await
            .expect("complete dep");
        transition_task(&mut conn, "t", Ready, &ctx())
            .await
            .expect("ready");
    }

    #[tokio::test]
    async fn dependency_cycles_are_rejected() {
        let pool = test_pool().await;
        for id in ["a", "b", "c"] {
            seed_task(&pool, id, Created).await;
        }
        let mut conn = pool.acquire().await.expect("conn");
        add_dependency(&mut conn, "b", "a").await.expect("b->a");
        add_dependency(&mut conn, "c", "b").await.expect("c->b");
        assert!(matches!(
            add_dependency(&mut conn, "a", "c").await,
            Err(TransitionError::DependencyCycle)
        ));
    }

    #[tokio::test]
    async fn only_one_session_may_run_at_a_time() {
        let pool = test_pool().await;
        seed_task(&pool, "t1", Assigned).await;
        seed_task(&pool, "t2", Assigned).await;
        let mut conn = pool.acquire().await.expect("conn");
        transition_task(&mut conn, "t1", Running, &ctx())
            .await
            .expect("first");
        let err = transition_task(&mut conn, "t2", Running, &ctx()).await;
        assert!(matches!(err, Err(TransitionError::ConcurrencyLimit(_))));
        assert_eq!(status_of(&pool, "t2").await, "assigned");
    }

    #[tokio::test]
    async fn repeating_a_transition_is_rejected() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Ready).await;
        let mut conn = pool.acquire().await.expect("conn");
        transition_task(&mut conn, "t", Assigned, &ctx())
            .await
            .expect("first");
        let again = transition_task(&mut conn, "t", Assigned, &ctx()).await;
        assert!(matches!(again, Err(TransitionError::Invalid { .. })));
    }

    #[tokio::test]
    async fn transitions_are_audited_and_failures_keep_their_reason() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Running).await;
        let mut conn = pool.acquire().await.expect("conn");
        transition_task(
            &mut conn,
            "t",
            Failed,
            &ctx().reason("agent exited with an error"),
        )
        .await
        .expect("failed");
        let (reason,): (Option<String>,) =
            sqlx::query_as("SELECT failure_reason FROM tasks WHERE id = 't'")
                .fetch_one(&pool)
                .await
                .expect("reason");
        assert_eq!(reason.as_deref(), Some("agent exited with an error"));
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_transitions
             WHERE task_id = 't' AND from_status = 'running' AND to_status = 'failed'",
        )
        .fetch_one(&pool)
        .await
        .expect("audit");
        assert_eq!(n, 1);

        transition_task(&mut conn, "t", Ready, &ctx())
            .await
            .expect("retry");
        let (reason,): (Option<String>,) =
            sqlx::query_as("SELECT failure_reason FROM tasks WHERE id = 't'")
                .fetch_one(&pool)
                .await
                .expect("reason");
        assert_eq!(reason, None);
    }

    #[tokio::test]
    async fn session_transitions_set_end_time_and_block_a_second_live_session() {
        let pool = test_pool().await;
        seed_task(&pool, "t", Running).await;
        sqlx::query(
            "INSERT INTO agent_sessions (id, task_id, status) VALUES ('s1', 't', 'running')",
        )
        .execute(&pool)
        .await
        .expect("session");
        let second = sqlx::query(
            "INSERT INTO agent_sessions (id, task_id, status) VALUES ('s2', 't', 'preparing')",
        )
        .execute(&pool)
        .await;
        assert!(
            second.is_err(),
            "unique index should block a second live session"
        );

        let mut conn = pool.acquire().await.expect("conn");
        transition_session(&mut conn, "s1", SessionStatus::Succeeded, Some(0))
            .await
            .expect("succeeded");
        let (ended, code): (Option<String>, Option<i64>) =
            sqlx::query_as("SELECT ended_at, exit_code FROM agent_sessions WHERE id = 's1'")
                .fetch_one(&pool)
                .await
                .expect("row");
        assert!(ended.is_some());
        assert_eq!(code, Some(0));
        assert!(matches!(
            transition_session(&mut conn, "s1", SessionStatus::Running, None).await,
            Err(TransitionError::Invalid { .. })
        ));
    }
}
