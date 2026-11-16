import React, { useState } from 'react';
import { Task } from '../types';
import { PriorityField } from './PriorityField';

export interface TaskFields {
  title: string;
  description: string;
  priority: string;
  parentTaskId: string | null;
  dependsOn: string[];
}

interface TaskDialogProps {
  mode: 'create' | 'edit';
  initial: TaskFields;
  /** Other tasks of the same quest, offered as parents and prerequisites. */
  candidates: Task[];
  onSubmit: (fields: TaskFields) => void;
  onClose: () => void;
}

export const TaskDialog: React.FC<TaskDialogProps> = ({ mode, initial, candidates, onSubmit, onClose }) => {
  const [fields, setFields] = useState(initial);
  const action = mode === 'create' ? 'Add task' : 'Save task';

  const toggleDep = (id: string) =>
    setFields({
      ...fields,
      dependsOn: fields.dependsOn.includes(id) ? fields.dependsOn.filter((d) => d !== id) : [...fields.dependsOn, id],
    });

  return (
    <div className="dialog-backdrop">
      <form
        className="dialog"
        role="dialog"
        aria-label={action}
        onSubmit={(e) => {
          e.preventDefault();
          if (fields.title.trim()) onSubmit(fields);
        }}
      >
        <h2>{mode === 'create' ? 'Add task' : 'Edit task'}</h2>
        <label className="field">
          Title
          <input required value={fields.title} onChange={(e) => setFields({ ...fields, title: e.target.value })} />
        </label>
        <label className="field">
          Description
          <textarea
            rows={3}
            value={fields.description}
            onChange={(e) => setFields({ ...fields, description: e.target.value })}
            placeholder="What the agent should do"
          />
        </label>
        <PriorityField value={fields.priority} onChange={(e) => setFields({ ...fields, priority: e.target.value })} />
        {mode === 'create' && (
          <label className="field">
            Subtask of
            <select
              value={fields.parentTaskId ?? ''}
              onChange={(e) => setFields({ ...fields, parentTaskId: e.target.value || null })}
            >
              <option value="">None, a top-level task</option>
              {candidates.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.title}
                </option>
              ))}
            </select>
          </label>
        )}
        <fieldset className="field deps">
          <legend>Waits for</legend>
          {candidates.length === 0 ? (
            <span className="muted">No other tasks in this quest.</span>
          ) : (
            candidates.map((t) => (
              <label key={t.id} className="row-inline">
                <input type="checkbox" checked={fields.dependsOn.includes(t.id)} onChange={() => toggleDep(t.id)} />
                {t.title}
              </label>
            ))
          )}
          <span className="muted">The task cannot start until these are verified.</span>
        </fieldset>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary">
            {action}
          </button>
        </div>
      </form>
    </div>
  );
};
