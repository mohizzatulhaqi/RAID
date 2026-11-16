import React from 'react';
import { StatusKind } from '../types';

interface StatusMeta {
  label: string;
  icon: React.ReactNode;
}

const stroke = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.6 } as const;

// One distinct shape per state (DESIGN §6.1). Color comes from the chip's
// `data-status` rule in index.css, which only uses `--status-*` tokens.
const STATUS: Record<StatusKind, StatusMeta> = {
  idle: {
    label: 'Idle',
    icon: <circle cx="8" cy="8" r="5.5" {...stroke} />,
  },
  assigned: {
    label: 'Assigned',
    icon: (
      <>
        <circle cx="8" cy="8" r="5.5" {...stroke} />
        <path d="M8 2.5a5.5 5.5 0 0 1 0 11z" fill="currentColor" />
      </>
    ),
  },
  preparing: {
    label: 'Preparing',
    icon: (
      <>
        <circle cx="8" cy="8" r="5.5" {...stroke} />
        <path d="M8 2.5a5.5 5.5 0 0 1 0 11z" fill="currentColor" />
      </>
    ),
  },
  running: {
    label: 'Running',
    icon: (
      <g className="spin">
        <path d="M8 2.5 13.5 12.5H2.5z" fill="currentColor" />
      </g>
    ),
  },
  waiting: {
    label: 'Waiting',
    icon: (
      <>
        <rect x="4" y="3" width="2.6" height="10" fill="currentColor" />
        <rect x="9.4" y="3" width="2.6" height="10" fill="currentColor" />
      </>
    ),
  },
  needs_approval: {
    label: 'Needs approval',
    icon: (
      <>
        <path d="M8 1.5 14.5 8 8 14.5 1.5 8z" {...stroke} />
        <path d="M8 4.8v4" stroke="currentColor" strokeWidth="1.6" />
        <circle cx="8" cy="11" r="0.9" fill="currentColor" />
      </>
    ),
  },
  verifying: {
    label: 'Verifying',
    icon: (
      <>
        <circle cx="8" cy="8" r="5.5" {...stroke} />
        <g className="spin">
          <circle cx="8" cy="4" r="1.6" fill="currentColor" />
        </g>
      </>
    ),
  },
  // Outlined check, no fill: finished but nothing proved it.
  finished_unverified: {
    label: 'Finished, not verified',
    icon: <path d="M2.5 8.5 6.3 12.3 13.5 4.2" {...stroke} strokeLinecap="round" strokeLinejoin="round" />,
  },
  // Check cut out of a filled box: proven by evidence.
  verified: {
    label: 'Verified',
    icon: (
      <>
        <rect x="1.5" y="1.5" width="13" height="13" rx="1.5" fill="currentColor" />
        <path
          d="M4.5 8.4 7 10.9 11.6 5.5"
          fill="none"
          stroke="var(--surface)"
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </>
    ),
  },
  needs_review: {
    label: 'Needs review',
    icon: (
      <>
        <rect x="2" y="2" width="12" height="12" rx="1.5" {...stroke} />
        <path d="M5 8h6" stroke="currentColor" strokeWidth="1.6" />
        <circle cx="8" cy="5" r="0.9" fill="currentColor" />
        <circle cx="8" cy="11" r="0.9" fill="currentColor" />
      </>
    ),
  },
  failed: {
    label: 'Failed',
    icon: (
      <>
        <path d="M5.2 1.5h5.6l3.7 3.7v5.6l-3.7 3.7H5.2l-3.7-3.7V5.2z" {...stroke} />
        <path d="M5.6 5.6l4.8 4.8M10.4 5.6l-4.8 4.8" stroke="currentColor" strokeWidth="1.6" />
      </>
    ),
  },
  cancelled: {
    label: 'Cancelled',
    icon: (
      <>
        <circle cx="8" cy="8" r="5.5" {...stroke} />
        <path d="M4.1 11.9 11.9 4.1" stroke="currentColor" strokeWidth="1.6" />
      </>
    ),
  },
};

export function statusLabel(kind: StatusKind): string {
  return STATUS[kind].label;
}

export const StatusChip: React.FC<{ status: StatusKind }> = ({ status }) => {
  const meta = STATUS[status];
  return (
    <span className="chip" data-status={status} role="status">
      <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
        {meta.icon}
      </svg>
      {meta.label}
    </span>
  );
};
