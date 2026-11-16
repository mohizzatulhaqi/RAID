pub mod commands;
pub mod events;
pub mod orchestrator;
pub mod permissions;
pub mod persistence;
pub mod providers;
pub mod repo;
pub mod verification;

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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize local SQLite database
            let app_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let db_path = app_dir.join("raid.db");
            let pool = tauri::async_runtime::block_on(persistence::init_db(&db_path))
                .map_err(|e| e.to_string())?;

            // Nothing can still be running after a restart: reconcile orphaned
            // tasks and checks so none stays stuck "running".
            match tauri::async_runtime::block_on(verification::recover_interrupted(&pool)) {
                Ok(n) if n > 0 => info!("Marked {n} interrupted verification runs as failed"),
                Ok(_) => {}
                Err(e) => tracing::warn!("Could not recover verification runs: {e}"),
            }
            if let Err(e) =
                tauri::async_runtime::block_on(orchestrator::lifecycle::recover_interrupted(&pool))
            {
                tracing::warn!("Could not recover interrupted tasks: {e}");
            }

            let (event_tx, mut event_rx) =
                tokio::sync::broadcast::channel::<events::NormalizedEvent>(200);
            let (exit_tx, mut exit_rx) =
                tokio::sync::mpsc::unbounded_channel::<providers::SessionExit>();

            // Forward agent events to the webview. An approval request also
            // moves the task and session to their waiting states.
            let handle = app_handle.clone();
            let event_pool = pool.clone();
            tauri::async_runtime::spawn(async move {
                use tokio::sync::broadcast::error::RecvError;
                loop {
                    match event_rx.recv().await {
                        Ok(event) => {
                            let _ = handle.emit("agent-event", &event);
                            if matches!(
                                event.payload,
                                events::EventPayload::ApprovalRequired { .. }
                            ) {
                                match orchestrator::lifecycle::on_approval_requested(
                                    &event_pool,
                                    &event.session_id,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        let _ =
                                            handle.emit("task-state-changed", &event.session_id);
                                    }
                                    Err(e) => {
                                        tracing::warn!("Could not record the approval request: {e}")
                                    }
                                }
                            }
                        }
                        Err(RecvError::Lagged(n)) => {
                            tracing::warn!("Dropped {n} agent events (UI too slow)")
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            });

            // Apply process exits to task and session status, then tell the UI.
            let handle = app_handle.clone();
            let exit_pool = pool.clone();
            tauri::async_runtime::spawn(async move {
                while let Some(exit) = exit_rx.recv().await {
                    if let Err(e) =
                        orchestrator::lifecycle::on_session_exit(&exit_pool, &exit).await
                    {
                        tracing::error!(
                            "Could not apply the exit of session {}: {e}",
                            exit.session_id
                        );
                    }
                    let _ = handle.emit("task-state-changed", &exit.task_id);
                }
            });

            let claude_adapter = Arc::new(providers::ClaudeCodeAdapter::new(
                event_tx,
                exit_tx,
                app_dir.join("hooks"),
            ));

            app.manage(commands::AppState {
                db: pool,
                claude_adapter,
            });

            info!("RAID backend state initialized successfully.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_project,
            commands::inspect_repo,
            commands::initialize_repo,
            commands::get_quests,
            commands::create_quest,
            commands::get_tasks,
            commands::create_task,
            commands::get_characters,
            commands::start_quest_task,
            commands::send_agent_input,
            commands::cancel_agent_session,
            commands::get_quest_changes,
            commands::get_task_detail,
            commands::get_boss_hp,
            commands::set_project_checks,
            commands::run_checks,
            commands::detect_checks,
            commands::mark_task_verified
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
