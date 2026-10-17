import React from 'react';
import { AlertTriangle, Check, X } from 'lucide-react';

interface ApprovalModalProps {
  actionDescription: string;
  sessionId: string;
  onApprove: () => void;
  onReject: () => void;
}

export const ApprovalModal: React.FC<ApprovalModalProps> = ({
  actionDescription,
  onApprove,
  onReject,
}) => {
  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0, 0, 0, 0.8)',
        backdropFilter: 'blur(8px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 200,
      }}
    >
      <div
        className="glass-panel"
        style={{
          width: '500px',
          padding: '24px',
          background: 'var(--bg-surface-elevated)',
          border: '2px solid var(--gold-primary)',
          boxShadow: '0 0 30px rgba(245, 158, 11, 0.3)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '14px' }}>
          <AlertTriangle size={24} color="var(--gold-primary)" />
          <h3 className="title-rpg" style={{ fontSize: '1.2rem', color: 'var(--gold-light)' }}>
            Guardian Approval Required
          </h3>
        </div>

        <p style={{ fontSize: '0.88rem', color: 'var(--text-main)', marginBottom: '12px', lineHeight: '1.5' }}>
          Your party agent has encountered a guarded boundary or risky action and requests your royal consent:
        </p>

        <div
          style={{
            background: 'rgba(0, 0, 0, 0.4)',
            border: '1px solid var(--border-subtle)',
            borderRadius: '6px',
            padding: '12px',
            fontFamily: 'var(--font-mono)',
            fontSize: '0.82rem',
            color: 'var(--text-gold)',
            marginBottom: '20px',
            wordBreak: 'break-all',
          }}
        >
          {actionDescription || 'Executing terminal operation...'}
        </div>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '12px' }}>
          <button className="btn-danger" onClick={onReject} style={{ padding: '8px 16px', fontSize: '0.85rem' }}>
            <X size={16} /> Deny Action (n)
          </button>
          <button className="btn-primary" onClick={onApprove} style={{ padding: '8px 20px', fontSize: '0.85rem' }}>
            <Check size={16} /> Approve Action (y)
          </button>
        </div>
      </div>
    </div>
  );
};
