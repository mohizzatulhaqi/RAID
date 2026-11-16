import { useRaidState } from './state/useRaidState';
import { usePreferences, ThemeChoice } from './state/preferences';
import { statusFromTask } from './state/status';
import { WorldView } from './world/WorldView';
import { BossBar } from './panels/BossBar';
import { QuestBoard } from './panels/QuestBoard';
import { PartyPanel } from './panels/PartyPanel';
import { ActivityLog } from './panels/ActivityLog';
import { ChecksPanel } from './panels/ChecksPanel';
import { TaskDetailPanel } from './panels/TaskDetailPanel';
import { RepoIssuePanel } from './panels/RepoIssuePanel';
import { ApprovalBanner } from './panels/ApprovalBanner';
import { StatusChip } from './panels/StatusChip';

export function App() {
  const raid = useRaidState();
  const prefs = usePreferences();
  // The status shown up top is the task's own, never an animation or event state.
  const ATTENTION = ['running', 'awaiting_approval', 'verification', 'needs_review', 'failed'];
  const focusTask = raid.tasks.find((t) => ATTENTION.includes(t.status)) ?? raid.tasks[0] ?? null;

  return (
    <div className="app">
      <header className="project-bar">
        <strong className="display">RAID</strong>
        <span>
          {raid.project ? raid.project.name : 'No project open'}
          {raid.project && <span className="muted"> on {raid.project.default_branch}</span>}
        </span>
        <button className="btn" onClick={raid.chooseProject}>
          {raid.project ? 'Change project' : 'Open project'}
        </button>
        {focusTask && (
          <span className="row-inline">
            <span className="muted">Task</span>
            <StatusChip status={statusFromTask(focusTask.status)} />
          </span>
        )}
        <span className="spacer" />
        <label>
          Theme
          <select
            value={prefs.theme}
            onChange={(e) => prefs.setTheme(e.target.value as ThemeChoice)}
            className="btn"
          >
            <option value="system">Follow system</option>
            <option value="ledger">Ledger (light)</option>
            <option value="crypt">Crypt (dark)</option>
          </select>
        </label>
        <label>
          <input
            type="checkbox"
            checked={prefs.reduceMotion}
            onChange={(e) => prefs.setReduceMotion(e.target.checked)}
          />
          Reduce motion
        </label>
      </header>

      <div className="world-host">
        <WorldView
          characters={raid.characters}
          bossName={raid.activeQuest?.boss_name ?? 'No active boss'}
          bossHp={raid.bossHp}
          questTitle={raid.activeQuest?.title ?? null}
          approvalText={raid.approval?.action ?? null}
          onApprove={raid.approve}
          onDeny={raid.deny}
          reduceMotion={prefs.reduceMotion}
        />
        {raid.activeQuest && (
          <div className="boss-hud">
            <BossBar name={raid.activeQuest.boss_name ?? 'Unnamed boss'} hp={raid.bossHp} />
          </div>
        )}
      </div>

      <aside className="dock" aria-label="Quest dock">
        {raid.notice && (
          <div className="notice" role="alert">
            {raid.notice}{' '}
            <button className="btn" onClick={raid.dismissNotice}>
              Dismiss
            </button>
          </div>
        )}
        {raid.repoIssue && (
          <RepoIssuePanel
            issue={raid.repoIssue}
            onInitialize={raid.initializeRepo}
            onChooseOther={raid.chooseProject}
            onDismiss={raid.dismissRepoIssue}
          />
        )}
        {raid.approval && (
          <ApprovalBanner actionDescription={raid.approval.action} onApprove={raid.approve} onReject={raid.deny} />
        )}
        {!raid.project && <p className="muted" style={{ padding: 'var(--space-4)' }}>Open a git repository to start. Quests run in their own worktree, not in your main working tree.</p>}
        <PartyPanel characters={raid.characters} />
        <QuestBoard
          quests={raid.quests}
          tasks={raid.tasks}
          activeQuest={raid.activeQuest}
          bossHp={raid.bossHp}
          canCreate={raid.project !== null}
          onSelectQuest={raid.setActiveQuest}
          onCreateQuest={raid.createQuest}
          characters={raid.characters}
          onStartTask={raid.startTask}
          onCancelSession={raid.cancelSession}
          onMarkVerified={raid.markVerified}
          selectedTaskId={raid.selectedTask?.id ?? null}
          onSelectTask={raid.selectTask}
        />
        {raid.selectedTask && (
          <TaskDetailPanel
            key={raid.selectedTask.id}
            task={raid.selectedTask}
            detail={raid.taskDetail}
            changes={raid.changes}
            onLoadChanges={raid.loadChanges}
          />
        )}
        {raid.project && (
          <ChecksPanel
            key={`${raid.project.id}:${raid.project.updated_at}`}
            project={raid.project}
            taskId={raid.tasks.find((t) => t.allowed_actions.includes('run_checks'))?.id ?? null}
            reports={raid.checkReports}
            onSave={raid.saveChecks}
            onRun={raid.runChecks}
            onDetect={raid.detectChecks}
            autoRun={raid.autoRun}
            onAutoRunChange={raid.changeAutoRun}
          />
        )}
      </aside>

      <section className="log-panel" aria-label="Activity log">
        <ActivityLog events={raid.events} />
      </section>
    </div>
  );
}

export default App;
