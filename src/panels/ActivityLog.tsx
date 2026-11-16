import React, { useEffect, useRef } from 'react';
import { NormalizedEvent } from '../types';
import { Terminal } from 'lucide-react';

interface ActivityLogProps {
  events: NormalizedEvent[];
}

export const ActivityLog: React.FC<ActivityLogProps> = ({ events }) => {
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [events]);

  const getEventColor = (state: string) => {
    switch (state) {
      case 'exploring':
        return 'var(--mana-cyan)';
      case 'fighting':
        return '#fb7185';
      case 'needs_approval':
        return 'var(--gold-primary)';
      case 'succeeded':
        return 'var(--emerald-success)';
      case 'failed':
        return 'var(--ruby-hp)';
      default:
        return 'var(--text-muted)';
    }
  };

  const formatPayload = (event: NormalizedEvent) => {
    const { payload } = event;
    if (payload.type === 'file_read') {
      return `[READ] ${payload.data?.path || 'file'}`;
    }
    if (payload.type === 'file_changed') {
      return `[WRITE] ${payload.data?.path || 'modified code'}`;
    }
    if (payload.type === 'approval_required') {
      return `[APPROVAL REQUIRED] ${payload.data?.action_description || 'Risky action requested'}`;
    }
    if (payload.type === 'session_output') {
      return payload.data?.text || '';
    }
    return JSON.stringify(payload.data || payload);
  };

  return (
    <div
      className="glass-panel"
      style={{
        padding: '16px',
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        minHeight: '200px',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '10px' }}>
        <Terminal size={18} color="var(--gold-primary)" />
        <h3 className="title-rpg" style={{ fontSize: '0.95rem' }}>Dungeon Combat & Agent Log</h3>
      </div>

      <div
        style={{
          flex: 1,
          overflowY: 'auto',
          background: 'rgba(0, 0, 0, 0.4)',
          borderRadius: '6px',
          padding: '10px',
          fontFamily: 'var(--font-mono)',
          fontSize: '0.78rem',
          lineHeight: '1.4',
          display: 'flex',
          flexDirection: 'column',
          gap: '4px',
        }}
      >
        {events.length === 0 ? (
          <div style={{ color: 'var(--text-faint)', fontStyle: 'italic' }}>
            Awaiting party deployment. Launch a task from the Quest Board to begin the raid.
          </div>
        ) : (
          events.map((ev, idx) => (
            <div key={idx} style={{ display: 'flex', gap: '8px', wordBreak: 'break-all' }}>
              <span style={{ color: 'var(--text-faint)' }}>
                {new Date(ev.timestamp).toLocaleTimeString()}
              </span>
              <span
                style={{
                  color: getEventColor(ev.presentation_state),
                  fontWeight: '600',
                  minWidth: '110px',
                }}
              >
                [{ev.presentation_state.toUpperCase()}]
              </span>
              <span style={{ color: 'var(--text-main)' }}>{formatPayload(ev)}</span>
            </div>
          ))
        )}
        <div ref={endRef} />
      </div>
    </div>
  );
};
