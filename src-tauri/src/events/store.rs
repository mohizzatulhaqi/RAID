use sqlx::SqlitePool;
use uuid::Uuid;

use super::NormalizedEvent;

pub async fn save_event(pool: &SqlitePool, event: &NormalizedEvent) -> Result<(), sqlx::Error> {
    let payload = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
    let event_type = serde_json::to_value(&event.payload)
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());
    sqlx::query(
        "INSERT INTO agent_events (id, session_id, event_type, timestamp, payload, source)
         VALUES ($1, $2, $3, $4, $5, 'agent')",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&event.session_id)
    .bind(event_type)
    .bind(event.timestamp)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}

fn parse(rows: Vec<(String,)>) -> Vec<NormalizedEvent> {
    let mut events: Vec<NormalizedEvent> = rows
        .into_iter()
        .filter_map(|(json,)| serde_json::from_str(&json).ok())
        .collect();
    events.reverse();
    events
}

/// Latest events of a project, oldest first.
pub async fn project_events(
    pool: &SqlitePool,
    project_id: &str,
    limit: i64,
) -> Result<Vec<NormalizedEvent>, sqlx::Error> {
    let rows = sqlx::query_as(
        "SELECT e.payload FROM agent_events e
         JOIN agent_sessions s ON s.id = e.session_id
         JOIN tasks t ON t.id = s.task_id
         JOIN quests q ON q.id = t.quest_id
         WHERE q.project_id = $1
         ORDER BY e.timestamp DESC, e.rowid DESC LIMIT $2",
    )
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(parse(rows))
}

/// Latest events of one task's sessions, oldest first.
pub async fn task_events(
    pool: &SqlitePool,
    task_id: &str,
    limit: i64,
) -> Result<Vec<NormalizedEvent>, sqlx::Error> {
    let rows = sqlx::query_as(
        "SELECT e.payload FROM agent_events e
         JOIN agent_sessions s ON s.id = e.session_id
         WHERE s.task_id = $1
         ORDER BY e.timestamp DESC, e.rowid DESC LIMIT $2",
    )
    .bind(task_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(parse(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventPayload, PresentationState};
    use crate::orchestrator::state::TaskStatus;
    use crate::orchestrator::state::tests::{seed_task, test_pool};
    use chrono::{Duration, Utc};

    fn event(session: &str, text: &str, offset: i64) -> NormalizedEvent {
        NormalizedEvent {
            session_id: session.to_string(),
            timestamp: Utc::now() + Duration::seconds(offset),
            presentation_state: PresentationState::Exploring,
            payload: EventPayload::SessionOutput {
                text: text.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn events_round_trip_in_order_and_respect_the_limit() {
        let pool = test_pool().await;
        seed_task(&pool, "t", TaskStatus::Running).await;
        sqlx::query(
            "INSERT INTO agent_sessions (id, task_id, status) VALUES ('s', 't', 'running')",
        )
        .execute(&pool)
        .await
        .expect("session");
        for (i, text) in ["one", "two", "three"].iter().enumerate() {
            save_event(&pool, &event("s", text, i as i64))
                .await
                .expect("save");
        }

        let texts = |events: Vec<NormalizedEvent>| -> Vec<String> {
            events
                .into_iter()
                .map(|e| match e.payload {
                    EventPayload::SessionOutput { text } => text,
                    _ => String::new(),
                })
                .collect()
        };
        assert_eq!(
            texts(task_events(&pool, "t", 10).await.expect("task")),
            vec!["one", "two", "three"]
        );
        assert_eq!(
            texts(project_events(&pool, "p", 2).await.expect("project")),
            vec!["two", "three"]
        );
    }
}
