use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use std::path::Path;
use tracing::info;

pub async fn init_db(db_path: &Path) -> Result<SqlitePool, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let db_str = db_path.to_string_lossy();
    info!("Connecting to SQLite database at {}", db_str);

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_str))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    info!("Running embedded SQLite migrations...");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await?;
    info!("Database initialized and migrations applied successfully.");

    Ok(pool)
}
