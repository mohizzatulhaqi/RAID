import React from 'react';
import { Character } from '../types';
import { statusFromPresentation } from '../state/status';
import { StatusChip } from './StatusChip';

interface PartyPanelProps {
  characters: Character[];
}

// Class is a visual identity and role is the expected contribution; neither
// claims a capability of the agent (DESIGN §5.5).
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export const PartyPanel: React.FC<PartyPanelProps> = ({ characters }) => (
  <section aria-labelledby="party-title">
    <div className="panel-head">
      <h2 id="party-title">Party</h2>
    </div>
    {characters.length === 0 && <p className="muted">No party members yet.</p>}
    {characters.map((c) => (
      <div key={c.id} className="party-card">
        <div className="panel-head" style={{ marginBottom: 'var(--space-1)' }}>
          <strong>{c.display_name}</strong>
          <StatusChip status={statusFromPresentation(c.state)} />
        </div>
        <div className="meta">
          <span>
            Class {cap(c.class)}, role {cap(c.party_role)}
          </span>
          <span className="num">Level {c.level}</span>
        </div>
        <div className="meta">
          <span>Claude Code</span>
          <span>No activity yet</span>
        </div>
      </div>
    ))}
  </section>
);
