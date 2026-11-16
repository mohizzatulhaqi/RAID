use crate::events::{EventPayload, NormalizedEvent, PresentationState};
use crate::providers::traits::{
    AgentProvider, ProviderError, SessionConfig, SessionExit, SessionOutcome,
};
use async_trait::async_trait;
use chrono::Utc;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{Mutex, broadcast, mpsc};
use tracing::{info, warn};

/// Time the agent gets to stop after Ctrl+C before its process is killed.
const CANCEL_GRACE: Duration = Duration::from_millis(1500);
/// Time the agent gets to exit after `/exit` before its process is killed.
const EXIT_GRACE: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(150);
/// How long a blocked permission hook waits for an answer (seconds).
const HOOK_TIMEOUT_SECS: u64 = 3600;
const TRUST_MARKER: &str = "Itrustthisfolder";
const TERMINAL_TAIL_BYTES: usize = 8 * 1024;

const HOOK_SCRIPT: &str = r#"#!/bin/sh
# RAID hook bridge. usage: raid-hook.sh <session-dir> <event>
# Saves the hook payload for RAID. For PermissionRequest it waits for RAID's
# answer and prints it. Printing nothing falls back to Claude Code's own flow.
dir="$1"; ev="$2"
id="$(date +%s)-$$"
cat > "$dir/$ev-$id.tmp" || exit 0
mv "$dir/$ev-$id.tmp" "$dir/$ev-$id.json"
if [ "$ev" = "PermissionRequest" ]; then
  while [ ! -f "$dir/$ev-$id.resp" ]; do
    [ -f "$dir/.closed" ] && exit 0
    sleep 0.2
  done
  cat "$dir/$ev-$id.resp"
fi
exit 0
"#;

#[derive(Debug, Clone)]
enum PendingKind {
    /// A blocked PermissionRequest hook; the answer goes to `<stem>.resp`.
    Hook { stem: String },
    /// Claude Code's folder trust menu, answered with keystrokes.
    Trust,
}

#[derive(Debug, Clone)]
struct PendingApproval {
    kind: PendingKind,
    description: String,
}

type PendingQueue = Arc<StdMutex<VecDeque<PendingApproval>>>;

pub struct ActiveSession {
    pub writer: Box<dyn Write + Send>,
    pub master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    cancelled: Arc<AtomicBool>,
    exited: Arc<AtomicBool>,
    pending: PendingQueue,
    dir: PathBuf,
}

#[derive(Clone)]
pub struct ClaudeCodeAdapter {
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    event_tx: broadcast::Sender<NormalizedEvent>,
    exit_tx: mpsc::UnboundedSender<SessionExit>,
    hooks_root: PathBuf,
}

fn lock_pending(q: &PendingQueue) -> std::sync::MutexGuard<'_, VecDeque<PendingApproval>> {
    q.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Plain-text description of a tool call, from the fields Claude Code sent.
fn describe_tool(name: &str, input: &Value) -> String {
    let text = |key: &str| input.get(key).and_then(Value::as_str);
    if let Some(command) = text("command") {
        return format!("{name}: {command}");
    }
    if let Some(path) = text("file_path").or_else(|| text("path")) {
        return format!("{name} {path}");
    }
    name.to_string()
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl ClaudeCodeAdapter {
    pub fn new(
        event_tx: broadcast::Sender<NormalizedEvent>,
        exit_tx: mpsc::UnboundedSender<SessionExit>,
        hooks_root: PathBuf,
    ) -> Self {
        // No session can be alive at startup, so leftovers are stale.
        let _ = std::fs::remove_dir_all(&hooks_root);
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            exit_tx,
            hooks_root,
        }
    }

    fn emit(
        tx: &broadcast::Sender<NormalizedEvent>,
        session_id: &str,
        state: PresentationState,
        payload: EventPayload,
    ) {
        let _ = tx.send(NormalizedEvent {
            session_id: session_id.to_string(),
            timestamp: Utc::now(),
            presentation_state: state,
            payload,
        });
    }

    fn emit_approval(tx: &broadcast::Sender<NormalizedEvent>, session_id: &str, description: &str) {
        Self::emit(
            tx,
            session_id,
            PresentationState::NeedsApproval,
            EventPayload::ApprovalRequired {
                action_description: description.to_string(),
                is_risky: true,
            },
        );
    }

    /// Writes the hook bridge script and this session's `settings.json`.
    fn prepare_hooks(&self, session_dir: &Path) -> Result<PathBuf, ProviderError> {
        #[cfg(not(unix))]
        {
            let _ = session_dir;
            return Err(ProviderError::SpawnFailed(
                "Claude Code hooks are only supported on macOS and Linux for now".to_string(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            std::fs::create_dir_all(session_dir)?;
            let script = self.hooks_root.join("raid-hook.sh");
            std::fs::write(&script, HOOK_SCRIPT)?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;

            let hook = |event: &str, timeout: u64| {
                serde_json::json!([{
                    "hooks": [{
                        "type": "command",
                        "command": format!(
                            "{} {} {}",
                            sh_quote(&script.to_string_lossy()),
                            sh_quote(&session_dir.to_string_lossy()),
                            event
                        ),
                        "timeout": timeout,
                    }]
                }])
            };
            let settings = serde_json::json!({
                "hooks": {
                    "PreToolUse": hook("PreToolUse", 10),
                    "PostToolUse": hook("PostToolUse", 10),
                    "PermissionRequest": hook("PermissionRequest", HOOK_TIMEOUT_SECS),
                    "Stop": hook("Stop", 10),
                }
            });
            let path = session_dir.join("settings.json");
            std::fs::write(&path, settings.to_string())?;
            Ok(path)
        }
    }
}

struct WatchCtx {
    session_id: String,
    dir: PathBuf,
    event_tx: broadcast::Sender<NormalizedEvent>,
    sessions: Arc<Mutex<HashMap<String, ActiveSession>>>,
    pending: PendingQueue,
    exited: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

/// Reads new hook payload files and turns them into normalized events.
async fn drain_hooks(ctx: &WatchCtx, seen: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(&ctx.dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, String)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".json") || name == "settings.json" || seen.contains(&name) {
                return None;
            }
            let modified = e.metadata().and_then(|m| m.modified()).ok()?;
            Some((modified, name))
        })
        .collect();
    files.sort();

    for (_, name) in files {
        seen.insert(name.clone());
        let Ok(text) = std::fs::read_to_string(ctx.dir.join(&name)) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(&text) else {
            warn!("Ignoring unreadable hook payload {name}");
            continue;
        };
        handle_hook(ctx, name.trim_end_matches(".json"), &payload).await;
    }
}

async fn handle_hook(ctx: &WatchCtx, stem: &str, payload: &Value) {
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let tool = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let input = payload.get("tool_input").unwrap_or(&Value::Null);
    let path = input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str);
    let tx = &ctx.event_tx;
    let sid = &ctx.session_id;

    match (event, tool) {
        ("PreToolUse", "Read" | "Grep" | "Glob" | "LS") => {
            if let Some(p) = path.or_else(|| input.get("pattern").and_then(Value::as_str)) {
                ClaudeCodeAdapter::emit(
                    tx,
                    sid,
                    PresentationState::Exploring,
                    EventPayload::FileRead {
                        path: p.to_string(),
                    },
                );
            }
        }
        ("PreToolUse", "Bash") => {
            if let Some(command) = input.get("command").and_then(Value::as_str) {
                ClaudeCodeAdapter::emit(
                    tx,
                    sid,
                    PresentationState::Fighting,
                    EventPayload::CommandStarted {
                        command: command.to_string(),
                    },
                );
            }
        }
        // Only after the tool succeeded: a change is reported when it happened.
        ("PostToolUse", "Write" | "Edit" | "MultiEdit" | "NotebookEdit") => {
            if let Some(p) = path {
                ClaudeCodeAdapter::emit(
                    tx,
                    sid,
                    PresentationState::Fighting,
                    EventPayload::FileChanged {
                        path: p.to_string(),
                    },
                );
            }
        }
        ("PermissionRequest", _) => {
            let description = describe_tool(tool, input);
            let first = {
                let mut queue = lock_pending(&ctx.pending);
                queue.push_back(PendingApproval {
                    kind: PendingKind::Hook {
                        stem: stem.to_string(),
                    },
                    description: description.clone(),
                });
                queue.len() == 1
            };
            // Later requests are announced when they reach the front.
            if first {
                ClaudeCodeAdapter::emit_approval(tx, sid, &description);
            }
        }
        ("Stop", _) => {
            ctx.stopped.store(true, Ordering::SeqCst);
            let summary = payload
                .get("last_assistant_message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if !summary.is_empty() {
                ClaudeCodeAdapter::emit(
                    tx,
                    sid,
                    PresentationState::Verification,
                    EventPayload::SessionOutput { text: summary },
                );
            }
            // The turn is over: end the interactive session so it exits normally.
            let mut sessions = ctx.sessions.lock().await;
            if let Some(session) = sessions.get_mut(sid) {
                let _ = session.writer.write_all(b"/exit\r");
                let _ = session.writer.flush();
            }
            drop(sessions);
            let sessions = ctx.sessions.clone();
            let exited = ctx.exited.clone();
            let sid = sid.clone();
            tokio::spawn(async move {
                tokio::time::sleep(EXIT_GRACE).await;
                if !exited.load(Ordering::SeqCst)
                    && let Some(session) = sessions.lock().await.get_mut(&sid)
                    && let Err(e) = session.killer.kill()
                {
                    warn!("Could not stop session {sid} after it finished: {e}");
                }
            });
        }
        _ => {}
    }
}

async fn watch_hooks(ctx: WatchCtx) {
    let mut seen = HashSet::new();
    loop {
        drain_hooks(&ctx, &mut seen).await;
        if ctx.exited.load(Ordering::SeqCst) {
            drain_hooks(&ctx, &mut seen).await;
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    // Let any blocked hook script end, then remove the session directory.
    let _ = std::fs::write(ctx.dir.join(".closed"), b"");
    tokio::time::sleep(Duration::from_millis(500)).await;
    let _ = std::fs::remove_dir_all(&ctx.dir);
}

#[async_trait]
impl AgentProvider for ClaudeCodeAdapter {
    async fn start_session(&self, config: SessionConfig) -> Result<(), ProviderError> {
        info!(
            "Starting Claude Code session {} for task {}",
            config.session_id, config.task_id
        );

        let session_dir = self.hooks_root.join(&config.session_id);
        let settings = self.prepare_hooks(&session_dir)?;

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
        // Never inherit auto or bypass modes from the user's settings.
        cmd.arg("--permission-mode");
        cmd.arg("default");
        cmd.arg("--settings");
        cmd.arg(&settings);
        cmd.arg(format!(
            "{}: {}",
            config.task_title, config.task_description
        ));

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| {
            ProviderError::SpawnFailed(format!(
                "Failed to spawn 'claude' CLI: {}. Is Claude Code installed?",
                e
            ))
        })?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| ProviderError::SpawnFailed(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| ProviderError::SpawnFailed(e.to_string()))?;

        let cancelled = Arc::new(AtomicBool::new(false));
        let exited = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let pending: PendingQueue = Arc::new(StdMutex::new(VecDeque::new()));

        {
            let mut sessions = self.sessions.lock().await;
            sessions.insert(
                config.session_id.clone(),
                ActiveSession {
                    writer,
                    master: pair.master,
                    killer: child.clone_killer(),
                    cancelled: cancelled.clone(),
                    exited: exited.clone(),
                    pending: pending.clone(),
                    dir: session_dir.clone(),
                },
            );
        }

        let session_id = config.session_id.clone();
        let event_tx = self.event_tx.clone();
        let sessions_ref = self.sessions.clone();
        let exit_tx = self.exit_tx.clone();
        let task_id = config.task_id.clone();
        let worktree = config.worktree_path.to_string_lossy().to_string();
        let rt = tokio::runtime::Handle::current();

        Self::emit(
            &event_tx,
            &session_id,
            PresentationState::Entering,
            EventPayload::SessionStarted {
                session_id: session_id.clone(),
                task_id: task_id.clone(),
            },
        );

        rt.spawn(watch_hooks(WatchCtx {
            session_id: session_id.clone(),
            dir: session_dir,
            event_tx: event_tx.clone(),
            sessions: sessions_ref.clone(),
            pending: pending.clone(),
            exited: exited.clone(),
            stopped: stopped.clone(),
        }));

        // Reader thread: drains the PTY (so the agent never blocks on output),
        // watches for the folder trust prompt, and reports the exit.
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            let mut tail = String::new();
            let mut trust_seen = false;

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        if trust_seen {
                            continue;
                        }
                        tail.push_str(&String::from_utf8_lossy(&buffer[..n]));
                        if tail.len() > TERMINAL_TAIL_BYTES {
                            let cut = tail.len() - TERMINAL_TAIL_BYTES;
                            let cut = (cut..tail.len())
                                .find(|i| tail.is_char_boundary(*i))
                                .unwrap_or(tail.len());
                            tail.drain(..cut);
                        }
                        let squashed: String = strip_ansi_escapes::strip_str(&tail)
                            .chars()
                            .filter(|c| !c.is_whitespace())
                            .collect();
                        if squashed.contains(TRUST_MARKER) {
                            trust_seen = true;
                            let description = format!(
                                "Trust this folder so Claude Code can read, edit and run files in it: {worktree}"
                            );
                            let first = {
                                let mut queue = lock_pending(&pending);
                                queue.push_back(PendingApproval {
                                    kind: PendingKind::Trust,
                                    description: description.clone(),
                                });
                                queue.len() == 1
                            };
                            if first {
                                Self::emit_approval(&event_tx, &session_id, &description);
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
            info!(
                "Session {} exited with status {:?}",
                session_id, exit_status
            );
            exited.store(true, Ordering::SeqCst);

            // Finishing is proven by the Stop hook; exiting without it (quit,
            // crash, trust denied) is a failure, never a success.
            let outcome = if cancelled.load(Ordering::SeqCst) {
                SessionOutcome::Cancelled
            } else if stopped.load(Ordering::SeqCst) {
                SessionOutcome::Succeeded
            } else {
                SessionOutcome::Failed
            };

            let (final_state, final_payload) = match outcome {
                SessionOutcome::Cancelled => {
                    (PresentationState::Cancelled, EventPayload::SessionCancelled)
                }
                SessionOutcome::Succeeded => (
                    PresentationState::Verification,
                    EventPayload::SessionSucceeded {
                        summary: "Claude Code finished its turn".to_string(),
                    },
                ),
                SessionOutcome::Failed => (
                    PresentationState::Failed,
                    EventPayload::SessionFailed {
                        error: "Claude Code exited before finishing the task".to_string(),
                    },
                ),
            };
            Self::emit(&event_tx, &session_id, final_state, final_payload);

            let _ = exit_tx.send(SessionExit {
                session_id: session_id.clone(),
                task_id,
                outcome,
            });

            rt.spawn(async move {
                sessions_ref.lock().await.remove(&session_id);
            });
        });

        Ok(())
    }

    async fn send_input(&self, session_id: &str, input: &str) -> Result<(), ProviderError> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;

        let answered = lock_pending(&session.pending).pop_front();
        let Some(request) = answered else {
            // No approval pending: pass the text to the terminal as typed.
            let mut formatted = input.to_string();
            if !formatted.ends_with('\n') {
                formatted.push('\n');
            }
            session.writer.write_all(formatted.as_bytes())?;
            session.writer.flush()?;
            return Ok(());
        };

        let approved = matches!(input.trim().to_lowercase().as_str(), "y" | "yes" | "allow");
        match &request.kind {
            PendingKind::Hook { stem } => {
                let decision = if approved {
                    serde_json::json!({"behavior": "allow"})
                } else {
                    serde_json::json!({"behavior": "deny", "message": "Denied by the user in RAID"})
                };
                let body = serde_json::json!({
                    "hookSpecificOutput": {
                        "hookEventName": "PermissionRequest",
                        "decision": decision,
                    }
                });
                // Write then rename so the hook script never reads a partial answer.
                let tmp = session.dir.join(format!("{stem}.resp.tmp"));
                let resp = session.dir.join(format!("{stem}.resp"));
                tokio::fs::write(&tmp, body.to_string()).await?;
                tokio::fs::rename(&tmp, &resp).await?;
            }
            PendingKind::Trust => {
                // Menu: "No, exit" is first, "Yes, I trust this folder" second.
                let keys: &[u8] = if approved { b"\x1b[B\r" } else { b"\x1b" };
                session.writer.write_all(keys)?;
                session.writer.flush()?;
            }
        }

        Self::emit(
            &self.event_tx,
            session_id,
            if approved {
                PresentationState::Fighting
            } else {
                PresentationState::Investigating
            },
            EventPayload::ApprovalAnswered { approved },
        );

        // If more requests queued up, show the next one now.
        let next = lock_pending(&session.pending)
            .front()
            .map(|p| p.description.clone());
        if let Some(description) = next {
            Self::emit_approval(&self.event_tx, session_id, &description);
        }
        Ok(())
    }

    async fn cancel(&self, session_id: &str) -> Result<(), ProviderError> {
        let removed = self.sessions.lock().await.remove(session_id);
        let mut session =
            removed.ok_or_else(|| ProviderError::SessionNotFound(session_id.to_string()))?;

        // Mark first so the reader reports `Cancelled` rather than `Failed`.
        session.cancelled.store(true, Ordering::SeqCst);
        // Release any blocked permission hook before stopping the agent.
        let _ = std::fs::write(session.dir.join(".closed"), b"");
        let _ = session.writer.write_all(&[0x03]); // Ctrl+C
        let _ = session.writer.flush();

        tokio::time::sleep(CANCEL_GRACE).await;
        if !session.exited.load(Ordering::SeqCst)
            && let Err(e) = session.killer.kill()
        {
            warn!("Could not kill session {}: {}", session_id, e);
        }
        info!("Cancelled session {}", session_id);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_descriptions_use_the_fields_claude_sent() {
        assert_eq!(
            describe_tool("Write", &json!({"file_path": "/w/a.txt", "content": "x"})),
            "Write /w/a.txt"
        );
        assert_eq!(
            describe_tool("Bash", &json!({"command": "rm -rf build"})),
            "Bash: rm -rf build"
        );
        assert_eq!(describe_tool("WebFetch", &json!({})), "WebFetch");
    }

    #[test]
    fn shell_quoting_survives_spaces_and_quotes() {
        assert_eq!(sh_quote("/a b/c"), "'/a b/c'");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn trust_prompt_is_found_through_escape_codes_and_missing_spaces() {
        let raw = "\x1b[1mYes,\x1b[0m\x1b[1CI\x1b[1Ctrust\x1b[1Cthis\x1b[1Cfolder";
        let squashed: String = strip_ansi_escapes::strip_str(raw)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(squashed.contains(TRUST_MARKER));
    }
}
