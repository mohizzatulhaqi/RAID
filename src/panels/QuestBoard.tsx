import React, { useState } from 'react';
import { Quest, Task } from '../types';
import { Swords, CheckCircle2, Play, Plus, Flame } from 'lucide-react';

interface QuestBoardProps {
  quests: Quest[];
  tasks: Task[];
  activeQuest: Quest | null;
  onSelectQuest: (quest: Quest) => void;
  onCreateQuest: (title: string, description: string, priority: string, bossName: string) => void;
  onStartTask: (taskId: string) => void;
}

export const QuestBoard: React.FC<QuestBoardProps> = ({
  quests,
  tasks,
  activeQuest,
  onSelectQuest,
  onCreateQuest,
  onStartTask,
}) => {
  const [showModal, setShowModal] = useState(false);
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
    setShowModal(false);
  };

  return (
    <div className="glass-panel" style={{ padding: '16px', display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '14px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Swords size={20} color="var(--gold-primary)" />
          <h2 className="title-rpg" style={{ fontSize: '1.1rem' }}>Quest Board</h2>
        </div>
        <button className="btn-primary" onClick={() => setShowModal(true)}>
          <Plus size={16} /> New Quest
        </button>
      </div>

      {/* Active Boss HP Banner */}
      {activeQuest && (
        <div
          style={{
            background: 'linear-gradient(180deg, rgba(239, 68, 68, 0.12), rgba(0,0,0,0.4))',
            border: '1px solid rgba(239, 68, 68, 0.35)',
            borderRadius: '8px',
            padding: '12px',
            marginBottom: '14px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '6px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              <Flame size={16} color="var(--ruby-hp)" />
              <span style={{ fontWeight: 'bold', fontSize: '0.85rem', color: '#fca5a5' }}>
                {activeQuest.boss_name}
              </span>
            </div>
            <span style={{ fontSize: '0.8rem', color: '#fca5a5', fontWeight: 'bold' }}>
              HP: {activeQuest.boss_current_hp} / {activeQuest.boss_max_hp}
            </span>
          </div>

          <div className="hp-bar-container">
            <div
              className="hp-bar-fill"
              style={{ width: `${Math.max(0, Math.min(100, activeQuest.boss_current_hp))}%` }}
            />
          </div>
          <div style={{ fontSize: '0.72rem', color: 'var(--text-muted)', marginTop: '4px' }}>
            70% Milestones • 30% Build & Test Verification
          </div>
        </div>
      )}

      {/* Quest Selection & Task List */}
      <div style={{ flex: 1, overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '10px' }}>
        {quests.length === 0 ? (
          <div style={{ textAlign: 'center', color: 'var(--text-faint)', padding: '24px', fontSize: '0.85rem' }}>
            No quests posted yet. Embark on a new quest above!
          </div>
        ) : (
          quests.map((q) => {
            const isSelected = activeQuest?.id === q.id;
            return (
              <div
                key={q.id}
                onClick={() => onSelectQuest(q)}
                style={{
                  background: isSelected ? 'rgba(245, 158, 11, 0.1)' : 'rgba(255, 255, 255, 0.03)',
                  border: isSelected ? '1px solid var(--gold-primary)' : '1px solid var(--border-subtle)',
                  borderRadius: '8px',
                  padding: '12px',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: '4px' }}>
                  <span style={{ fontWeight: '600', fontSize: '0.9rem', color: isSelected ? 'var(--gold-light)' : 'var(--text-main)' }}>
                    {q.title}
                  </span>
                  <span className={`badge badge-${q.status === 'completed' ? 'emerald' : 'gold'}`}>
                    {q.status}
                  </span>
                </div>
                <p style={{ fontSize: '0.8rem', color: 'var(--text-muted)', marginBottom: '8px' }}>
                  {q.description}
                </p>

                {/* Subtasks */}
                {isSelected && (
                  <div style={{ marginTop: '10px', borderTop: '1px solid var(--border-subtle)', paddingTop: '8px' }}>
                    <div style={{ fontSize: '0.75rem', fontWeight: 'bold', color: 'var(--gold-light)', marginBottom: '6px' }}>
                      TASK OBJECTIVES:
                    </div>
                    {tasks.map((task) => (
                      <div
                        key={task.id}
                        style={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                          padding: '6px 8px',
                          background: 'rgba(0, 0, 0, 0.25)',
                          borderRadius: '6px',
                          marginBottom: '4px',
                        }}
                      >
                        <div>
                          <div style={{ fontSize: '0.82rem', fontWeight: '500' }}>{task.title}</div>
                          <div style={{ fontSize: '0.72rem', color: 'var(--text-faint)' }}>{task.description}</div>
                        </div>

                        {task.status === 'completed' ? (
                          <CheckCircle2 size={16} color="var(--emerald-success)" />
                        ) : task.status === 'running' ? (
                          <span className="badge badge-ruby glow-pulse">RUNNING</span>
                        ) : (
                          <button
                            className="btn-ghost"
                            style={{ padding: '4px 8px', fontSize: '0.75rem' }}
                            onClick={(e) => {
                              e.stopPropagation();
                              onStartTask(task.id);
                            }}
                          >
                            <Play size={12} /> Launch
                          </button>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Create Quest Modal */}
      {showModal && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0, 0, 0, 0.75)',
            backdropFilter: 'blur(8px)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 100,
          }}
        >
          <div className="glass-panel" style={{ width: '450px', padding: '24px', background: 'var(--bg-surface)' }}>
            <h3 className="title-rpg" style={{ marginBottom: '16px', fontSize: '1.2rem' }}>Post New Quest</h3>
            <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)', display: 'block', marginBottom: '4px' }}>
                  Quest Title
                </label>
                <input
                  type="text"
                  required
                  value={title}
                  onChange={(e) => setTitle(e.target.value)}
                  placeholder="e.g. Implement JWT Authentication"
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '6px',
                    background: 'var(--bg-darker)',
                    border: '1px solid var(--border-subtle)',
                    color: '#fff',
                    outline: 'none',
                  }}
                />
              </div>

              <div>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)', display: 'block', marginBottom: '4px' }}>
                  Description / Task Objectives
                </label>
                <textarea
                  required
                  rows={3}
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  placeholder="Describe the software engineering goal..."
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '6px',
                    background: 'var(--bg-darker)',
                    border: '1px solid var(--border-subtle)',
                    color: '#fff',
                    outline: 'none',
                  }}
                />
              </div>

              <div>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)', display: 'block', marginBottom: '4px' }}>
                  Boss Encounter Name
                </label>
                <input
                  type="text"
                  value={bossName}
                  onChange={(e) => setBossName(e.target.value)}
                  placeholder="e.g. Auth Hydra or Build Titan"
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '6px',
                    background: 'var(--bg-darker)',
                    border: '1px solid var(--border-subtle)',
                    color: '#fff',
                    outline: 'none',
                  }}
                />
              </div>

              <div>
                <label style={{ fontSize: '0.8rem', color: 'var(--text-muted)', display: 'block', marginBottom: '4px' }}>
                  Quest Priority
                </label>
                <select
                  value={priority}
                  onChange={(e) => setPriority(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '6px',
                    background: 'var(--bg-darker)',
                    border: '1px solid var(--border-subtle)',
                    color: '#fff',
                    outline: 'none',
                  }}
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                  <option value="urgent">Urgent</option>
                </select>
              </div>

              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '10px', marginTop: '12px' }}>
                <button type="button" className="btn-ghost" onClick={() => setShowModal(false)}>
                  Cancel
                </button>
                <button type="submit" className="btn-primary">
                  Summon Quest
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
