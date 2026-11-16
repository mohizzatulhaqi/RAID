import { NormalizedEvent } from '../types';

export function isErrorEvent(ev: NormalizedEvent): boolean {
  const p = ev.payload;
  return (
    ev.presentation_state === 'failed' ||
    p.type === 'session_failed' ||
    (p.type === 'verification_completed' && !p.data.success) ||
    (p.type === 'command_completed' && p.data.exit_code !== 0)
  );
}

export function describeEvent(ev: NormalizedEvent): string {
  const p = ev.payload;
  switch (p.type) {
    case 'file_read':
      return `Read ${p.data.path}`;
    case 'file_changed':
      return `Changed ${p.data.path}`;
    case 'command_started':
      return `Started ${p.data.command}`;
    case 'command_completed':
      return `Finished ${p.data.command} with exit code ${p.data.exit_code}`;
    case 'approval_required':
      return `Approval needed: ${p.data.action_description}`;
    case 'approval_answered':
      return p.data.approved ? 'You approved the request' : 'You denied the request';
    case 'verification_started':
      return `Started ${p.data.verification_type} check: ${p.data.command}`;
    case 'verification_completed':
      return `${p.data.verification_type} check ${p.data.success ? 'passed' : 'failed'} (exit code ${p.data.exit_code}). ${p.data.summary}`;
    case 'session_started':
      return 'Session started';
    case 'session_succeeded':
      return `Session finished, not verified. ${p.data.summary}`;
    case 'session_failed':
      return `Session failed: ${p.data.error}`;
    case 'session_cancelled':
      return 'Session cancelled';
    case 'session_output':
      return p.data.text;
  }
}
