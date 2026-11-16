import React, { useState } from 'react';
import { BossHp, Character, Quest, Task } from '../types';
import { statusFromTask } from '../state/status';
import { StatusChip } from './StatusChip';
import { BossBar } from './BossBar';
import { QuestDialog, QuestFields } from './QuestDialog';
import { TaskDialog, TaskFields } from './TaskDialog';
import { PRIORITY_LABEL } from './PriorityField';

const EDITABLE: Task['status'][] = ['created', 'ready', 'blocked', 'failed', 'cancelled'];

/** Top-level tasks first, each followed by its subtasks. */
function ordered(tasks: Task[]): { task: Task; depth: number }[] {
  const ids = new Set(tasks.map((t) => t.id));
  const out: { task: Task; depth: number }[] = [];
  const visit = (parent: string | null, depth: number) => {
    tasks
      .filter((t) => (t.parent_task_id && ids.has(t.parent_task_id) ? t.parent_task_id : null) === parent)
      .forEach((t) => {
        out.push({ task: t, depth });
        if (depth < 4) visit(t.id, depth + 1);
      });
  };
  visit(null, 0);
  return out;
}

interface QuestBoardProps {
  quests: Quest[];
  tasks: Task[];
  activeQuest: Quest | null;
  bossHp: BossHp | null;
  onSelectQuest: (quest: Quest) => void;
  canCreate: boolean;
  onCreateQuest: (fields: QuestFields) => void;
  onUpdateQuest: (questId: string, fields: QuestFields) => void;
  onCreateTask: (questId: string, fields: TaskFields) => void;
  onUpdateTask: (taskId: string, fields: TaskFields) => void;
  characters: Character[];
  onStartTask: (taskId: string, characterId: string | null) => void;
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
  onUpdateQuest,
  onCreateTask,
  onUpdateTask,
  characters,
  onStartTask,
  onCancelSession,
  onMarkVerified,
  selectedTaskId,
  onSelectTask,
}) => {
  const [questDialog, setQuestDialog] = useState<'create' | 'edit' | null>(null);
  const [taskDialog, setTaskDialog] = useState<{ mode: 'create' } | { mode: 'edit'; task: Task } | null>(null);
  const [assignee, setAssignee] = useState<string>('');
  const freeMembers = characters.filter((c) => !c.active_session_id);

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
          <button className="btn btn-primary" disabled={!canCreate} onClick={() => setQuestDialog('create')}>
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
                  <div className="task-actions" style={{ margin: 'var(--space-2) 0' }}>
                    <button className="btn" onClick={() => setQuestDialog('edit')}>
                      Edit quest
                    </button>
                    <button className="btn" onClick={() => setTaskDialog({ mode: 'create' })}>
                      Add task
                    </button>
                    <span className="muted">Priority {PRIORITY_LABEL[q.priority] ?? q.priority}</span>
                  </div>
                )}
                {selected && (
                  <ul className="row-list" aria-label={`Tasks in ${q.title}`}>
                    {ordered(tasks).map(({ task, depth }) => (
                      <li key={task.id} style={{ paddingLeft: depth * 20 }}>
                        <StatusChip status={statusFromTask(task.status)} />
                        <button
                          className="quest-item"
                          aria-current={selectedTaskId === task.id}
                          onClick={() => onSelectTask(task.id)}
                        >
                          {task.title}
                        </button>
                        <span className="task-actions">
                          <span className="muted">{PRIORITY_LABEL[task.priority] ?? task.priority}</span>
                          {EDITABLE.includes(task.status) && (
                            <button className="btn" onClick={() => setTaskDialog({ mode: 'edit', task })}>
                              Edit
                            </button>
                          )}
                          {(task.allowed_actions.includes('start') || task.allowed_actions.includes('retry')) && (
                            <>
                              <select
                                className="btn"
                                aria-label={`Party member for ${task.title}`}
                                value={assignee}
                                onChange={(e) => setAssignee(e.target.value)}
                              >
                                <option value="">Any free member</option>
                                {freeMembers.map((c) => (
                                  <option key={c.id} value={c.id}>
                                    {c.display_name}
                                  </option>
                                ))}
                              </select>
                              <button className="btn" onClick={() => onStartTask(task.id, assignee || null)}>
                                {task.allowed_actions.includes('retry') ? 'Retry task' : 'Start task'}
                              </button>
                            </>
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
                        {task.status === 'blocked' && (
                          <span className="sub">
                            Waiting for:{' '}
                            {task.depends_on
                              .filter((d) => d.status !== 'completed')
                              .map((d) => d.title)
                              .join(', ') || 'other tasks'}
                          </span>
                        )}
                        {task.status !== 'blocked' && task.failure_reason && (
                          <span className="sub">{task.failure_reason}</span>
                        )}
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

      {questDialog && (
        <QuestDialog
          mode={questDialog}
          initial={
            questDialog === 'edit' && activeQuest
              ? {
                  title: activeQuest.title,
                  description: activeQuest.description,
                  priority: activeQuest.priority,
                  bossName: activeQuest.boss_name ?? '',
                }
              : { title: '', description: '', priority: 'medium', bossName: 'Build Failure Behemoth' }
          }
          onClose={() => setQuestDialog(null)}
          onSubmit={(fields) => {
            if (questDialog === 'edit' && activeQuest) onUpdateQuest(activeQuest.id, fields);
            else onCreateQuest(fields);
            setQuestDialog(null);
          }}
        />
      )}

      {taskDialog && activeQuest && (
        <TaskDialog
          mode={taskDialog.mode}
          initial={
            taskDialog.mode === 'edit'
              ? {
                  title: taskDialog.task.title,
                  description: taskDialog.task.description,
                  priority: taskDialog.task.priority,
                  parentTaskId: taskDialog.task.parent_task_id ?? null,
                  dependsOn: taskDialog.task.depends_on.map((d) => d.id),
                }
              : { title: '', description: '', priority: 'medium', parentTaskId: null, dependsOn: [] }
          }
          candidates={tasks.filter((t) => taskDialog.mode === 'create' || t.id !== taskDialog.task.id)}
          onClose={() => setTaskDialog(null)}
          onSubmit={(fields) => {
            if (taskDialog.mode === 'edit') onUpdateTask(taskDialog.task.id, fields);
            else onCreateTask(activeQuest.id, fields);
            setTaskDialog(null);
          }}
        />
      )}
    </>
  );
};
