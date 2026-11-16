import React, { useEffect, useMemo, useRef, useState } from 'react';
import { NormalizedEvent } from '../types';
import { statusFromPresentation } from '../state/status';
import { StatusChip } from './StatusChip';

type Filter = 'all' | 'events' | 'errors';

function isError(ev: NormalizedEvent): boolean {
  const p = ev.payload;
  return (
    ev.presentation_state === 'failed' ||
    p.type === 'session_failed' ||
    (p.type === 'verification_completed' && !p.data.success) ||
    (p.type === 'command_completed' && p.data.exit_code !== 0)
  );
}

function describe(ev: NormalizedEvent): string {
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

export const ActivityLog: React.FC<{ events: NormalizedEvent[] }> = ({ events }) => {
  const [filter, setFilter] = useState<Filter>('all');
  const [pinned, setPinned] = useState(true);
  const bodyRef = useRef<HTMLDivElement>(null);

  const shown = useMemo(
    () =>
      events.filter((e) =>
        filter === 'all' ? true : filter === 'errors' ? isError(e) : e.payload.type !== 'session_output',
      ),
    [events, filter],
  );

  useEffect(() => {
    const el = bodyRef.current;
    if (pinned && el) el.scrollTop = el.scrollHeight;
  }, [shown, pinned]);

  const onScroll = () => {
    const el = bodyRef.current;
    if (el) setPinned(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
  };

  const jump = () => {
    const el = bodyRef.current;
    if (el) el.scrollTop = el.scrollHeight;
    setPinned(true);
  };

  return (
    <>
      <div className="log-head">
        <h2>Activity log</h2>
        <div role="group" aria-label="Filter log" style={{ display: 'flex', gap: 'var(--space-1)' }}>
          {(['all', 'events', 'errors'] as const).map((f) => (
            <button key={f} className="btn" aria-pressed={filter === f} onClick={() => setFilter(f)}>
              {f === 'all' ? 'All' : f === 'events' ? 'Events' : 'Errors'}
            </button>
          ))}
        </div>
      </div>
      <div className="log-body mono" ref={bodyRef} onScroll={onScroll}>
        {shown.length === 0 ? (
          <span className="muted">No activity yet. Start a task from the Quests panel.</span>
        ) : (
          shown.map((ev, i) => (
            <div key={i} className="log-line">
              <time className="num">{new Date(ev.timestamp).toLocaleTimeString()}</time>
              <StatusChip status={statusFromPresentation(ev.presentation_state)} />
              <span>{describe(ev)}</span>
            </div>
          ))
        )}
        {!pinned && (
          <button className="btn jump-latest" onClick={jump}>
            Jump to latest
          </button>
        )}
      </div>
    </>
  );
};
