import React from 'react';
import { Character } from '../types';
import { Shield, Sparkles, Footprints, Bot } from 'lucide-react';

interface PartyPanelProps {
  characters: Character[];
}

export const PartyPanel: React.FC<PartyPanelProps> = ({ characters }) => {
  const getClassIcon = (className: string) => {
    switch (className.toLowerCase()) {
      case 'warrior':
        return <Shield size={16} color="var(--gold-primary)" />;
      case 'mage':
        return <Sparkles size={16} color="var(--mana-cyan)" />;
      case 'rogue':
        return <Footprints size={16} color="var(--emerald-success)" />;
      default:
        return <Bot size={16} color="var(--purple-arcane)" />;
    }
  };

  const getStateBadge = (state: string) => {
    switch (state) {
      case 'exploring':
        return <span className="badge badge-cyan">Exploring</span>;
      case 'fighting':
        return <span className="badge badge-ruby glow-pulse">Fighting</span>;
      case 'needs_approval':
        return <span className="badge badge-gold glow-pulse">Approval Req.</span>;
      case 'succeeded':
        return <span className="badge badge-emerald">Victorious</span>;
      default:
        return <span className="badge" style={{ background: 'rgba(255,255,255,0.08)' }}>Idle</span>;
    }
  };

  return (
    <div className="glass-panel" style={{ padding: '16px', display: 'flex', flexDirection: 'column' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '14px' }}>
        <Bot size={20} color="var(--mana-cyan)" />
        <h2 className="title-rpg" style={{ fontSize: '1.1rem' }}>Adventurer Party</h2>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '12px' }}>
        {characters.map((char) => (
          <div
            key={char.id}
            style={{
              background: 'rgba(255, 255, 255, 0.03)',
              border: '1px solid var(--border-subtle)',
              borderRadius: '8px',
              padding: '12px',
              display: 'flex',
              flexDirection: 'column',
              gap: '6px',
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                {getClassIcon(char.class)}
                <span style={{ fontWeight: 'bold', fontSize: '0.9rem' }}>{char.display_name}</span>
              </div>
              {getStateBadge(char.state)}
            </div>

            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--text-muted)' }}>
              <span>Class: {char.class.toUpperCase()}</span>
              <span>Role: {char.party_role.toUpperCase()}</span>
            </div>

            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: 'var(--text-faint)', borderTop: '1px solid var(--border-subtle)', paddingTop: '6px' }}>
              <span>Level {char.level}</span>
              <span>Engine: Claude Code</span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};
