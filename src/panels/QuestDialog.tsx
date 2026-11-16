import React, { useState } from 'react';
import { PriorityField } from './PriorityField';

export interface QuestFields {
  title: string;
  description: string;
  priority: string;
  bossName: string;
}

interface QuestDialogProps {
  mode: 'create' | 'edit';
  initial: QuestFields;
  onSubmit: (fields: QuestFields) => void;
  onClose: () => void;
}

export const QuestDialog: React.FC<QuestDialogProps> = ({ mode, initial, onSubmit, onClose }) => {
  const [fields, setFields] = useState(initial);
  const set = (key: keyof QuestFields) => (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    setFields({ ...fields, [key]: e.target.value });
  const action = mode === 'create' ? 'Create quest' : 'Save quest';

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
        <h2>{mode === 'create' ? 'Create quest' : 'Edit quest'}</h2>
        <label className="field">
          Title
          <input required value={fields.title} onChange={set('title')} placeholder="Implement JWT authentication" />
        </label>
        <label className="field">
          Description
          <textarea required rows={3} value={fields.description} onChange={set('description')} placeholder="Describe the software goal" />
        </label>
        <label className="field">
          Boss name
          <input value={fields.bossName} onChange={set('bossName')} />
        </label>
        <PriorityField value={fields.priority} onChange={set('priority')} />
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
