import React from 'react';
import { BossHp } from '../types';
import { formatHp } from '../state/status';

const Check: React.FC = () => (
  <svg viewBox="0 0 12 12" aria-hidden="true" focusable="false">
    <path d="M2 6.4 4.8 9 10 3" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

const CHECK_TEXT: Record<BossHp['build']['state'], string> = {
  passed: 'passed',
  failed: 'failed',
  not_run: 'not run yet',
  not_set: 'not set',
};

interface BossBarProps {
  name: string;
  hp: BossHp | null;
}

export const BossBar: React.FC<BossBarProps> = ({ name, hp }) => {
  if (!hp) {
    return (
      <div>
        <div className="boss-bar-head">
          <span className="display">{name}</span>
        </div>
        <p className="muted">No boss HP yet. Select a quest to see its encounter.</p>
      </div>
    );
  }

  const checks = [
    { key: 'build', name: 'Build', seg: hp.build },
    { key: 'test', name: 'Test', seg: hp.test },
    { key: 'lint', name: 'Lint', seg: hp.lint },
  ] as const;

  return (
    <div>
      <div className="boss-bar-head">
        <span className="display">{name}</span>
        <span className="num">HP {formatHp(hp.remaining_bp)}</span>
      </div>

      <div className={`boss-bar${hp.defeated ? ' defeated' : ''}`} role="group" aria-label={`${name} health by evidence`}>
        {hp.milestones.map((m, i) => (
          <div
            key={`m${i}`}
            className="boss-seg"
            data-proven={m.proven}
            style={{ '--w': m.weight_bp } as React.CSSProperties}
            title={`Milestone ${i + 1}: ${m.proven ? 'done' : 'not done'}`}
          >
            {m.proven && <Check />}
          </div>
        ))}
        {checks.map(({ key, name: checkName, seg }) => (
          <div
            key={key}
            className="boss-seg"
            data-proven={seg.state === 'passed'}
            style={{ '--w': seg.weight_bp } as React.CSSProperties}
            title={`${checkName}: ${CHECK_TEXT[seg.state]}`}
          >
            {seg.state === 'passed' && <Check />}
          </div>
        ))}
      </div>

      <p className="boss-label">{hp.label}</p>
      <p className="muted">{hp.next_step}</p>
      <ul className="sr-only">
        {hp.milestones.map((m, i) => (
          <li key={i}>
            Milestone {i + 1} of {hp.milestones_total}: {m.proven ? 'done' : 'not done'}
          </li>
        ))}
        {checks.map(({ key, name: checkName, seg }) => (
          <li key={key}>
            {checkName}: {CHECK_TEXT[seg.state]}
          </li>
        ))}
      </ul>
      {hp.defeated && (
        <p className="boss-defeated" role="status">
          Boss defeated: build, test and lint passed
        </p>
      )}
    </div>
  );
};
