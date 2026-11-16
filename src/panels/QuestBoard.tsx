import React, { useState } from 'react';
import { BossHp, Quest, Task } from '../types';
import { statusFromTask } from '../state/status';
import { StatusChip } from './StatusChip';
import { BossBar } from './BossBar';

interface QuestBoardProps {
  quests: Quest[];
  tasks: Task[];
  activeQuest: Quest | null;
  bossHp: BossHp | null;
  onSelectQuest: (quest: Quest) => void;
  canCreate: boolean;
  onCreateQuest: (title: string, description: string, priority: string, bossName: string) => void;
  onStartTask: (taskId: string) => void;
  onCancelSession: (sessionId: string) => void;
  onMarkVerified: (taskId: string) => void;
  selectedTaskId: string | null;
  onSelectTask: (taskId: string) => void;
}

export const QuestBoard: React.FC<QuestBoardProps> = ({
  quests,
  tasks,
  activeQuest,
  bossHp,
  canCreate,
  onSelectQuest,
  onCreateQuest,
  onStartTask,
  onCancelSession,
  onMarkVerified,
  selectedTaskId,
  onSelectTask,
}) => {
  const [showDialog, setShowDialog] = useState(false);
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [bossName, setBossName] = useState('Build Failure Behemoth');
  const [priority, setPriority] = useState('medium');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;
    onCreateQuest(title, description, priority, bossName);
    setTitle('');
    setDescription('');
    setShowDialog(false);
  };

  return (
    <>
      {activeQuest && (
        <section aria-label="Boss encounter">
          <BossBar name={activeQuest.boss_name ?? 'Unnamed boss'} hp={bossHp} />
        </section>
      )}

      <section aria-labelledby="quests-title">
        <div className="panel-head">
          <h2 id="quests-title">Quests</h2>
          <button className="btn btn-primary" disabled={!canCreate} onClick={() => setShowDialog(true)}>
            Create quest
          </button>
        </div>

        {quests.length === 0 ? (
          <p className="muted">{canCreate ? 'No quests yet. Create a quest to give an agent its first task.' : 'Open a project first, then create a quest.'}</p>
        ) : (
          quests.map((q) => {
            const selected = activeQuest?.id === q.id;
            return (
              <div key={q.id}>
                <button className="quest-item" aria-current={selected} onClick={() => onSelectQuest(q)}>
                  <strong>{q.title}</strong>
                  <p className="muted" style={{ maxWidth: '72ch' }}>
                    {q.description}
                  </p>
                </button>
                {selected && (
                  <ul className="row-list" aria-label={`Tasks in ${q.title}`}>
                    {tasks.map((task) => (
                      <li key={task.id}>
                        <StatusChip status={statusFromTask(task.status)} />
                        <button
                          className="quest-item"
                          aria-current={selectedTaskId === task.id}
                          onClick={() => onSelectTask(task.id)}
                        >
                          {task.title}
                        </button>
                        <span className="task-actions">
                          {task.allowed_actions.includes('start') && (
                            <button className="btn" onClick={() => onStartTask(task.id)}>
                              Start task
                            </button>
                          )}
                          {task.allowed_actions.includes('retry') && (
                            <button className="btn" onClick={() => onStartTask(task.id)}>
                              Retry task
                            </button>
                          )}
                          {task.allowed_actions.includes('cancel') && task.active_session_id && (
                            <button className="btn" onClick={() => onCancelSession(task.active_session_id as string)}>
                              Cancel session
                            </button>
                          )}
                          {task.allowed_actions.includes('mark_verified') && (
                            <button className="btn" onClick={() => onMarkVerified(task.id)}>
                              Mark verified
                            </button>
                          )}
                        </span>
                        {task.failure_reason && <span className="sub">{task.failure_reason}</span>}
                        {task.status === 'verification' && (
                          <span className="sub">Finished, not verified. Run checks to confirm.</span>
                        )}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            );
          })
        )}
      </section>

      {showDialog && (
        <div className="dialog-backdrop">
          <form className="dialog" role="dialog" aria-label="Create quest" onSubmit={handleSubmit}>
            <h2>Create quest</h2>
            <label className="field">
              Title
              <input required value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Implement JWT authentication" />
            </label>
            <label className="field">
              Description
              <textarea
                required
                rows={3}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder="Describe the software goal"
              />
            </label>
            <label className="field">
              Boss name
              <input value={bossName} onChange={(e) => setBossName(e.target.value)} />
            </label>
            <label className="field">
              Priority
              <select value={priority} onChange={(e) => setPriority(e.target.value)}>
                <option value="low">Low</option>
                <option value="medium">Medium</option>
                <option value="high">High</option>
                <option value="urgent">Urgent</option>
              </select>
            </label>
            <div className="dialog-actions">
              <button type="button" className="btn" onClick={() => setShowDialog(false)}>
                Cancel
              </button>
              <button type="submit" className="btn btn-primary">
                Create quest
              </button>
            </div>
          </form>
        </div>
      )}
    </>
  );
};
