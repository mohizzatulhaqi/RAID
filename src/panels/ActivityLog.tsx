import React, { useEffect, useMemo, useRef, useState } from 'react';
import { NormalizedEvent } from '../types';
import { statusFromPresentation } from '../state/status';
import { describeEvent, isErrorEvent } from '../state/eventText';
import { StatusChip } from './StatusChip';

type Filter = 'all' | 'events' | 'errors';

export const ActivityLog: React.FC<{ events: NormalizedEvent[] }> = ({ events }) => {
  const [filter, setFilter] = useState<Filter>('all');
  const [pinned, setPinned] = useState(true);
  const bodyRef = useRef<HTMLDivElement>(null);

  const shown = useMemo(
    () =>
      events.filter((e) =>
        filter === 'all' ? true : filter === 'errors' ? isErrorEvent(e) : e.payload.type !== 'session_output',
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
              <span>{describeEvent(ev)}</span>
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
