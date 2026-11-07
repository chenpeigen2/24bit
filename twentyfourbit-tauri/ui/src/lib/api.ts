/**
 * The only seam to Rust.
 *
 * In the packaged app these hit Tauri commands. In a plain browser (used for
 * headless layout review) they fall back to fixed data so the interface can be
 * rendered without a window — the same code path, different data.
 */

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type {
  LibraryRow,
  PlayerSnapshot,
  Prepared,
  ProgressPayload,
  SearchItem,
  Settings,
  WeeklyCharts,
} from "./types";
import { demo } from "./demo";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Seed data for headless review. `null` in the packaged app. */
export const demoSeed = isTauri ? null : demo;

const invoke = async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
  if (isTauri) return tauriInvoke<T>(cmd, args);
  return demoInvoke<T>(cmd, args);
};

export const api = {
  search: (keyword: string) => invoke<SearchItem[]>("search", { keyword }),
  searchPage: (keyword: string, page: number) =>
    invoke<SearchItem[]>("search_page", { keyword, page }),
  charts: () => invoke<WeeklyCharts>("charts"),
  prepare: (id: string, quality: string) => invoke<Prepared>("prepare", { id, quality }),
  export: (id: string, quality: string) => invoke<string>("export", { id, quality }),
  library: () => invoke<LibraryRow[]>("library"),
  clearCache: () => invoke<void>("clear_cache"),
  settings: () => invoke<Settings>("settings"),
  setQuality: (quality: string) => invoke<void>("set_quality", { quality }),
  login: (account: string, password: string) =>
    invoke<string>("login", { account, password }),
  whoami: () => invoke<string | null>("whoami"),
  deleteTrack: (path: string) => invoke<void>("delete_track", { path }),
  reveal: (path: string) => invoke<void>("reveal", { path }),
  mute: (muted: boolean, restore: number) => invoke<number>("mute", { muted, restore }),
  toggle: () => invoke<void>("toggle"),
  pause: () => invoke<void>("pause"),
  restart: () => invoke<void>("restart"),
  seek: (args: { seconds?: number; fraction?: number }) => invoke<void>("seek", args),
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  playFile: (path: string) => invoke<void>("play_file", { path }),
};

export const onPlayer = (fn: (snapshot: PlayerSnapshot) => void): (() => void) => {
  if (!isTauri) return () => {};
  let unlisten: (() => void) | undefined;
  void tauriListen<PlayerSnapshot>("player", (e) => fn(e.payload)).then((u) => {
    unlisten = u;
  });
  return () => unlisten?.();
};

export const onProgress = (fn: (payload: ProgressPayload) => void): (() => void) => {
  if (!isTauri) return () => {};
  let unlisten: (() => void) | undefined;
  void tauriListen<ProgressPayload>("progress", (e) => fn(e.payload)).then((u) => {
    unlisten = u;
  });
  return () => unlisten?.();
};

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function demoInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await wait(90);
  switch (cmd) {
    case "search":
      return demo.results as unknown as T;
    case "search_page":
      return [] as unknown as T;
    case "charts":
      return demo.charts as unknown as T;
    case "library":
      return demo.library as unknown as T;
    case "settings":
      return demo.settings as unknown as T;
    case "prepare":
      return {
        id: args?.id ?? demo.results[0]?.id,
        lyrics: demo.lyrics,
        accent: demo.accent,
      } as unknown as T;
    case "whoami":
      return null as unknown as T;
    default:
      return true as unknown as T;
  }
}
