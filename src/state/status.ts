import { Character, PresentationState, StatusKind, Task } from '../types';

/** Presentation state (from normalized events) -> status chip. */
export function statusFromPresentation(state: PresentationState | string): StatusKind {
  switch (state) {
    case 'entering':
    case 'preparing':
      return 'preparing';
    case 'assigned':
      return 'assigned';
    case 'exploring':
    case 'planning':
    case 'fighting':
    case 'investigating':
    case 'returning':
    case 'running':
      return 'running';
    case 'waiting':
      return 'waiting';
    case 'needs_approval':
      return 'needs_approval';
    case 'verification':
      return 'verifying';
    // A finished session is not a verified task (CLAUDE.md principle 4).
    case 'succeeded':
      return 'finished_unverified';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    default:
      return 'idle';
  }
}

/** Task status (from the backend) -> status chip. Only `completed` is Verified. */
export function statusFromTask(status: Task['status']): StatusKind {
  switch (status) {
    case 'assigned':
      return 'assigned';
    case 'running':
      return 'running';
    case 'verification':
      return 'verifying';
    case 'completed':
      return 'verified';
    case 'needs_review':
      return 'needs_review';
    case 'blocked':
      return 'waiting';
    case 'awaiting_approval':
      return 'needs_approval';
    case 'failed':
      return 'failed';
    case 'cancelled':
      return 'cancelled';
    default:
      return 'idle';
  }
}

/** HP in basis points as text, e.g. 5500 -> "55%", 3333 -> "33.33%". */
export function formatHp(bp: number): string {
  const pct = bp / 100;
  return `${Number.isInteger(pct) ? pct : pct.toFixed(2).replace(/0+$/, '')}%`;
}

const SCENE_LABEL: Record<string, string> = {
  entering: 'Entering',
  exploring: 'Exploring',
  planning: 'Planning',
  fighting: 'Fighting',
  investigating: 'Investigating',
  needs_approval: 'Needs approval',
  verification: 'Returning',
  returning: 'Returning',
};

/** World label for a character: the latest real event while a session is live. */
export function sceneLabel(char: Character): string | null {
  if (!char.active_session_id || !char.last_event) return null;
  return SCENE_LABEL[char.last_event.presentation_state] ?? null;
}
