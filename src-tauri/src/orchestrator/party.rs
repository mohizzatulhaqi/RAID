use serde::Serialize;
use sqlx::SqlitePool;

use crate::events::NormalizedEvent;
use crate::orchestrator::state::SessionStatus;
use crate::persistence::models::Character;

/// Character state derived from its live session (skill `raid-state-machine`).
pub fn character_state(session: Option<SessionStatus>) -> &'static str {
    match session {
        Some(SessionStatus::Preparing) => "preparing",
        Some(SessionStatus::Running) => "running",
        Some(SessionStatus::Waiting) => "waiting",
        Some(SessionStatus::WaitingApproval) => "needs_approval",
        _ => "idle",
    }
}

#[derive(Debug, Serialize)]
pub struct PartyMember {
    #[serde(flatten)]
    pub character: Character,
    pub task_id: Option<String>,
    pub task_title: Option<String>,
    pub active_session_id: Option<String>,
    /// Latest real event of the live session; drives the world animation.
    pub last_event: Option<NormalizedEvent>,
}

type LiveRow = (String, String, String, String);

/// Every character with its current task and live session, if any.
pub async fn party(pool: &SqlitePool) -> Result<Vec<PartyMember>, sqlx::Error> {
    let characters: Vec<Character> =
        sqlx::query_as("SELECT * FROM characters ORDER BY created_at, rowid")
            .fetch_all(pool)
            .await?;

    let mut members = Vec::with_capacity(characters.len());
    for mut character in characters {
        let live: Option<LiveRow> = sqlx::query_as(
            "SELECT t.id, t.title, s.id, s.status FROM tasks t
             JOIN agent_sessions s ON s.task_id = t.id
             WHERE t.assigned_character_id = $1
               AND s.status IN ('preparing', 'running', 'waiting', 'waiting_approval')
             ORDER BY s.started_at DESC LIMIT 1",
        )
        .bind(&character.id)
        .fetch_optional(pool)
        .await?;

        let (task_id, task_title, session_id, status) = match live {
            Some((t, title, s, st)) => (Some(t), Some(title), Some(s), SessionStatus::parse(&st)),
            None => (None, None, None, None),
        };
        character.state = character_state(status).to_string();

        let last_event = match &session_id {
            Some(sid) => {
                let row: Option<(String,)> = sqlx::query_as(
                    "SELECT payload FROM agent_events WHERE session_id = $1
                     ORDER BY timestamp DESC, rowid DESC LIMIT 1",
                )
                .bind(sid)
                .fetch_optional(pool)
                .await?;
                row.and_then(|(json,)| serde_json::from_str(&json).ok())
            }
            None => None,
        };

        members.push(PartyMember {
            character,
            task_id,
            task_title,
            active_session_id: session_id,
            last_event,
        });
    }
    Ok(members)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::lifecycle::begin_session;
    use crate::orchestrator::state::TaskStatus;
    use crate::orchestrator::state::tests::{seed_task, test_pool};

    #[test]
    fn only_live_sessions_make_a_character_busy() {
        assert_eq!(character_state(None), "idle");
        assert_eq!(character_state(Some(SessionStatus::Running)), "running");
        assert_eq!(
            character_state(Some(SessionStatus::WaitingApproval)),
            "needs_approval"
        );
        assert_eq!(character_state(Some(SessionStatus::Succeeded)), "idle");
    }

    #[tokio::test]
    async fn party_shows_who_works_on_what() {
        let pool = test_pool().await;
        for id in ["aron", "lyra"] {
            sqlx::query(
                "INSERT INTO characters (id, display_name, class, party_role) VALUES ($1, $1, 'mage', 'dps')",
            )
            .bind(id)
            .execute(&pool)
            .await
            .expect("character");
        }
        seed_task(&pool, "t", TaskStatus::Ready).await;
        let sid = begin_session(&pool, "t", "/w", "b", "m", Some("lyra"))
            .await
            .expect("start");

        let members = party(&pool).await.expect("party");
        let lyra = members
            .iter()
            .find(|m| m.character.id == "lyra")
            .expect("lyra");
        assert_eq!(lyra.character.state, "running");
        assert_eq!(lyra.task_id.as_deref(), Some("t"));
        assert_eq!(lyra.active_session_id.as_deref(), Some(sid.as_str()));
        let aron = members
            .iter()
            .find(|m| m.character.id == "aron")
            .expect("aron");
        assert_eq!(aron.character.state, "idle");
        assert!(aron.task_id.is_none());
    }
}
