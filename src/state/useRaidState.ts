import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type { QuestFields } from '../panels/QuestDialog';
import type { TaskFields } from '../panels/TaskDialog';
import { BossHp, CheckReport, DetectedChecks, TaskDetail, WorktreeChanges, RepoIssue, RepoReport, Character, NormalizedEvent, PresentationState, Project, Quest, Task } from '../types';

const MAX_EVENTS = 1000; // ring buffer: bounded memory for long sessions

// Events that can change task or verification evidence, so HP is re-read.
const EVIDENCE_EVENTS = new Set([
  'verification_completed',
  'session_succeeded',
  'session_failed',
  'session_cancelled',
]);

const LAST_PROJECT_KEY = 'raid.lastProjectPath';

function readLastProject(): string | null {
  try {
    return localStorage.getItem(LAST_PROJECT_KEY);
  } catch {
    return null;
  }
}

function rememberProject(path: string) {
  try {
    localStorage.setItem(LAST_PROJECT_KEY, path);
  } catch {
    // Storage can be unavailable; remembering the project is only a convenience.
  }
}

const AUTO_RUN_KEY = 'raid.autoRunChecks';

function readAutoRun(): boolean {
  try {
    return localStorage.getItem(AUTO_RUN_KEY) !== 'off';
  } catch {
    return true;
  }
}

const hasChecks = (p: Project) => [p.build_cmd, p.test_cmd, p.lint_cmd].some((c) => c && c.trim() !== '');
const hasFound = (d: DetectedChecks) => Boolean(d.build_cmd || d.test_cmd || d.lint_cmd);

const errorText = (err: unknown) => (err instanceof Error ? err.message : String(err));

export function useRaidState() {
  const [project, setProject] = useState<Project | null>(null);
  const [quests, setQuests] = useState<Quest[]>([]);
  const [activeQuest, setActiveQuest] = useState<Quest | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [bossHp, setBossHp] = useState<BossHp | null>(null);
  const [events, setEvents] = useState<NormalizedEvent[]>([]);
  const [sessionState, setSessionState] = useState<PresentationState>('idle');
  const [approval, setApproval] = useState<{ action: string; sessionId: string } | null>(null);
  const [checkReports, setCheckReports] = useState<CheckReport[]>([]);
  const [repoIssue, setRepoIssue] = useState<RepoIssue | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const [pickedTaskId, setPickedTaskId] = useState<string | null>(null);
  const [taskDetail, setTaskDetail] = useState<TaskDetail | null>(null);
  const [changes, setChanges] = useState<{ questId: string; data: WorktreeChanges | null } | null>(null);
  const [autoRun, setAutoRun] = useState<boolean>(readAutoRun);

  const activeQuestId = activeQuest?.id ?? null;

  // Latest values for callbacks that must not change identity.
  const projectRef = useRef<Project | null>(null);
  const autoRunRef = useRef(autoRun);
  const autoTriggered = useRef(new Set<string>());
  const autoVerifyRef = useRef<(task: Task, questId: string) => Promise<void>>(async () => undefined);
  useEffect(() => {
    projectRef.current = project;
    autoRunRef.current = autoRun;
  }, [project, autoRun]);

  const refreshQuest = useCallback(async (questId: string) => {
    try {
      const [t, hp] = await Promise.all([
        invoke<Task[]>('get_tasks', { questId }),
        invoke<BossHp>('get_boss_hp', { questId }),
      ]);
      setTasks(t);
      setBossHp(hp);
      // A finished agent leaves its task in Verification: gather the evidence.
      if (autoRunRef.current) {
        for (const task of t) {
          const key = `${task.id}:${task.updated_at}`;
          if (task.status === 'verification' && !autoTriggered.current.has(key)) {
            autoTriggered.current.add(key);
            void autoVerifyRef.current(task, questId);
          }
        }
      }
    } catch (err) {
      setNotice(`Could not load quest data: ${errorText(err)}`);
    }
  }, []);

  const refreshParty = useCallback(async () => {
    try {
      setCharacters(await invoke<Character[]>('get_characters'));
    } catch (err) {
      setNotice(`Could not load the party: ${errorText(err)}`);
    }
  }, []);

  const loadProject = useCallback(async (repoPath: string) => {
    try {
      const report = await invoke<RepoReport>('inspect_repo', { repoPath });
      if (report.kind !== 'ready') {
        setRepoIssue({ path: repoPath, report });
        return;
      }
      setRepoIssue(null);
      let proj = await invoke<Project>('open_project', { repoPath });
      rememberProject(repoPath);
      // First look: fill in checks from files that really exist.
      if (!hasChecks(proj)) {
        const found = await invoke<DetectedChecks>('detect_checks', { projectId: proj.id, questId: null });
        if (hasFound(found)) {
          proj = await invoke<Project>('set_project_checks', {
            projectId: proj.id,
            buildCmd: found.build_cmd,
            testCmd: found.test_cmd,
            lintCmd: found.lint_cmd,
          });
        }
      }
      setProject(proj);
      setEvents(await invoke<NormalizedEvent[]>('get_recent_events', { projectId: proj.id }));
      setCharacters(await invoke<Character[]>('get_characters'));
      const qs = await invoke<Quest[]>('get_quests', { projectId: proj.id });
      setQuests(qs);
      setActiveQuest(qs.length > 0 ? qs[0] : null);
      setTasks([]);
      setCheckReports([]);
    } catch (err) {
      setNotice(`Could not open the project: ${errorText(err)}`);
    }
  }, []);

  useEffect(() => {
    const last = readLastProject();
    // Initial load of the remembered project; state is set after awaiting the backend.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (last) loadProject(last);
  }, [loadProject]);

  const initializeRepo = async () => {
    if (!repoIssue) return;
    try {
      const report = await invoke<RepoReport>('initialize_repo', { repoPath: repoIssue.path });
      if (report.kind === 'ready') await loadProject(repoIssue.path);
      else setRepoIssue({ path: repoIssue.path, report });
    } catch (err) {
      setNotice(errorText(err));
    }
  };

  const chooseProject = async () => {
    const picked = await open({ directory: true, title: 'Choose a git repository' });
    if (typeof picked === 'string') await loadProject(picked);
  };

  useEffect(() => {
    if (!activeQuestId) return;
    let stale = false;
    (async () => {
      try {
        const [t, hp] = await Promise.all([
          invoke<Task[]>('get_tasks', { questId: activeQuestId }),
          invoke<BossHp>('get_boss_hp', { questId: activeQuestId }),
        ]);
        if (!stale) {
          setTasks(t);
          setBossHp(hp);
        }
      } catch (err) {
        if (!stale) setNotice(`Could not load quest data: ${errorText(err)}`);
      }
    })();
    return () => {
      stale = true;
    };
  }, [activeQuestId]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<NormalizedEvent>('agent-event', (e) => {
      const ev = e.payload;
      setEvents((prev) => [...prev.slice(-(MAX_EVENTS - 1)), ev]);
      setSessionState(ev.presentation_state);
      if (ev.payload.type === 'approval_required') {
        setApproval({ action: ev.payload.data.action_description, sessionId: ev.session_id });
      }
      // Only the character that owns this session reacts to it.
      setCharacters((prev) =>
        prev.map((c) => (c.active_session_id === ev.session_id ? { ...c, last_event: ev } : c)),
      );
      if (EVIDENCE_EVENTS.has(ev.payload.type) && activeQuestId) refreshQuest(activeQuestId);
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => setNotice(`Could not subscribe to agent events: ${errorText(err)}`));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [activeQuestId, refreshQuest]);

  const selectedTask = tasks.find((t) => t.id === pickedTaskId) ?? tasks[0] ?? null;
  const selectedTaskId = selectedTask?.id ?? null;
  const selectedStamp = selectedTask ? `${selectedTask.id}:${selectedTask.updated_at}` : null;

  // Reload the history whenever the selected task changes state.
  useEffect(() => {
    if (!selectedTaskId) return;
    let stale = false;
    invoke<TaskDetail>('get_task_detail', { taskId: selectedTaskId })
      .then((d) => {
        if (!stale) setTaskDetail(d);
      })
      .catch((err) => {
        if (!stale) setNotice(`Could not load the task history: ${errorText(err)}`);
      });
    return () => {
      stale = true;
    };
  }, [selectedTaskId, selectedStamp]);

  // The backend announces every task/session status change after committing it.
  useEffect(() => {
    if (!activeQuestId) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen('task-state-changed', () => {
      refreshQuest(activeQuestId);
      refreshParty();
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => setNotice(`Could not subscribe to task updates: ${errorText(err)}`));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [activeQuestId, refreshQuest, refreshParty]);

  const createQuest = async (fields: QuestFields) => {
    if (!project) {
      setNotice('Open a project before creating a quest.');
      return;
    }
    try {
      const quest = await invoke<Quest>('create_quest', {
        projectId: project.id,
        title: fields.title,
        description: fields.description,
        priority: fields.priority,
        bossName: fields.bossName,
      });
      // Every quest starts with one task the agent can pick up.
      await invoke<string>('create_task', {
        questId: quest.id,
        title: `Implement: ${fields.title}`,
        description: fields.description,
        priority: fields.priority,
        parentTaskId: null,
        dependsOn: [],
      });
      setQuests((prev) => [quest, ...prev]);
      setActiveQuest(quest);
      await refreshQuest(quest.id);
    } catch (err) {
      setNotice(`Could not create the quest: ${errorText(err)}`);
    }
  };

  const updateQuest = async (questId: string, fields: QuestFields) => {
    try {
      const quest = await invoke<Quest>('update_quest', {
        questId,
        title: fields.title,
        description: fields.description,
        priority: fields.priority,
        bossName: fields.bossName,
      });
      setQuests((prev) => prev.map((q) => (q.id === quest.id ? quest : q)));
      setActiveQuest((prev) => (prev?.id === quest.id ? quest : prev));
    } catch (err) {
      setNotice(`Could not save the quest: ${errorText(err)}`);
    }
  };

  const createTask = async (questId: string, fields: TaskFields) => {
    try {
      await invoke<string>('create_task', {
        questId,
        title: fields.title,
        description: fields.description,
        priority: fields.priority,
        parentTaskId: fields.parentTaskId,
        dependsOn: fields.dependsOn,
      });
    } catch (err) {
      setNotice(`Could not add the task: ${errorText(err)}`);
    } finally {
      await refreshQuest(questId);
    }
  };

  const updateTask = async (taskId: string, fields: TaskFields) => {
    try {
      await invoke('update_task', {
        taskId,
        title: fields.title,
        description: fields.description,
        priority: fields.priority,
        dependsOn: fields.dependsOn,
      });
    } catch (err) {
      setNotice(`Could not save the task: ${errorText(err)}`);
    } finally {
      if (activeQuestId) await refreshQuest(activeQuestId);
    }
  };

  const startTask = async (taskId: string, characterId: string | null) => {
    try {
      await invoke<string>('start_quest_task', { taskId, characterId });
      if (activeQuestId) await refreshQuest(activeQuestId);
      await refreshParty();
    } catch (err) {
      setNotice(`Could not start the agent session: ${errorText(err)}`);
    }
  };

  const saveChecks = async (build: string, test: string, lint: string) => {
    if (!project) return;
    try {
      setProject(
        await invoke<Project>('set_project_checks', {
          projectId: project.id,
          buildCmd: build,
          testCmd: test,
          lintCmd: lint,
        }),
      );
      if (activeQuestId) await refreshQuest(activeQuestId);
    } catch (err) {
      setNotice(`Could not save the checks: ${errorText(err)}`);
    }
  };

  const runChecks = async (taskId: string) => {
    try {
      setCheckReports(await invoke<CheckReport[]>('run_checks', { taskId }));
    } catch (err) {
      setNotice(`Could not run the checks: ${errorText(err)}`);
    } finally {
      if (activeQuestId) await refreshQuest(activeQuestId);
    }
  };

  const autoVerify = async (task: Task, questId: string) => {
    let proj = projectRef.current;
    if (!proj) return;
    try {
      if (!hasChecks(proj)) {
        const found = await invoke<DetectedChecks>('detect_checks', { projectId: proj.id, questId });
        if (hasFound(found)) {
          proj = await invoke<Project>('set_project_checks', {
            projectId: proj.id,
            buildCmd: found.build_cmd,
            testCmd: found.test_cmd,
            lintCmd: found.lint_cmd,
          });
          setProject(proj);
        }
      }
      if (!hasChecks(proj)) {
        setNotice(
          'The agent finished, but no build, test or lint command is set. Add one in Checks, then press Run checks.',
        );
        return;
      }
      setCheckReports(await invoke<CheckReport[]>('run_checks', { taskId: task.id }));
    } catch (err) {
      setNotice(`Could not run the checks: ${errorText(err)}`);
    } finally {
      await refreshQuest(questId);
    }
  };

  useEffect(() => {
    autoVerifyRef.current = autoVerify;
  });

  const detectChecks = async (): Promise<DetectedChecks | null> => {
    if (!project) return null;
    try {
      return await invoke<DetectedChecks>('detect_checks', { projectId: project.id, questId: activeQuestId });
    } catch (err) {
      setNotice(`Could not look for checks: ${errorText(err)}`);
      return null;
    }
  };

  const changeAutoRun = (value: boolean) => {
    setAutoRun(value);
    try {
      localStorage.setItem(AUTO_RUN_KEY, value ? 'on' : 'off');
    } catch {
      // Remembering the choice is only a convenience.
    }
  };

  const loadChanges = async () => {
    if (!activeQuestId) return;
    const questId = activeQuestId;
    try {
      const data = await invoke<WorktreeChanges | null>('get_quest_changes', { questId });
      setChanges({ questId, data });
    } catch (err) {
      setNotice(errorText(err));
    }
  };

  const cancelSession = async (sessionId: string) => {
    try {
      await invoke('cancel_agent_session', { sessionId });
    } catch (err) {
      setNotice(`Could not cancel the session: ${errorText(err)}`);
    }
  };

  const markVerified = async (taskId: string) => {
    try {
      await invoke('mark_task_verified', { taskId });
    } catch (err) {
      setNotice(`Could not mark the task verified: ${errorText(err)}`);
    } finally {
      if (activeQuestId) await refreshQuest(activeQuestId);
    }
  };

  const answerApproval = async (input: 'y' | 'n') => {
    if (!approval) return;
    try {
      await invoke('send_agent_input', { sessionId: approval.sessionId, input });
      setApproval(null);
    } catch (err) {
      setNotice(`Could not send your answer to the agent: ${errorText(err)}`);
    }
  };

  return {
    selectedTask,
    selectTask: setPickedTaskId,
    taskDetail: selectedTaskId ? taskDetail : null,
    changes: changes && changes.questId === activeQuestId ? changes.data : undefined,
    loadChanges,
    project,
    quests,
    activeQuest,
    setActiveQuest,
    tasks,
    characters,
    bossHp: activeQuestId ? bossHp : null,
    events,
    sessionState,
    approval,
    checkReports,
    notice,
    dismissNotice: () => setNotice(null),
    createQuest,
    updateQuest,
    createTask,
    updateTask,
    startTask,
    cancelSession,
    markVerified,
    chooseProject,
    repoIssue,
    initializeRepo,
    dismissRepoIssue: () => setRepoIssue(null),
    saveChecks,
    runChecks,
    detectChecks,
    autoRun,
    changeAutoRun,
    approve: () => answerApproval('y'),
    deny: () => answerApproval('n'),
  };
}
