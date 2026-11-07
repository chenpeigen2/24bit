import { useCallback, useEffect, useRef, useState } from "react";
import { api, demoSeed, onPlayer } from "../lib/api";
import { applyAccent, applyAmbient } from "../lib/theme";
import type { LyricLine, PlayerSnapshot, SearchItem } from "../lib/types";

const EMPTY: PlayerSnapshot = {
  playing: false,
  position: 0,
  duration: 0,
  title: null,
  artist: null,
  quality: null,
  cover: null,
  engine_ready: true,
  pending: null,
};

export interface PlayerApi {
  snapshot: PlayerSnapshot;
  playingId: string | null;
  /** Set while a track is being fetched and handed to the engine. */
  pendingPlay: string | null;
  lyrics: LyricLine[];
  playing: boolean;
  toggle: () => void;
  step: (delta: number) => void;
  seekFraction: (f: number) => void;
  seekTo: (ms: number) => void;
  restart: () => void;
  volume: number;
  muted: boolean;
  setVolume: (v: number) => void;
  toggleMute: () => void;
  play: (item: SearchItem, queue: SearchItem[]) => Promise<void>;
  playPath: (path: string, cover?: string) => Promise<void>;
}

/**
 * Owns transport state.
 *
 * Playback lives in Rust; this hook mirrors it and keeps the queue, which is a
 * view concern: "next" means "next in the list the user was looking at".
 */
export const usePlayer = (
  quality: string,
  onError: (message: string) => void,
): PlayerApi => {
  // In a plain browser there are no events, so the transport is seeded into a
  // plausible playing state. Headless review needs the real layout, and that
  // layout only appears when something is playing.
  const demo = demoSeed?.results[0] ?? null;
  const [snapshot, setSnapshot] = useState<PlayerSnapshot>(
    demo
      ? {
          playing: true,
          position: 26_000,
          duration: 258_000,
          title: demo.name,
          artist: demo.player,
          quality: "至臻母带",
          cover: demoSeed?.cover ?? null,
          engine_ready: true,
          // `?pending=1` renders the fetching state for review.
          pending: new URLSearchParams(location.search).has("pending")
            ? {
                id: demo.id,
                title: demo.name,
                artist: demo.player,
                cover: demoSeed?.cover ?? "",
                quality: "至臻母带",
                done: 62_000_000,
                total: 156_129_336,
              }
            : null,
        }
      : EMPTY,
  );
  const [playingId, setPlayingId] = useState<string | null>(demo?.id ?? null);
  /**
   * What the user just asked for, before the engine confirms it.
   *
   * The transport is polled at 10Hz, so without this the player bar sits
   * unchanged for a beat after a click — long enough to read as "nothing
   * happened".
   */
  const [pendingPlay, setPendingPlay] = useState<string | null>(null);
  const [lyrics, setLyrics] = useState<LyricLine[]>(demoSeed?.lyrics ?? []);
  const [volume, setVolumeState] = useState(1);
  const [muted, setMuted] = useState(false);

  const queue = useRef<string[]>([]);
  const queueIndex = useRef(-1);
  const lastVolume = useRef(1);
  /** The list the queue was built from, so `next` can resolve an id to a track. */
  const source = useRef<SearchItem[]>([]);

  /** Set while a request this hook started has not been acknowledged yet. */
  const awaiting = useRef<string | null>(null);

  useEffect(
    () =>
      onPlayer((next) => {
        setSnapshot((prev) => {
          // The publisher runs at 10Hz and reports "nothing loaded" until the
          // backend has the track's metadata. Without this merge, that empty
          // snapshot wipes the label the click just produced, and the bar falls
          // back to "选一首歌开始播放" for the length of a metadata round trip.
          if (awaiting.current && !next.title && !next.pending) {
            return { ...next, title: prev.title, artist: prev.artist, cover: prev.cover };
          }
          if (!awaiting.current && (next.title || next.pending)) {
            awaiting.current = null;
          }
          return next;
        });
      }),
    [],
  );

  const refreshGlow = useCallback((cover: string | null) => {
    applyAmbient(cover);
  }, []);

  const play = useCallback(
    async (item: SearchItem, list: SearchItem[]) => {
      if (item.id === playingId) {
        await api.toggle();
        return;
      }

      source.current = list;
      queue.current = list.map((r) => r.id);
      queueIndex.current = list.findIndex((r) => r.id === item.id);

      setPlayingId(item.id);
      setPendingPlay(item.id);
      awaiting.current = item.id;
      setLyrics([]);
      // Show the selection straight away; the engine will confirm.
      setSnapshot((prev) => ({
        ...prev,
        title: item.name,
        artist: item.player,
        cover: item.cover || null,
        playing: true,
        // A new track starts at the beginning. Leaving the old position here
        // would highlight the wrong lyric until the engine caught up.
        position: 0,
        duration: 0,
      }));

      try {
        const prepared = await api.prepare(item.id, quality);
        setLyrics(prepared.lyrics ?? []);
        applyAccent(prepared.accent);
        refreshGlow(item.cover || null);
      } catch (e) {
        onError(String(e));
        setPlayingId(null);
        setSnapshot((prev) => ({ ...prev, playing: false }));
      } finally {
        setPendingPlay(null);
      }
    },
    [playingId, quality, onError, refreshGlow],
  );

  const playPath = useCallback(
    async (path: string, cover?: string) => {
      try {
        await api.playFile(path);
        setPlayingId(path);
        if (cover) {
          applyAccent();
          refreshGlow(cover);
        }
      } catch (e) {
        onError(String(e));
      }
    },
    [onError, refreshGlow],
  );

  const step = useCallback(
    async (delta: number) => {
      const next = queueIndex.current + delta;
      if (next < 0) return onError("已经是第一首");
      if (next >= queue.current.length) return onError("已经是最后一首");

      const id = queue.current[next];
      if (!id) return;
      const item = source.current.find((r) => r.id === id);
      if (!item) return;

      queueIndex.current = next;
      setPlayingId(id);
      setPendingPlay(id);
      awaiting.current = id;
      setSnapshot((prev) => ({
        ...prev,
        title: item.name,
        artist: item.player,
        cover: item.cover || null,
        playing: true,
        // A new track starts at the beginning. Leaving the old position here
        // would highlight the wrong lyric until the engine caught up.
        position: 0,
        duration: 0,
      }));
      try {
        const prepared = await api.prepare(id, quality);
        setLyrics(prepared.lyrics ?? []);
        applyAccent(prepared.accent);
        refreshGlow(item.cover || null);
      } catch (e) {
        onError(String(e));
      } finally {
        setPendingPlay(null);
      }
    },
    [quality, onError, refreshGlow],
  );

  const setVolume = useCallback((v: number) => {
    setVolumeState(v);
    if (v > 0.01) lastVolume.current = v;
    setMuted(v <= 0.01);
    void api.setVolume(v);
  }, []);

  const toggleMute = useCallback(() => {
    const next = !muted;
    setMuted(next);
    const level = next ? 0 : lastVolume.current <= 0.01 ? 0.5 : lastVolume.current;
    if (!next) setVolumeState(level);
    void api.mute(next, lastVolume.current);
  }, [muted]);

  return {
    snapshot,
    playingId,
    pendingPlay,
    lyrics,
    playing: snapshot.playing,
    toggle: () => void api.toggle(),
    step: (d) => void step(d),
    seekFraction: (f) => void api.seek({ fraction: f }),
    seekTo: (ms) => void api.seek({ seconds: ms / 1000 }),
    restart: () => void api.restart(),
    volume,
    muted,
    setVolume,
    toggleMute,
    play,
    playPath,
  };
};
