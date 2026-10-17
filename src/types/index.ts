export type PresentationState =
  | 'idle'
  | 'entering'
  | 'exploring'
  | 'planning'
  | 'fighting'
  | 'investigating'
  | 'needs_approval'
  | 'verification'
  | 'returning'
  | 'succeeded'
  | 'failed'
  | 'cancelled';

export interface Project {
  id: string;
  name: string;
  repository_path: string;
  default_branch: string;
  build_cmd?: string;
  test_cmd?: string;
  lint_cmd?: string;
  created_at: string;
  updated_at: string;
}

export interface Quest {
  id: string;
  project_id: string;
  title: string;
  description: string;
  status: 'created' | 'ready' | 'active' | 'completed' | 'failed';
  priority: 'low' | 'medium' | 'high' | 'urgent';
  boss_name: string;
  boss_max_hp: number;
  boss_current_hp: number;
  created_at: string;
  completed_at?: string;
}

export interface Task {
  id: string;
  quest_id: string;
  parent_task_id?: string;
  title: string;
  description: string;
  status: 'created' | 'ready' | 'assigned' | 'running' | 'verification' | 'completed' | 'blocked' | 'awaiting_approval' | 'failed' | 'cancelled';
  assigned_character_id?: string;
  priority: 'low' | 'medium' | 'high' | 'urgent';
  weight_percent: number;
  created_at: string;
  updated_at: string;
}

export interface Character {
  id: string;
  display_name: string;
  class: 'warrior' | 'mage' | 'rogue' | 'cleric' | 'ranger' | 'paladin' | 'engineer' | 'bard' | 'summoner';
  party_role: 'tank' | 'dps' | 'healer' | 'support' | 'controller' | 'scout' | 'utility';
  level: number;
  experience: number;
  appearance_config: string;
  state: PresentationState;
  created_at: string;
}

export interface NormalizedEvent {
  session_id: string;
  timestamp: string;
  presentation_state: PresentationState;
  payload: {
    type: string;
    data?: any;
  };
}
