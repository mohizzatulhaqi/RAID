import React from 'react';
import { StatusChip } from './StatusChip';

interface ApprovalBannerProps {
  actionDescription: string;
  onApprove: () => void;
  onReject: () => void;
}

/** Approval banner in the dock (DESIGN §6.5). No countdown, never automatic. */
export const ApprovalBanner: React.FC<ApprovalBannerProps> = ({ actionDescription, onApprove, onReject }) => (
  <div className="banner-approval" role="alertdialog" aria-label="Approval needed">
    <div className="panel-head">
      <StatusChip status="needs_approval" />
    </div>
    <p>Approval needed:</p>
    <p className="mono" style={{ wordBreak: 'break-all', margin: 'var(--space-2) 0' }}>
      {actionDescription}
    </p>
    <div className="dialog-actions">
      <button className="btn" onClick={onReject}>
        Deny
      </button>
      <button className="btn btn-primary" onClick={onApprove}>
        Approve
      </button>
    </div>
  </div>
);
