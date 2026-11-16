-- State machine support: failure reasons, transition audit trail, and a
-- database-level guard against two live sessions on the same task.

ALTER TABLE tasks ADD COLUMN failure_reason TEXT;

CREATE TABLE IF NOT EXISTS task_transitions (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_status TEXT NOT NULL,
    to_status TEXT NOT NULL,
    reason TEXT,
    actor TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_task_transitions_task ON task_transitions(task_id, created_at);

-- Sessions written by the pre-state-machine code can never have a live process
-- after an upgrade, so close them before adding the unique guard.
UPDATE agent_sessions
SET status = 'failed', ended_at = CURRENT_TIMESTAMP
WHERE status IN ('preparing', 'running', 'waiting', 'waiting_approval');

CREATE UNIQUE INDEX IF NOT EXISTS idx_one_live_session_per_task
    ON agent_sessions(task_id)
    WHERE status IN ('preparing', 'running', 'waiting', 'waiting_approval');
