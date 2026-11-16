use crate::orchestrator::state::{
    SessionStatus, TaskStatus, TransitionContext, TransitionError, transition_session,
    transition_task,
};
use crate::providers::{SessionExit, SessionOutcome};
use crate::repo::GitService;
use sqlx::{SqliteConnection, SqlitePool};
use std::path::Path;
use tracing::{info, warn};
use uuid::Uuid;

const ACTIVE: [TaskStatus; 3] = [
    TaskStatus::Assigned,
    TaskStatus::Running,
    TaskStatus::AwaitingApproval,
];

async fn task_status(
    conn: &mut SqliteConnection,
    task_id: &str,
) -> Result<TaskStatus, TransitionError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_optional(conn)
        .await?;
    let (raw,) = row.ok_or(TransitionError::NotFound)?;
    TaskStatus::parse(&raw).ok_or(TransitionError::UnknownStatus(raw))
}

async fn session_info(
    conn: &mut SqliteConnection,
    session_id: &str,
) -> Result<Option<(String, SessionStatus)>, TransitionError> {
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT task_id, status FROM agent_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(conn)
            .await?;
    Ok(row.and_then(|(task, status)| SessionStatus::parse(&status).map(|s| (task, s))))
}

/// Moves a task to a running session: `-> Ready -> Assigned -> Running`, and
/// records the session as `Running`. All or nothing: if any guard fails (for
/// example the concurrency limit) nothing is written.
pub async fn begin_session(
    pool: &SqlitePool,
    task_id: &str,
    worktree_path: &str,
    branch_name: &str,
    model: &str,
) -> Result<String, TransitionError> {
    let ctx = TransitionContext::new("user");
    let mut tx = pool.begin().await?;

    if task_status(&mut tx, task_id).await? != TaskStatus::Ready {
        transition_task(&mut tx, task_id, TaskStatus::Ready, &ctx).await?;
    }
    transition_task(&mut tx, task_id, TaskStatus::Assigned, &ctx).await?;

    let session_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO agent_sessions (id, task_id, provider, model, worktree_path, branch_name, status)
         VALUES ($1, $2, 'claude-code', $3, $4, $5, 'preparing')",
    )
    .bind(&session_id)
    .bind(task_id)
    .bind(model)
    .bind(worktree_path)
    .bind(branch_name)
    .execute(&mut *tx)
    .await?;

    transition_task(&mut tx, task_id, TaskStatus::Running, &ctx).await?;
    transition_session(&mut tx, &session_id, SessionStatus::Running, None).await?;
    tx.commit().await?;
    Ok(session_id)
}

/// The session could not be launched (worktree or spawn failure).
pub async fn fail_start(
    pool: &SqlitePool,
    task_id: &str,
    session_id: &str,
    reason: &str,
) -> Result<(), TransitionError> {
    let mut tx = pool.begin().await?;
    transition_session(&mut tx, session_id, SessionStatus::Failed, None).await?;
    transition_task(
        &mut tx,
        task_id,
        TaskStatus::Failed,
        &TransitionContext::new("system").reason(reason),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Applies a process exit. Idempotent: an already finished session is ignored.
/// A normal exit only moves the task to `Verification`, never to `Completed`.
pub async fn on_session_exit(pool: &SqlitePool, exit: &SessionExit) -> Result<(), TransitionError> {
    let mut tx = pool.begin().await?;
    let Some((_, status)) = session_info(&mut tx, &exit.session_id).await? else {
        return Ok(());
    };
    if status.is_terminal() {
        return Ok(());
    }

    let (session_to, exit_code) = match exit.outcome {
        SessionOutcome::Succeeded => (SessionStatus::Succeeded, Some(0)),
        SessionOutcome::Failed => (SessionStatus::Failed, None),
        SessionOutcome::Cancelled => (SessionStatus::Cancelled, None),
    };
    transition_session(&mut tx, &exit.session_id, session_to, exit_code).await?;

    let current = task_status(&mut tx, &exit.task_id).await?;
    if ACTIVE.contains(&current) {
        let ctx = TransitionContext::new("system");
        match exit.outcome {
            SessionOutcome::Succeeded => {
                if current == TaskStatus::AwaitingApproval {
                    transition_task(&mut tx, &exit.task_id, TaskStatus::Running, &ctx).await?;
                }
                transition_task(&mut tx, &exit.task_id, TaskStatus::Verification, &ctx).await?;
            }
            SessionOutcome::Failed => {
                transition_task(
                    &mut tx,
                    &exit.task_id,
                    TaskStatus::Failed,
                    &ctx.reason("The agent process exited with an error"),
                )
                .await?;
            }
            SessionOutcome::Cancelled => {
                transition_task(
                    &mut tx,
                    &exit.task_id,
                    TaskStatus::Cancelled,
                    &ctx.reason("Cancelled by the user"),
                )
                .await?;
            }
        }
    } else {
        warn!(
            "Session {} ended but task {} is {}; leaving the task as is",
            exit.session_id,
            exit.task_id,
            current.as_str()
        );
    }
    tx.commit().await?;
    Ok(())
}

/// The agent is waiting for the user: session `WaitingApproval`, task `AwaitingApproval`.
pub async fn on_approval_requested(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), TransitionError> {
    let mut tx = pool.begin().await?;
    let Some((task_id, SessionStatus::Running)) = session_info(&mut tx, session_id).await? else {
        return Ok(());
    };
    transition_session(&mut tx, session_id, SessionStatus::WaitingApproval, None).await?;
    transition_task(
        &mut tx,
        &task_id,
        TaskStatus::AwaitingApproval,
        &TransitionContext::new("agent"),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// The user answered an approval: back to `Running`.
pub async fn on_approval_answered(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<(), TransitionError> {
    let mut tx = pool.begin().await?;
    let Some((task_id, SessionStatus::WaitingApproval)) = session_info(&mut tx, session_id).await?
    else {
        return Ok(());
    };
    transition_session(&mut tx, session_id, SessionStatus::Running, None).await?;
    transition_task(
        &mut tx,
        &task_id,
        TaskStatus::Running,
        &TransitionContext::new("user"),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// At startup no agent process can be alive, so any task still `Assigned`,
/// `Running` or `AwaitingApproval` is orphaned. It becomes `NeedsReview` if
/// its worktree has changes, otherwise `Failed` (reason: interrupted).
/// Returns how many tasks were reconciled.
pub async fn recover_interrupted(pool: &SqlitePool) -> Result<u64, TransitionError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT t.id, q.id, p.repository_path
         FROM tasks t
         JOIN quests q ON q.id = t.quest_id
         JOIN projects p ON p.id = q.project_id
         WHERE t.status IN ('assigned', 'running', 'awaiting_approval')",
    )
    .fetch_all(pool)
    .await?;

    let mut recovered = 0;
    for (task_id, quest_id, repo_path) in rows {
        let worktree = GitService::quest_worktree_path(Path::new(&repo_path), &quest_id);
        let changed = tokio::task::spawn_blocking(move || GitService::has_changes(&worktree))
            .await
            .unwrap_or(true);

        let (to, reason) = if changed {
            (
                TaskStatus::NeedsReview,
                "Interrupted: the app stopped while this task was running. The worktree has changes to review.",
            )
        } else {
            (
                TaskStatus::Failed,
                "Interrupted: the app stopped while this task was running.",
            )
        };

        let mut tx = pool.begin().await?;
        let live: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM agent_sessions
             WHERE task_id = $1 AND status IN ('preparing', 'running', 'waiting', 'waiting_approval')",
        )
        .bind(&task_id)
        .fetch_all(&mut *tx)
        .await?;
        for (session_id,) in live {
            transition_session(&mut tx, &session_id, SessionStatus::Failed, None).await?;
        }
        transition_task(
            &mut tx,
            &task_id,
            to,
            &TransitionContext::new("recovery").reason(reason),
        )
        .await?;
        tx.commit().await?;
        recovered += 1;
    }
    if recovered > 0 {
        info!("Reconciled {recovered} interrupted task(s) after restart");
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::state::tests::{seed_task, test_pool};

    async fn task_row(pool: &SqlitePool, id: &str) -> (String, Option<String>) {
        sqlx::query_as("SELECT status, failure_reason FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("task row")
    }

    async fn session_status(pool: &SqlitePool, id: &str) -> String {
        let (s,): (String,) = sqlx::query_as("SELECT status FROM agent_sessions WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("session");
        s
    }

    fn exit(session: &str, task: &str, outcome: SessionOutcome) -> SessionExit {
        SessionExit {
            session_id: session.to_string(),
            task_id: task.to_string(),
            outcome,
        }
    }

    #[tokio::test]
    async fn begin_session_runs_the_task_and_blocks_a_double_start() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "raid/quest-q", "m")
            .await
            .expect("start");
        assert_eq!(task_row(&pool, "t").await.0, "running");
        assert_eq!(session_status(&pool, &sid).await, "running");

        let again = begin_session(&pool, "t", "/w", "raid/quest-q", "m").await;
        assert!(again.is_err());
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM agent_sessions WHERE task_id = 't'")
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn concurrency_limit_rolls_everything_back() {
        let pool = test_pool().await;
        seed_task(&pool, "t1", TaskStatus::Ready).await;
        seed_task(&pool, "t2", TaskStatus::Ready).await;
        begin_session(&pool, "t1", "/w", "b", "m")
            .await
            .expect("first");

        let second = begin_session(&pool, "t2", "/w", "b", "m").await;
        assert!(matches!(second, Err(TransitionError::ConcurrencyLimit(_))));
        assert_eq!(task_row(&pool, "t2").await.0, "ready");
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM agent_sessions WHERE task_id = 't2'")
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn failed_and_cancelled_tasks_can_be_started_again() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Failed).await;
        begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("retry");
        assert_eq!(task_row(&pool, "t").await.0, "running");
    }

    #[tokio::test]
    async fn normal_exit_moves_to_verification_not_completed() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");
        let e = exit(&sid, "t", SessionOutcome::Succeeded);
        on_session_exit(&pool, &e).await.expect("exit");
        assert_eq!(task_row(&pool, "t").await.0, "verification");
        assert_eq!(session_status(&pool, &sid).await, "succeeded");

        // A repeated notification changes nothing.
        on_session_exit(&pool, &e).await.expect("again");
        assert_eq!(task_row(&pool, "t").await.0, "verification");
    }

    #[tokio::test]
    async fn failed_exit_records_a_reason() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");
        on_session_exit(&pool, &exit(&sid, "t", SessionOutcome::Failed))
            .await
            .expect("exit");
        let (status, reason) = task_row(&pool, "t").await;
        assert_eq!(status, "failed");
        assert!(reason.is_some());
    }

    #[tokio::test]
    async fn cancelled_exit_cancels_the_task() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");
        on_session_exit(&pool, &exit(&sid, "t", SessionOutcome::Cancelled))
            .await
            .expect("exit");
        assert_eq!(task_row(&pool, "t").await.0, "cancelled");
        assert_eq!(session_status(&pool, &sid).await, "cancelled");
    }

    #[tokio::test]
    async fn approval_round_trip() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");
        on_approval_requested(&pool, &sid).await.expect("requested");
        assert_eq!(task_row(&pool, "t").await.0, "awaiting_approval");
        assert_eq!(session_status(&pool, &sid).await, "waiting_approval");

        on_approval_answered(&pool, &sid).await.expect("answered");
        assert_eq!(task_row(&pool, "t").await.0, "running");

        // Answering again, or a request with no live session, is a no-op.
        on_approval_answered(&pool, &sid).await.expect("noop");
        on_approval_requested(&pool, "missing").await.expect("noop");
    }

    #[tokio::test]
    async fn process_ending_while_approval_is_pending_still_reaches_verification() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");
        on_approval_requested(&pool, &sid).await.expect("requested");

        on_session_exit(&pool, &exit(&sid, "t", SessionOutcome::Succeeded))
            .await
            .expect("exit");
        assert_eq!(task_row(&pool, "t").await.0, "verification");
        assert_eq!(session_status(&pool, &sid).await, "succeeded");
    }

    #[tokio::test]
    async fn recovery_fails_orphaned_tasks_without_changes() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m")
            .await
            .expect("start");

        assert_eq!(recover_interrupted(&pool).await.expect("recover"), 1);
        let (status, reason) = task_row(&pool, "t").await;
        assert_eq!(status, "failed");
        assert!(reason.unwrap_or_default().contains("Interrupted"));
        assert_eq!(session_status(&pool, &sid).await, "failed");
        assert_eq!(recover_interrupted(&pool).await.expect("second run"), 0);
    }

    #[tokio::test]
    async fn recovery_sends_tasks_with_worktree_changes_to_review() {
        let pool = test_pool().await;
        let repo = std::env::temp_dir().join(format!("raid-test-{}", Uuid::new_v4().simple()));
        let worktree = GitService::quest_worktree_path(&repo, "q");
        std::fs::create_dir_all(&worktree).expect("mkdir");
        let git_ok = std::process::Command::new("git")
            .arg("init")
            .current_dir(&worktree)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !git_ok {
            return; // git unavailable in this environment
        }
        std::fs::write(worktree.join("work.txt"), "unfinished").expect("write");
        sqlx::query("UPDATE projects SET repository_path = $1 WHERE id = 'p'")
            .bind(repo.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("repo path");

        seed_task(&pool, "t", TaskStatus::Running).await;
        recover_interrupted(&pool).await.expect("recover");
        assert_eq!(task_row(&pool, "t").await.0, "needs_review");
        let _ = std::fs::remove_dir_all(&repo);
    }
}
