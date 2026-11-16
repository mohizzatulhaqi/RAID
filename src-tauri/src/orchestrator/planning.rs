use serde::Serialize;
use sqlx::SqliteConnection;
use thiserror::Error;
use uuid::Uuid;

use crate::orchestrator::state::{
    TaskStatus, TransitionContext, TransitionError, add_dependency, transition_task,
};

/// Statuses in which a task's text and dependencies may still change.
const EDITABLE: [TaskStatus; 5] = [
    TaskStatus::Created,
    TaskStatus::Ready,
    TaskStatus::Blocked,
    TaskStatus::Failed,
    TaskStatus::Cancelled,
];
const PRIORITIES: [&str; 4] = ["low", "medium", "high", "urgent"];

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("Title cannot be empty")]
    EmptyTitle,
    #[error("Unknown priority: {0}")]
    BadPriority(String),
    #[error("Tasks can only depend on, or belong to, tasks of the same quest")]
    OtherQuest,
    #[error("A task cannot be edited while it is {0}")]
    NotEditable(&'static str),
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
}

pub struct TaskInput<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub priority: &'a str,
    pub depends_on: &'a [String],
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DependencyView {
    pub id: String,
    pub title: String,
    pub status: String,
}

fn check_input(input: &TaskInput<'_>) -> Result<(), PlanError> {
    if input.title.trim().is_empty() {
        return Err(PlanError::EmptyTitle);
    }
    if !PRIORITIES.contains(&input.priority) {
        return Err(PlanError::BadPriority(input.priority.to_string()));
    }
    Ok(())
}

async fn quest_of(conn: &mut SqliteConnection, task_id: &str) -> Result<String, PlanError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT quest_id FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_optional(conn)
        .await?;
    row.map(|(q,)| q)
        .ok_or(PlanError::Transition(TransitionError::NotFound))
}

async fn same_quest(
    conn: &mut SqliteConnection,
    quest_id: &str,
    ids: &[String],
) -> Result<(), PlanError> {
    for id in ids {
        if quest_of(&mut *conn, id).await? != quest_id {
            return Err(PlanError::OtherQuest);
        }
    }
    Ok(())
}

async fn status_of(conn: &mut SqliteConnection, task_id: &str) -> Result<TaskStatus, PlanError> {
    let (raw,): (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(&mut *conn)
        .await?;
    TaskStatus::parse(&raw).ok_or(PlanError::Transition(TransitionError::UnknownStatus(raw)))
}

async fn deps_complete(conn: &mut SqliteConnection, task_id: &str) -> Result<bool, sqlx::Error> {
    let (open,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_dependencies d JOIN tasks t ON t.id = d.depends_on_task_id
         WHERE d.task_id = $1 AND t.status != 'completed'",
    )
    .bind(task_id)
    .fetch_one(conn)
    .await?;
    Ok(open == 0)
}

/// Moves a waiting task between Ready and Blocked to match its prerequisites.
pub async fn sync_blocked(conn: &mut SqliteConnection, task_id: &str) -> Result<(), PlanError> {
    let ctx = TransitionContext::new("planner");
    let complete = deps_complete(&mut *conn, task_id).await?;
    match (status_of(&mut *conn, task_id).await?, complete) {
        (TaskStatus::Created | TaskStatus::Blocked, true) => {
            transition_task(conn, task_id, TaskStatus::Ready, &ctx).await?;
        }
        (TaskStatus::Created | TaskStatus::Ready, false) => {
            transition_task(
                conn,
                task_id,
                TaskStatus::Blocked,
                &ctx.reason("Waiting for other tasks"),
            )
            .await?;
        }
        _ => {}
    }
    Ok(())
}

/// Re-checks every task that depends on `task_id` (call after it completes or reopens).
pub async fn sync_dependents(conn: &mut SqliteConnection, task_id: &str) -> Result<(), PlanError> {
    let dependents: Vec<(String,)> =
        sqlx::query_as("SELECT task_id FROM task_dependencies WHERE depends_on_task_id = $1")
            .bind(task_id)
            .fetch_all(&mut *conn)
            .await?;
    for (id,) in dependents {
        sync_blocked(&mut *conn, &id).await?;
    }
    Ok(())
}

pub async fn create_task(
    conn: &mut SqliteConnection,
    quest_id: &str,
    parent_task_id: Option<&str>,
    input: &TaskInput<'_>,
) -> Result<String, PlanError> {
    check_input(input)?;
    if let Some(parent) = parent_task_id {
        same_quest(&mut *conn, quest_id, &[parent.to_string()]).await?;
    }
    same_quest(&mut *conn, quest_id, input.depends_on).await?;

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO tasks (id, quest_id, parent_task_id, title, description, status, priority)
         VALUES ($1, $2, $3, $4, $5, 'created', $6)",
    )
    .bind(&id)
    .bind(quest_id)
    .bind(parent_task_id)
    .bind(input.title.trim())
    .bind(input.description.trim())
    .bind(input.priority)
    .execute(&mut *conn)
    .await?;

    for dep in input.depends_on {
        add_dependency(&mut *conn, &id, dep).await?;
    }
    sync_blocked(conn, &id).await?;
    Ok(id)
}

pub async fn update_task(
    conn: &mut SqliteConnection,
    task_id: &str,
    input: &TaskInput<'_>,
) -> Result<(), PlanError> {
    check_input(input)?;
    let status = status_of(&mut *conn, task_id).await?;
    if !EDITABLE.contains(&status) {
        return Err(PlanError::NotEditable(status.as_str()));
    }
    let quest_id = quest_of(&mut *conn, task_id).await?;
    same_quest(&mut *conn, &quest_id, input.depends_on).await?;

    sqlx::query(
        "UPDATE tasks SET title = $1, description = $2, priority = $3, updated_at = CURRENT_TIMESTAMP
         WHERE id = $4",
    )
    .bind(input.title.trim())
    .bind(input.description.trim())
    .bind(input.priority)
    .bind(task_id)
    .execute(&mut *conn)
    .await?;

    let current: Vec<(String,)> =
        sqlx::query_as("SELECT depends_on_task_id FROM task_dependencies WHERE task_id = $1")
            .bind(task_id)
            .fetch_all(&mut *conn)
            .await?;
    for (dep,) in &current {
        if !input.depends_on.contains(dep) {
            sqlx::query(
                "DELETE FROM task_dependencies WHERE task_id = $1 AND depends_on_task_id = $2",
            )
            .bind(task_id)
            .bind(dep)
            .execute(&mut *conn)
            .await?;
        }
    }
    for dep in input.depends_on {
        if !current.iter().any(|(c,)| c == dep) {
            add_dependency(&mut *conn, task_id, dep).await?;
        }
    }
    sync_blocked(conn, task_id).await
}

pub async fn update_quest(
    conn: &mut SqliteConnection,
    quest_id: &str,
    title: &str,
    description: &str,
    priority: &str,
    boss_name: &str,
) -> Result<(), PlanError> {
    if title.trim().is_empty() {
        return Err(PlanError::EmptyTitle);
    }
    if !PRIORITIES.contains(&priority) {
        return Err(PlanError::BadPriority(priority.to_string()));
    }
    let boss = boss_name.trim();
    sqlx::query(
        "UPDATE quests SET title = $1, description = $2, priority = $3, boss_name = $4 WHERE id = $5",
    )
    .bind(title.trim())
    .bind(description.trim())
    .bind(priority)
    .bind(if boss.is_empty() { None } else { Some(boss) })
    .bind(quest_id)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn dependencies_of(
    conn: &mut SqliteConnection,
    task_id: &str,
) -> Result<Vec<DependencyView>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.id, t.title, t.status FROM task_dependencies d JOIN tasks t ON t.id = d.depends_on_task_id
         WHERE d.task_id = $1 ORDER BY t.created_at",
    )
    .bind(task_id)
    .fetch_all(conn)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::state::tests::{seed_task, test_pool};

    fn input<'a>(title: &'a str, deps: &'a [String]) -> TaskInput<'a> {
        TaskInput {
            title,
            description: "d",
            priority: "medium",
            depends_on: deps,
        }
    }

    async fn status(pool: &sqlx::SqlitePool, id: &str) -> String {
        let (s,): (String,) = sqlx::query_as("SELECT status FROM tasks WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .expect("status");
        s
    }

    #[tokio::test]
    async fn new_tasks_are_ready_or_blocked_by_their_prerequisites() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("conn");
        let a = create_task(&mut conn, "q", None, &input("Add migration", &[]))
            .await
            .expect("a");
        assert_eq!(status(&pool, &a).await, "ready");

        let b = create_task(
            &mut conn,
            "q",
            None,
            &input("Use table", std::slice::from_ref(&a)),
        )
        .await
        .expect("b");
        assert_eq!(status(&pool, &b).await, "blocked");
        let deps = dependencies_of(&mut conn, &b).await.expect("deps");
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].title, "Add migration");

        sqlx::query("UPDATE tasks SET status = 'completed' WHERE id = $1")
            .bind(&a)
            .execute(&pool)
            .await
            .expect("complete");
        sync_dependents(&mut conn, &a).await.expect("sync");
        assert_eq!(status(&pool, &b).await, "ready");
    }

    #[tokio::test]
    async fn editing_dependencies_blocks_unblocks_and_rejects_cycles() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("conn");
        let a = create_task(&mut conn, "q", None, &input("A", &[]))
            .await
            .expect("a");
        let b = create_task(&mut conn, "q", None, &input("B", &[]))
            .await
            .expect("b");

        update_task(&mut conn, &b, &input("B", std::slice::from_ref(&a)))
            .await
            .expect("add dep");
        assert_eq!(status(&pool, &b).await, "blocked");

        let cycle = update_task(&mut conn, &a, &input("A", std::slice::from_ref(&b))).await;
        assert!(matches!(
            cycle,
            Err(PlanError::Transition(TransitionError::DependencyCycle))
        ));

        update_task(&mut conn, &b, &input("B renamed", &[]))
            .await
            .expect("remove dep");
        assert_eq!(status(&pool, &b).await, "ready");
        let (title,): (String,) = sqlx::query_as("SELECT title FROM tasks WHERE id = $1")
            .bind(&b)
            .fetch_one(&pool)
            .await
            .expect("title");
        assert_eq!(title, "B renamed");
    }

    #[tokio::test]
    async fn running_tasks_and_bad_input_are_refused() {
        let pool = test_pool().await;
        seed_task(&pool, "run", TaskStatus::Running).await;
        let mut conn = pool.acquire().await.expect("conn");
        assert!(matches!(
            update_task(&mut conn, "run", &input("x", &[])).await,
            Err(PlanError::NotEditable("running"))
        ));
        assert!(matches!(
            create_task(&mut conn, "q", None, &input("  ", &[])).await,
            Err(PlanError::EmptyTitle)
        ));
        let bad = TaskInput {
            priority: "now",
            ..input("x", &[])
        };
        assert!(matches!(
            create_task(&mut conn, "q", None, &bad).await,
            Err(PlanError::BadPriority(_))
        ));
    }

    #[tokio::test]
    async fn links_across_quests_are_refused() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO quests (id, project_id, title, description) VALUES ('q2', 'p', 't', 'd')",
        )
        .execute(&pool)
        .await
        .expect("quest");
        let mut conn = pool.acquire().await.expect("conn");
        let other = create_task(&mut conn, "q2", None, &input("Other", &[]))
            .await
            .expect("other");
        assert!(matches!(
            create_task(&mut conn, "q", Some(&other), &input("Child", &[])).await,
            Err(PlanError::OtherQuest)
        ));
        assert!(matches!(
            create_task(&mut conn, "q", None, &input("Dep", &[other])).await,
            Err(PlanError::OtherQuest)
        ));
    }

    #[tokio::test]
    async fn quests_can_be_edited() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("conn");
        update_quest(&mut conn, "q", "New title", "New text", "high", "")
            .await
            .expect("update");
        let (title, priority, boss): (String, String, Option<String>) =
            sqlx::query_as("SELECT title, priority, boss_name FROM quests WHERE id = 'q'")
                .fetch_one(&pool)
                .await
                .expect("quest");
        assert_eq!(
            (title.as_str(), priority.as_str(), boss),
            ("New title", "high", None)
        );
    }
}
