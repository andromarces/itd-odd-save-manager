export interface AppConfig {
  save_path: string | null;
  auto_launch_game: boolean;
  auto_close: boolean;
}

export interface BackupInfo {
  path: string;
  filename: string;
  original_filename: string;
  original_path: string;
  size: number;
  modified: string;
  game_number: number;
}

export type StatusType = 'info' | 'success' | 'error';

