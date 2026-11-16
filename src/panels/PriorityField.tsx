import React from 'react';

export const PRIORITY_LABEL: Record<string, string> = {
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  urgent: 'Urgent',
};

export const PriorityField: React.FC<{
  value: string;
  onChange: (e: React.ChangeEvent<HTMLSelectElement>) => void;
}> = ({ value, onChange }) => (
  <label className="field">
    Priority
    <select value={value} onChange={onChange}>
      {Object.entries(PRIORITY_LABEL).map(([v, label]) => (
        <option key={v} value={v}>
          {label}
        </option>
      ))}
    </select>
  </label>
);
