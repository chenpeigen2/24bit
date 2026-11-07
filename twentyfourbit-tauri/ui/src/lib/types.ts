/** Shapes returned by the Rust commands. Keep in step with src/main.rs. */

export interface SearchItem {
  id: string;
  cover: string;
  name: string;
  player: string;
  album: string;
}

export interface LyricLine {
  at: number;
  text: string;
}

export interface Prepared {
  id: string;
  lyrics: LyricLine[];
  /** Dominant artwork colour, `[r, g, b]`. */
  accent: [number, number, number];
}

export interface ChartEntry {
  name: string;
  plays: number;
}

export interface WeeklyCharts {
  songs: ChartEntry[];
  artists: ChartEntry[];
}

export interface LibraryRow {
  title: string;
  subtitle: string;
  path: string;
  cover?: string;
}

export interface Settings {
  quality: "master" | "surround";
  files: number;
  bytes: string;
  cache_dir: string;
}

export interface PlayerSnapshot {
  playing: boolean;
  position: number;
  duration: number;
  title: string | null;
  artist: string | null;
  quality: string | null;
  cover: string | null;
}

export interface ProgressPayload {
  id: string;
  done: number;
  total: number | null;
}

export type Page = "search" | "charts" | "library" | "settings";
