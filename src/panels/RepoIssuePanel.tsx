import React from 'react';
import { RepoIssue } from '../types';

interface RepoIssuePanelProps {
  issue: RepoIssue;
  onInitialize: () => void;
  onChooseOther: () => void;
  onDismiss: () => void;
}

/** Explains why a folder cannot be opened and offers the fix, with consent. */
export const RepoIssuePanel: React.FC<RepoIssuePanelProps> = ({ issue, onInitialize, onChooseOther, onDismiss }) => {
  const { kind } = issue.report;
  const canInitialize = kind === 'not_a_repo' || kind === 'no_commits';

  return (
    <section className="notice" aria-labelledby="repo-issue-title">
      <h2 id="repo-issue-title">This folder is not ready for quests</h2>
      <p className="mono" style={{ wordBreak: 'break-all' }}>
        {issue.path}
      </p>
      <p>{issue.report.problem}</p>
      {canInitialize && (
        <>
          <p className="muted">Initialize git here will:</p>
          <ul className="muted">
            {kind === 'not_a_repo' && <li>Run git init in this folder.</li>}
            <li>Add a .gitignore (node_modules, target, dist) only if the folder has none.</li>
            <li>Commit the files of this folder as the first commit. Files your .gitignore lists are skipped.</li>
          </ul>
        </>
      )}
      <div className="dialog-actions">
        <button className="btn" onClick={onDismiss}>
          Dismiss
        </button>
        <button className="btn" onClick={onChooseOther}>
          Choose another folder
        </button>
        {canInitialize && (
          <button className="btn btn-primary" onClick={onInitialize}>
            Initialize git here
          </button>
        )}
      </div>
    </section>
  );
};
