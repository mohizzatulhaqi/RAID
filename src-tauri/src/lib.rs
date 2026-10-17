pub mod commands;
pub mod db;
pub mod events;
pub mod git;
pub mod models;
pub mod providers;

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    info!("Starting RAID Desktop Application...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let (event_tx, mut event_rx) = tokio::sync::broadcast::channel::<events::NormalizedEvent>(200);

            // Forward backend agent events directly to Tauri webview
            let handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    let _ = handle_clone.emit("agent-event", &event);
                }
            });

            // Initialize local SQLite database
            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let db_path = app_dir.join("raid.db");

            let pool = tauri::async_runtime::block_on(async {
                db::init_db(&db_path)
                    .await
                    .expect("Failed to initialize SQLite database and migrations")
            });

            let claude_adapter = Arc::new(providers::ClaudeCodeAdapter::new(event_tx));

            app.manage(commands::AppState {
                db: pool,
                claude_adapter,
            });

            info!("RAID backend state initialized successfully.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_project,
            commands::get_quests,
            commands::create_quest,
            commands::get_tasks,
            commands::create_task,
            commands::get_characters,
            commands::start_quest_task,
            commands::send_agent_input,
            commands::cancel_agent_session,
            commands::get_quest_diff
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
