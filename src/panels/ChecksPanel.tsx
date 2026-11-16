import React, { useState } from 'react';
import { CheckReport, DetectedChecks, Project } from '../types';
import { StatusChip } from './StatusChip';

interface ChecksPanelProps {
  project: Project;
  taskId: string | null;
  reports: CheckReport[];
  onSave: (build: string, test: string, lint: string) => void;
  onRun: (taskId: string) => void;
  onDetect: () => Promise<DetectedChecks | null>;
  autoRun: boolean;
  onAutoRunChange: (value: boolean) => void;
}

const KINDS = ['build', 'test', 'lint'] as const;
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** Verification commands and the latest results (DESIGN §6.6). */
export const ChecksPanel: React.FC<ChecksPanelProps> = ({
  project,
  taskId,
  reports,
  onSave,
  onRun,
  onDetect,
  autoRun,
  onAutoRunChange,
}) => {
  const [detectNote, setDetectNote] = useState<string | null>(null);
  const [cmds, setCmds] = useState({
    build: project.build_cmd ?? '',
    test: project.test_cmd ?? '',
    lint: project.lint_cmd ?? '',
  });

  const anySet = KINDS.some((k) => cmds[k].trim() !== '');

  return (
    <section aria-labelledby="checks-title">
      <div className="panel-head">
        <h2 id="checks-title">Checks</h2>
        <button className="btn" disabled={!taskId || !anySet} onClick={() => taskId && onRun(taskId)}>
          Run checks
        </button>
      </div>
      {!anySet && (
        <p className="muted">No checks set. Add build, test or lint commands so bosses can be defeated.</p>
      )}
      {KINDS.map((k) => (
        <label key={k} className="field" style={{ marginBottom: 'var(--space-2)' }}>
          {cap(k)} command
          <input
            className="mono"
            value={cmds[k]}
            onChange={(e) => setCmds({ ...cmds, [k]: e.target.value })}
            placeholder="Not set"
          />
        </label>
      ))}
      <div className="task-actions">
        <button className="btn" onClick={() => onSave(cmds.build, cmds.test, cmds.lint)}>
          Save checks
        </button>
        <button
          className="btn"
          onClick={async () => {
            const found = await onDetect();
            if (!found) return;
            setCmds({ build: found.build_cmd ?? '', test: found.test_cmd ?? '', lint: found.lint_cmd ?? '' });
            setDetectNote(
              found.sources.length > 0
                ? `Found in ${found.sources.join(' and ')}. Save checks to keep them.`
                : 'No build, test or lint scripts found in the project files.',
            );
          }}
        >
          Detect from project files
        </button>
      </div>
      {detectNote && <p className="muted">{detectNote}</p>}
      <label style={{ display: 'flex', gap: 'var(--space-2)', marginTop: 'var(--space-2)' }}>
        <input type="checkbox" checked={autoRun} onChange={(e) => onAutoRunChange(e.target.checked)} />
        Run checks automatically when the agent finishes
      </label>

      {reports.length > 0 && (
        <ul className="row-list" aria-label="Latest check results" style={{ marginTop: 'var(--space-3)' }}>
          {reports.map((r) => (
            <li key={r.verification_type}>
              <StatusChip status={r.passed ? 'verified' : 'failed'} />
              <span>
                {cap(r.verification_type)}
                <span className="muted mono"> {r.command}</span>
              </span>
              <span className="num muted">{r.exit_code === null ? 'No exit code' : `Exit ${r.exit_code}`}</span>
              {!r.passed && <span className="sub mono">{r.summary}</span>}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};
