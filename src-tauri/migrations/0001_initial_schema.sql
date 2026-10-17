-- RAID Initial SQLite Schema Migration
-- Matches PRD Section 11 & Section 18 Locked Architecture

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    repository_path TEXT NOT NULL UNIQUE,
    default_branch TEXT NOT NULL DEFAULT 'main',
    build_cmd TEXT,
    test_cmd TEXT,
    lint_cmd TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS quests (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'created', -- 'created', 'ready', 'active', 'completed', 'failed'
    priority TEXT NOT NULL DEFAULT 'medium', -- 'low', 'medium', 'high', 'urgent'
    boss_name TEXT,
    boss_max_hp INTEGER NOT NULL DEFAULT 100,
    boss_current_hp INTEGER NOT NULL DEFAULT 100,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    quest_id TEXT NOT NULL REFERENCES quests(id) ON DELETE CASCADE,
    parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'created', -- 'created', 'ready', 'assigned', 'running', 'verification', 'completed', 'blocked', 'awaiting_approval', 'failed', 'cancelled'
    assigned_character_id TEXT,
    priority TEXT NOT NULL DEFAULT 'medium',
    weight_percent REAL NOT NULL DEFAULT 0.0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on_task_id)
);

CREATE TABLE IF NOT EXISTS characters (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    class TEXT NOT NULL, -- 'warrior', 'mage', 'rogue', 'cleric', 'ranger', 'paladin', 'engineer', 'bard', 'summoner'
    party_role TEXT NOT NULL, -- 'tank', 'dps', 'healer', 'support', 'controller', 'scout', 'utility'
    level INTEGER NOT NULL DEFAULT 1,
    experience INTEGER NOT NULL DEFAULT 0,
    appearance_config TEXT NOT NULL DEFAULT '{}',
    state TEXT NOT NULL DEFAULT 'idle', -- 'idle', 'assigned', 'preparing', 'running', 'waiting', 'needs_approval', 'succeeded', 'failed', 'cancelled'
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS agent_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'claude-code',
    model TEXT NOT NULL DEFAULT 'claude-3-7-sonnet',
    process_identifier INTEGER,
    worktree_path TEXT,
    branch_name TEXT,
    status TEXT NOT NULL DEFAULT 'preparing', -- 'preparing', 'running', 'waiting_approval', 'succeeded', 'failed', 'cancelled'
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ended_at DATETIME,
    exit_code INTEGER
);

CREATE TABLE IF NOT EXISTS agent_events (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    payload TEXT NOT NULL DEFAULT '{}',
    source TEXT NOT NULL DEFAULT 'agent'
);

CREATE TABLE IF NOT EXISTS verification_runs (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    verification_type TEXT NOT NULL, -- 'build', 'test', 'lint', 'custom'
    command TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- 'pending', 'running', 'passed', 'failed'
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ended_at DATETIME,
    exit_code INTEGER,
    result_summary TEXT
);

CREATE TABLE IF NOT EXISTS world_states (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scene_id TEXT NOT NULL DEFAULT 'guild_hub',
    camera_state TEXT NOT NULL DEFAULT '{"x":0,"y":0,"zoom":1.0}',
    character_positions TEXT NOT NULL DEFAULT '{}',
    visual_preferences TEXT NOT NULL DEFAULT '{}',
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
