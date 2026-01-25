export interface AppConfig {
  save_path: string | null;
  auto_launch_game: boolean;
  auto_close: boolean;
  max_backups_per_game: number;
}

export interface BackupInfo {
  path: string;
  filename: string;
  original_filename: string;
  original_path: string;
  size: number;
  modified: string;
  game_number: number;
  locked: boolean;
}

export type StatusType = 'info' | 'success' | 'error';

