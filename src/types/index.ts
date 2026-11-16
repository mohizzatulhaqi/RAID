export type PresentationState =
  | 'idle'
  | 'entering'
  | 'exploring'
  | 'planning'
  | 'fighting'
  | 'investigating'
  | 'needs_approval'
  | 'verification'
  | 'returning'
  | 'succeeded'
  | 'failed'
  | 'cancelled';

export interface Project {
  id: string;
  name: string;
  repository_path: string;
  default_branch: string;
  build_cmd?: string;
  test_cmd?: string;
  lint_cmd?: string;
  created_at: string;
  updated_at: string;
}

export interface Quest {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: 'created' | 'ready' | 'active' | 'completed' | 'failed';
  priority: 'low' | 'medium' | 'high' | 'urgent';
  boss_name: string;
  boss_max_hp: number;
  boss_current_hp: number;
  created_at: string;
  completed_at?: string;
}

export interface Task {
  id: string;
  quest_id: string;
  parent_task_id?: string;
  title: string;
  description: string;
  status: 'created' | 'ready' | 'assigned' | 'running' | 'verification' | 'completed' | 'blocked' | 'awaiting_approval' | 'failed' | 'cancelled' | 'needs_review';
  assigned_character_id?: string;
  priority: 'low' | 'medium' | 'high' | 'urgent';
  weight_percent: number;
  created_at: string;
  updated_at: string;
  failure_reason: string | null;
  active_session_id: string | null;
  /** What the backend allows for this task right now. */
  allowed_actions: TaskAction[];
  depends_on: { id: string; title: string; status: Task['status'] }[];
}

export type TaskAction = 'start' | 'retry' | 'cancel' | 'run_checks' | 'mark_verified';

export interface Character {
  id: string;
  display_name: string;
  class: 'warrior' | 'mage' | 'rogue' | 'cleric' | 'ranger' | 'paladin' | 'engineer' | 'bard' | 'summoner';
  party_role: 'tank' | 'dps' | 'healer' | 'support' | 'controller' | 'scout' | 'utility';
  level: number;
  experience: number;
  appearance_config: string;
  /** Derived by the backend from the live session, never stored by the UI. */
  state: string;
  created_at: string;
  task_id: string | null;
  task_title: string | null;
  active_session_id: string | null;
  last_event: NormalizedEvent | null;
}

/** Mirrors the Rust `EventPayload` enum (`events/mod.rs`). */
export type EventPayload =
  | { type: 'session_started'; data: { session_id: string; task_id: string } }
  | { type: 'session_output'; data: { text: string } }
  | { type: 'file_read'; data: { path: string } }
  | { type: 'file_changed'; data: { path: string } }
  | { type: 'command_started'; data: { command: string } }
  | { type: 'command_completed'; data: { command: string; exit_code: number } }
  | { type: 'approval_required'; data: { action_description: string; is_risky: boolean } }
  | { type: 'approval_answered'; data: { approved: boolean } }
  | { type: 'verification_started'; data: { verification_type: string; command: string } }
  | {
      type: 'verification_completed';
      data: { verification_type: string; success: boolean; exit_code: number; summary: string };
    }
  | { type: 'session_succeeded'; data: { summary: string } }
  | { type: 'session_failed'; data: { error: string } }
  | { type: 'session_cancelled' };

export interface NormalizedEvent {
  session_id: string;
  timestamp: string;
  presentation_state: PresentationState;
  payload: EventPayload;
}

/** Status kinds of the shared status chip (DESIGN §6.1). */
export type StatusKind =
  | 'idle'
  | 'assigned'
  | 'preparing'
  | 'running'
  | 'waiting'
  | 'needs_approval'
  | 'verifying'
  | 'finished_unverified'
  | 'verified'
  | 'needs_review'
  | 'failed'
  | 'cancelled';

export type CheckState = 'passed' | 'failed' | 'not_run' | 'not_set';

/** Mirrors the Rust `BossHp` struct. All values are computed by the backend. */
export interface BossHp {
  remaining_bp: number;
  milestone_bp: number;
  verification_bp: number;
  defeated: boolean;
  milestones_done: number;
  milestones_total: number;
  milestones: { weight_bp: number; proven: boolean }[];
  build: { weight_bp: number; state: CheckState };
  test: { weight_bp: number; state: CheckState };
  lint: { weight_bp: number; state: CheckState };
  label: string;
  next_step: string;
}

export interface CheckReport {
  verification_type: 'build' | 'test' | 'lint';
  command: string;
  passed: boolean;
  exit_code: number | null;
  summary: string;
}

/** Result of checking a folder before opening it (mirrors Rust `RepoReport`). */
export type RepoReport = (
  | { kind: 'ready'; branch: string }
  | { kind: 'no_commits' }
  | { kind: 'not_a_repo' }
  | { kind: 'not_repo_root'; root: string }
  | { kind: 'missing' }
) & { problem: string | null };

export interface RepoIssue {
  path: string;
  report: RepoReport;
}

export interface DetectedChecks {
  build_cmd: string | null;
  test_cmd: string | null;
  lint_cmd: string | null;
  sources: string[];
}

export interface FileChange {
  path: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed';
  additions: number | null;
  deletions: number | null;
}

export interface WorktreeChanges {
  files: FileChange[];
  patch: string;
  truncated: boolean;
}

export interface TaskDetail {
  events: NormalizedEvent[];
  transitions: {
    from_status: string;
    to_status: string;
    reason: string | null;
    actor: string;
    created_at: string;
  }[];
  sessions: {
    id: string;
    provider: string;
    status: string;
    worktree_path: string | null;
    branch_name: string | null;
    started_at: string;
    ended_at: string | null;
    exit_code: number | null;
  }[];
  runs: {
    id: string;
    verification_type: string;
    command: string;
    status: string;
    exit_code: number | null;
    result_summary: string | null;
    started_at: string;
    ended_at: string | null;
  }[];
}
