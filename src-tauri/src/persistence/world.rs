use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

pub async fn load_camera(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Option<Camera>, sqlx::Error> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT camera_state FROM world_states WHERE project_id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(json,)| serde_json::from_str(&json).ok()))
}

pub async fn save_camera(
    pool: &SqlitePool,
    project_id: &str,
    camera: &Camera,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(camera).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        "INSERT INTO world_states (project_id, camera_state, updated_at)
         VALUES ($1, $2, CURRENT_TIMESTAMP)
         ON CONFLICT(project_id) DO UPDATE
         SET camera_state = excluded.camera_state, updated_at = CURRENT_TIMESTAMP",
    )
    .bind(project_id)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::state::tests::test_pool;

    #[tokio::test]
    async fn camera_is_saved_and_replaced() {
        let pool = test_pool().await;
        assert_eq!(load_camera(&pool, "p").await.expect("load"), None);
        let first = Camera {
            x: 10.0,
            y: 20.0,
            zoom: 2.0,
        };
        save_camera(&pool, "p", &first).await.expect("save");
        let second = Camera {
            x: 5.0,
            y: 6.0,
            zoom: 1.0,
        };
        save_camera(&pool, "p", &second).await.expect("replace");
        assert_eq!(load_camera(&pool, "p").await.expect("load"), Some(second));
    }

    #[tokio::test]
    async fn the_migration_default_camera_is_readable() {
        let pool = test_pool().await;
        sqlx::query("INSERT INTO world_states (project_id) VALUES ('p')")
            .execute(&pool)
            .await
            .expect("row");
        assert_eq!(
            load_camera(&pool, "p").await.expect("load"),
            Some(Camera {
                x: 0.0,
                y: 0.0,
                zoom: 1.0
            })
        );
    }
}
