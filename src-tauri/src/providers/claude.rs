use crate::events::{EventPayload, NormalizedEvent, PresentationState};
use crate::providers::traits::{AgentProvider, ProviderError, SessionConfig};
use async_trait::async_trait;
use chrono::Utc;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

pub struct ActiveSession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
}

#[derive(Clone)]
pub struct ClaudeCodeAdapter {
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    event_tx: broadcast::Sender<NormalizedEvent>,
}

impl ClaudeCodeAdapter {
    pub fn new(event_tx: broadcast::Sender<NormalizedEvent>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// Basic line-by-line heuristic to normalize raw CLI output into PresentationState
    fn parse_line_to_event(session_id: &str, line: &str) -> NormalizedEvent {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();

        let (state, payload) = if lower.contains("reading") || lower.contains("inspecting") || lower.contains("searching") {
            (
                PresentationState::Exploring,
                EventPayload::FileRead {
                    path: trimmed.to_string(),
                },
            )
        } else if lower.contains("editing") || lower.contains("writing") || lower.contains("modifying") || lower.contains("patch") {
            (
                PresentationState::Fighting,
                EventPayload::FileChanged {
                    path: trimmed.to_string(),
                },
            )
        } else if lower.contains("allow") || lower.contains("approve") || lower.contains("permission") || lower.contains("(y/n)") || lower.ends_with('?') {
            (
                PresentationState::NeedsApproval,
                EventPayload::ApprovalRequired {
                    action_description: trimmed.to_string(),
                    is_risky: true,
                },
            )
        } else if lower.contains("error") || lower.contains("failed") || lower.contains("exception") {
            (
                PresentationState::Investigating,
                EventPayload::SessionOutput {
                    text: trimmed.to_string(),
                },
            )
        } else {
            (
                PresentationState::Fighting,
                EventPayload::SessionOutput {
                    text: trimmed.to_string(),
                },
            )
        };

        NormalizedEvent {
            session_id: session_id.to_string(),
            timestamp: Utc::now(),
            presentation_state: state,
            payload,
        }
    }
}

#[async_trait]
impl AgentProvider for ClaudeCodeAdapter {
    async fn start_session(&self, config: SessionConfig) -> Result<(), ProviderError> {
        info!("Starting Claude Code session {} for task {}", config.session_id, config.task_id);

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 30,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ProviderError::SpawnFailed(e.to_string()))?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.cwd(&config.worktree_path);
        // Start Claude Code passing the task prompt
        cmd.arg("-p");
        cmd.arg(&format!("{}: {}", config.task_title, config.task_description));

        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| ProviderError::SpawnFailed(format!("Failed to spawn 'claude' CLI: {}. Is Claude Code installed?", e)))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| ProviderError::SpawnFailed(e.to_string()))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| ProviderError::SpawnFailed(e.to_string()))?;

        // Store active session handles
        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(
                config.session_id.clone(),
                ActiveSession {
                    writer,
                    master: pair.master,
                },
            );
        }

        let session_id = config.session_id.clone();
        let event_tx = self.event_tx.clone();
        let sessions_ref = self.sessions.clone();

        // Emit initial session_started event
        let _ = event_tx.send(NormalizedEvent {
            session_id: session_id.clone(),
            timestamp: Utc::now(),
            presentation_state: PresentationState::Entering,
            payload: EventPayload::SessionStarted {
                session_id: session_id.clone(),
                task_id: config.task_id.clone(),
            },
        });

        // Background reader thread
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            let mut line_buffer = String::new();

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        info!("Session {} reached EOF", session_id);
                        break;
                    }
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buffer[..n]);
                        line_buffer.push_str(&text);

                        while let Some(pos) = line_buffer.find('\n') {
                            let line: String = line_buffer.drain(..=pos).collect();
                            let clean_line = strip_ansi_escapes::strip_str(&line);
                            if !clean_line.trim().is_empty() {
                                let event = Self::parse_line_to_event(&session_id, &clean_line);
                                let _ = event_tx.send(event);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Error reading from session {} PTY: {}", session_id, e);
                        break;
                    }
                }
            }

            let exit_status = child.wait();
            info!("Session {} exited with status {:?}", session_id, exit_status);

            let success = exit_status.map(|s| s.success()).unwrap_or(false);

            let final_state = if success {
                PresentationState::Verification
            } else {
                PresentationState::Failed
            };

            let final_payload = if success {
                EventPayload::SessionSucceeded {
                    summary: "Claude Code task finished".to_string(),
                }
            } else {
                EventPayload::SessionFailed {
                    error: "Claude Code process exited with non-zero status".to_string(),
                }
            };

            let _ = event_tx.send(NormalizedEvent {
                session_id: session_id.clone(),
                timestamp: Utc::now(),
                presentation_state: final_state,
                payload: final_payload,
            });

            // Cleanup session from active map
            let sessions = sessions_ref;
            let rt = tokio::runtime::Handle::current();
            rt.spawn(async move {
                let mut lock = sessions.lock().await;
                lock.remove(&session_id);
            });
        });

        Ok(())
    }

    async fn send_input(&self, session_id: &str, input: &str) -> Result<(), ProviderError> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            let mut formatted = input.to_string();
            if !formatted.ends_with('\n') {
                formatted.push('\n');
            }
            session
                .writer
                .write_all(formatted.as_bytes())
                .map_err(ProviderError::IoError)?;
            session.writer.flush().map_err(ProviderError::IoError)?;
            Ok(())
        } else {
            Err(ProviderError::SessionNotFound(session_id.to_string()))
        }
    }

    async fn cancel(&self, session_id: &str) -> Result<(), ProviderError> {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut session) = sessions.remove(session_id) {
            // Send Ctrl+C then kill
            let _ = session.writer.write_all(&[0x03]); // SIGINT / Ctrl+C
            let _ = session.writer.flush();
            info!("Cancelled session {}", session_id);
            let _ = self.event_tx.send(NormalizedEvent {
                session_id: session_id.to_string(),
                timestamp: Utc::now(),
                presentation_state: PresentationState::Cancelled,
                payload: EventPayload::SessionCancelled,
            });
            Ok(())
        } else {
            Err(ProviderError::SessionNotFound(session_id.to_string()))
        }
    }
}

// Simple strip helper if strip_ansi_escapes crate is not added yet
mod strip_ansi_escapes {
    pub fn strip_str(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_escape = false;

        for ch in s.chars() {
            if ch == '\x1b' {
                in_escape = true;
            } else if in_escape {
                if ch.is_ascii_alphabetic() {
                    in_escape = false;
                }
            } else {
                result.push(ch);
            }
        }
        result
    }
}
