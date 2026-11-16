import React, { useState } from 'react';
import { Task, TaskDetail, WorktreeChanges } from '../types';
import { statusFromPresentation, statusFromTask } from '../state/status';
import { describeEvent } from '../state/eventText';
import { StatusChip } from './StatusChip';

type Tab = 'summary' | 'changes' | 'checks' | 'history';

interface TaskDetailPanelProps {
  task: Task;
  detail: TaskDetail | null;
  /** undefined: not loaded yet; null: the quest has no worktree yet. */
  changes: WorktreeChanges | null | undefined;
  onLoadChanges: () => void;
}

const TABS: { id: Tab; label: string }[] = [
  { id: 'summary', label: 'Summary' },
  { id: 'changes', label: 'Changes' },
  { id: 'checks', label: 'Checks' },
  { id: 'history', label: 'History' },
];

const CHANGE_LABEL = { added: 'Added', modified: 'Modified', deleted: 'Deleted', renamed: 'Renamed' } as const;

const time = (iso: string) => new Date(iso).toLocaleString();

function duration(start: string, end: string | null): string {
  if (!end) return 'Still running';
  const s = Math.max(0, (new Date(end).getTime() - new Date(start).getTime()) / 1000);
  return s < 60 ? `${s.toFixed(1)} s` : `${Math.floor(s / 60)} min ${Math.round(s % 60)} s`;
}

function patchFor(patch: string, path: string): string {
  const chunks = patch.split(/^(?=diff --git )/m);
  return chunks.find((c) => (c.split('\n')[0] ?? '').endsWith(` b/${path}`)) ?? '';
}

const Diff: React.FC<{ text: string }> = ({ text }) => (
  <pre className="diff mono">
    {text.split('\n').map((line, i) => {
      const kind = line.startsWith('+') && !line.startsWith('+++')
        ? 'add'
        : line.startsWith('-') && !line.startsWith('---')
          ? 'del'
          : line.startsWith('@@')
            ? 'hunk'
            : 'ctx';
      return (
        <span key={i} className={`diff-line diff-${kind}`}>
          {line || ' '}
          {'\n'}
        </span>
      );
    })}
  </pre>
);

export const TaskDetailPanel: React.FC<TaskDetailPanelProps> = ({ task, detail, changes, onLoadChanges }) => {
  const [tab, setTab] = useState<Tab>('summary');
  const [openFile, setOpenFile] = useState<string | null>(null);
  const [openRun, setOpenRun] = useState<string | null>(null);
  const session = detail?.sessions[0];

  const selectTab = (next: Tab) => {
    setTab(next);
    if (next === 'changes') onLoadChanges();
  };

  return (
    <section aria-labelledby="task-detail-title">
      <div className="panel-head">
        <h2 id="task-detail-title">Task detail</h2>
        <StatusChip status={statusFromTask(task.status)} />
      </div>
      <div className="tabs" role="tablist" aria-label="Task detail sections">
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            aria-selected={tab === t.id}
            className="btn tab"
            onClick={() => selectTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'summary' && (
        <div className="tab-body">
          <strong>{task.title}</strong>
          <p className="muted" style={{ maxWidth: '72ch' }}>
            {task.description}
          </p>
          {task.failure_reason && <p>{task.failure_reason}</p>}
          {task.status === 'verification' && <p>Finished, not verified. Run checks to confirm.</p>}
          {session ? (
            <dl className="facts">
              <dt>Branch</dt>
              <dd className="mono">{session.branch_name ?? 'Unknown'}</dd>
              <dt>Worktree</dt>
              <dd className="mono">{session.worktree_path ?? 'Unknown'}</dd>
              <dt>Last session</dt>
              <dd>
                {session.status}, started {time(session.started_at)}
              </dd>
            </dl>
          ) : (
            <p className="muted">This task has not run yet.</p>
          )}
        </div>
      )}

      {tab === 'changes' && (
        <div className="tab-body">
          <div className="panel-head">
            <span className="muted">Compared with the branch the quest started from</span>
            <button className="btn" onClick={onLoadChanges}>
              Refresh
            </button>
          </div>
          {changes === undefined && <p className="muted">Loading changes.</p>}
          {changes === null && <p className="muted">No worktree yet. Start the task to see its changes.</p>}
          {changes && changes.files.length === 0 && <p className="muted">No changes in the worktree.</p>}
          {changes && changes.files.length > 0 && (
            <ul className="row-list" aria-label="Changed files">
              {changes.files.map((f) => (
                <li key={f.path} style={{ gridTemplateColumns: '1fr auto' }}>
                  <button
                    className="quest-item mono"
                    aria-expanded={openFile === f.path}
                    onClick={() => setOpenFile(openFile === f.path ? null : f.path)}
                  >
                    {f.path}
                  </button>
                  <span className="num muted">
                    {CHANGE_LABEL[f.status]}
                    {f.additions !== null && ` +${f.additions}`}
                    {f.deletions !== null && ` -${f.deletions}`}
                    {f.additions === null && ' (binary or large)'}
                  </span>
                  {openFile === f.path && (
                    <div className="sub" style={{ gridColumn: '1 / 3', maxWidth: 'none' }}>
                      <Diff text={patchFor(changes.patch, f.path) || 'No text diff for this file.'} />
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
          {changes?.truncated && (
            <p className="muted">The change list or diff was too large and is shortened.</p>
          )}
        </div>
      )}

      {tab === 'checks' && (
        <div className="tab-body">
          {!detail || detail.runs.length === 0 ? (
            <p className="muted">No checks have run for this task.</p>
          ) : (
            <ul className="row-list" aria-label="Check runs">
              {detail.runs.map((r) => (
                <li key={r.id}>
                  <StatusChip
                    status={r.status === 'passed' ? 'verified' : r.status === 'failed' ? 'failed' : 'verifying'}
                  />
                  <span>
                    {r.verification_type} <span className="mono muted">{r.command}</span>
                  </span>
                  <span className="num muted">
                    {r.exit_code === null ? 'No exit code' : `Exit ${r.exit_code}`}, {duration(r.started_at, r.ended_at)}
                  </span>
                  <span className="sub">
                    {time(r.started_at)}{' '}
                    {r.result_summary && (
                      <button className="btn" onClick={() => setOpenRun(openRun === r.id ? null : r.id)}>
                        {openRun === r.id ? 'Hide output' : 'Open output'}
                      </button>
                    )}
                  </span>
                  {openRun === r.id && r.result_summary && (
                    <pre className="sub diff mono" style={{ maxWidth: 'none' }}>
                      {r.result_summary}
                    </pre>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>
      )}

      {tab === 'history' && (
        <div className="tab-body">
          {!detail || detail.transitions.length === 0 ? (
            <p className="muted">No status changes recorded yet.</p>
          ) : (
            <ol className="history" aria-label="Status changes">
              {detail.transitions.map((t, i) => (
                <li key={i}>
                  <time className="num muted">{time(t.created_at)}</time>
                  <span>
                    {t.from_status} to {t.to_status}
                    <span className="muted"> by {t.actor}</span>
                  </span>
                  {t.reason && <span className="muted">{t.reason}</span>}
                </li>
              ))}
            </ol>
          )}
          {detail && detail.events.length > 0 && (
            <>
              <h3 className="history-head">Activity</h3>
              <ol className="history" aria-label="Agent activity">
                {detail.events.map((ev, i) => (
                  <li key={i}>
                    <span className="row-inline">
                      <time className="num muted">{time(ev.timestamp)}</time>
                      <StatusChip status={statusFromPresentation(ev.presentation_state)} />
                    </span>
                    <span className="mono" style={{ wordBreak: 'break-word' }}>
                      {describeEvent(ev)}
                    </span>
                  </li>
                ))}
              </ol>
            </>
          )}
          {detail && detail.sessions.length > 0 && (
            <>
              <h3 className="history-head">Sessions</h3>
              <ol className="history" aria-label="Agent sessions">
                {detail.sessions.map((sn) => (
                  <li key={sn.id}>
                    <time className="num muted">{time(sn.started_at)}</time>
                    <span>
                      {sn.provider}, {sn.status}
                      <span className="muted"> {duration(sn.started_at, sn.ended_at)}</span>
                    </span>
                  </li>
                ))}
              </ol>
            </>
          )}
        </div>
      )}
    </section>
  );
};
