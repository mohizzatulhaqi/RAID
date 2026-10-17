import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Project, Quest, Task, Character, NormalizedEvent, PresentationState } from './types';
import { WorldView } from './components/WorldView';
import { QuestBoard } from './components/QuestBoard';
import { PartyPanel } from './components/PartyPanel';
import { ActivityLog } from './components/ActivityLog';
import { ApprovalModal } from './components/ApprovalModal';
import { Shield, FolderGit2, Compass } from 'lucide-react';

export function App() {
  const [project, setProject] = useState<Project | null>(null);
  const [quests, setQuests] = useState<Quest[]>([]);
  const [activeQuest, setActiveQuest] = useState<Quest | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [events, setEvents] = useState<NormalizedEvent[]>([]);
  const [, setActiveSessionId] = useState<string | null>(null);
  const [approvalPrompt, setApprovalPrompt] = useState<{ action: string; sessionId: string } | null>(null);

  // Default active presentation state
  const [currentWorldState, setCurrentWorldState] = useState<PresentationState>('idle');

  // 1. Initialize Project & Characters on Mount
  useEffect(() => {
    const initApp = async () => {
      try {
        // Automatically open the current working project directory
        const proj = await invoke<Project>('open_project', {
          repoPath: '/Users/kiki/Desktop/KULIAH/projekan/RAID',
        });
        setProject(proj);

        // Fetch characters
        const chars = await invoke<Character[]>('get_characters');
        setCharacters(chars);

        // Fetch existing quests
        const qs = await invoke<Quest[]>('get_quests', { projectId: proj.id });
        setQuests(qs);
        if (qs.length > 0) {
          setActiveQuest(qs[0]);
        }
      } catch (err) {
        console.error('Error initializing RAID app:', err);
      }
    };

    initApp();
  }, []);

  // 2. Fetch Tasks when activeQuest changes
  useEffect(() => {
    if (!activeQuest) return;
    const fetchTasks = async () => {
      try {
        const tList = await invoke<Task[]>('get_tasks', { questId: activeQuest.id });
        setTasks(tList);
      } catch (err) {
        console.error('Error fetching tasks:', err);
      }
    };
    fetchTasks();
  }, [activeQuest]);

  // 3. Listen to Real-time Backend Agent Events
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<NormalizedEvent>('agent-event', (event) => {
          const ev = event.payload;
          setEvents((prev) => [...prev, ev]);
          setCurrentWorldState(ev.presentation_state);

          // Update character states dynamically
          setCharacters((prev) =>
            prev.map((c) => ({
              ...c,
              state: ev.presentation_state,
            }))
          );

          // Detect approval requirement
          if (ev.presentation_state === 'needs_approval' || ev.payload.type === 'approval_required') {
            setApprovalPrompt({
              action: ev.payload.data?.action_description || 'Claude Code requires command approval',
              sessionId: ev.session_id,
            });
          }

          // Check if task completed or failed to reduce Boss HP
          if (ev.presentation_state === 'verification' || ev.payload.type === 'session_succeeded') {
            setActiveQuest((prev) => {
              if (!prev) return null;
              // Reduce boss HP by 35% on milestone completion
              const newHp = Math.max(0, prev.boss_current_hp - 35);
              return { ...prev, boss_current_hp: newHp };
            });
          }
        });
      } catch (err) {
        console.error('Failed to listen to agent-event:', err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  // Create Quest Handler
  const handleCreateQuest = async (title: string, description: string, priority: string, bossName: string) => {
    if (!project) return;
    try {
      const newQuest = await invoke<Quest>('create_quest', {
        projectId: project.id,
        title,
        description,
        priority,
        bossName,
      });

      // Automatically add initial subtask objective
      const initialTask = await invoke<Task>('create_task', {
        questId: newQuest.id,
        title: `Implement: ${title}`,
        description,
        priority,
        weightPercent: 70.0,
      });

      setQuests((prev) => [newQuest, ...prev]);
      setActiveQuest(newQuest);
      setTasks([initialTask]);
    } catch (err) {
      console.error('Error creating quest:', err);
    }
  };

  // Launch Task Handler
  const handleStartTask = async (taskId: string) => {
    try {
      setEvents((prev) => [
        ...prev,
        {
          session_id: 'init',
          timestamp: new Date().toISOString(),
          presentation_state: 'entering',
          payload: {
            type: 'session_started',
            data: { text: 'Departing Guild Hub into the repository dungeon...' },
          },
        },
      ]);

      const sessionId = await invoke<string>('start_quest_task', { taskId });
      setActiveSessionId(sessionId);
    } catch (err) {
      console.error('Failed to start task:', err);
      alert(`Could not start agent session: ${err}`);
    }
  };

  // User Approval Handlers
  const handleApprove = async () => {
    if (!approvalPrompt) return;
    try {
      await invoke('send_agent_input', {
        sessionId: approvalPrompt.sessionId,
        input: 'y',
      });
      setApprovalPrompt(null);
    } catch (err) {
      console.error('Failed to send approval:', err);
    }
  };

  const handleReject = async () => {
    if (!approvalPrompt) return;
    try {
      await invoke('send_agent_input', {
        sessionId: approvalPrompt.sessionId,
        input: 'n',
      });
      setApprovalPrompt(null);
    } catch (err) {
      console.error('Failed to reject action:', err);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', width: '100vw', background: 'var(--bg-darker)' }}>
      {/* Top Navigation Bar */}
      <header
        style={{
          height: '56px',
          borderBottom: '1px solid var(--border-subtle)',
          background: 'rgba(15, 19, 28, 0.85)',
          backdropFilter: 'blur(12px)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 20px',
          zIndex: 10,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div
            style={{
              background: 'linear-gradient(135deg, #f59e0b, #b45309)',
              padding: '6px',
              borderRadius: '8px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              boxShadow: '0 0 12px var(--gold-glow)',
            }}
          >
            <Shield size={20} color="#fff" />
          </div>
          <div>
            <h1 className="title-rpg" style={{ fontSize: '1.15rem', lineHeight: '1.2' }}>
              RAID
            </h1>
            <div style={{ fontSize: '0.68rem', color: 'var(--text-faint)', letterSpacing: '0.04em' }}>
              ROLE-BASED AI DEVELOPMENT
            </div>
          </div>
        </div>

        {/* Project Path Indicator */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            background: 'rgba(0, 0, 0, 0.35)',
            border: '1px solid var(--border-subtle)',
            borderRadius: '6px',
            padding: '6px 12px',
            fontSize: '0.8rem',
          }}
        >
          <FolderGit2 size={16} color="var(--gold-primary)" />
          <span style={{ color: 'var(--text-muted)' }}>Project:</span>
          <span style={{ color: '#fff', fontWeight: '500' }}>
            {project ? project.name : 'Connecting...'}
          </span>
          <span className="badge badge-cyan" style={{ marginLeft: '6px' }}>
            {project?.default_branch || 'main'}
          </span>
        </div>

        {/* World Status Indicator */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Compass size={16} color="var(--mana-cyan)" />
          <span style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>Zone:</span>
          <span className="badge badge-gold">GUILD HUB (2D)</span>
        </div>
      </header>

      {/* Main Content Layout */}
      <div style={{ flex: 1, display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: '14px', padding: '14px', overflow: 'hidden' }}>
        {/* Left Column: PixiJS 2D World View + Activity Log */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '14px', overflow: 'hidden' }}>
          {/* 2D RPG World Canvas */}
          <div style={{ flex: '1.3', position: 'relative', minHeight: '300px' }}>
            <WorldView
              characters={characters}
              activeState={currentWorldState}
              bossHp={activeQuest ? activeQuest.boss_current_hp : 100}
              bossName={activeQuest ? activeQuest.boss_name : 'No Active Boss'}
            />
          </div>

          {/* Combat & Agent Log */}
          <div style={{ flex: '1', overflow: 'hidden' }}>
            <ActivityLog events={events} />
          </div>
        </div>

        {/* Right Column: Quest Board + Party Panel */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '14px', overflow: 'hidden' }}>
          {/* Party Roster */}
          <PartyPanel characters={characters} />

          {/* Quest Board */}
          <div style={{ flex: 1, overflow: 'hidden' }}>
            <QuestBoard
              quests={quests}
              tasks={tasks}
              activeQuest={activeQuest}
              onSelectQuest={setActiveQuest}
              onCreateQuest={handleCreateQuest}
              onStartTask={handleStartTask}
            />
          </div>
        </div>
      </div>

      {/* Guardian Approval Modal for Risky Operations */}
      {approvalPrompt && (
        <ApprovalModal
          actionDescription={approvalPrompt.action}
          sessionId={approvalPrompt.sessionId}
          onApprove={handleApprove}
          onReject={handleReject}
        />
      )}
    </div>
  );
}

export default App;
