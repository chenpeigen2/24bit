import { useCallback, useEffect, useRef, useState } from "react";
import { api, demoSeed, onPlayer } from "../lib/api";
import { applyAccent, applyAmbient } from "../lib/theme";
import type { LyricLine, PlaybackMode, PlayerSnapshot, SearchItem } from "../lib/types";

const EMPTY: PlayerSnapshot = {
  playing: false,
  ended: false,
  position: 0,
  duration: 0,
  title: null,
  artist: null,
  quality: null,
  cover: null,
  engine_ready: true,
  pending: null,
};

const MODES: PlaybackMode[] = ["sequential", "repeat-all", "repeat-one", "shuffle"];
const MODE_KEY = "24bit.playback-mode";

const initialMode = (): PlaybackMode => {
  const saved = localStorage.getItem(MODE_KEY);
  return MODES.includes(saved as PlaybackMode) ? (saved as PlaybackMode) : "sequential";
};

export interface PlayerApi {
  snapshot: PlayerSnapshot;
  playingId: string | null;
  pendingPlay: string | null;
  lyrics: LyricLine[];
  playing: boolean;
  mode: PlaybackMode;
  cycleMode: () => void;
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
  playPath: (path: string, cover?: string, title?: string, artist?: string) => Promise<void>;
}

/** Owns transport state, playback mode and the view-local queue. */
export const usePlayer = (
  quality: string,
  onError: (message: string) => void,
): PlayerApi => {
  const demo = new URLSearchParams(location.search).get("track") === "last"
    ? (demoSeed?.results.at(-1) ?? null)
    : (demoSeed?.results[0] ?? null);
  const [snapshot, setSnapshot] = useState<PlayerSnapshot>(() => {
    if (!demo) return EMPTY;
    const params = new URLSearchParams(location.search);
    const ended = params.has("ended");
    return {
      playing: !ended,
      ended,
      position: ended ? 258_000 : 26_000,
      duration: 258_000,
      title: demo.name,
      artist: demo.player,
      quality: "至臻母带",
      cover: demoSeed?.cover ?? null,
      engine_ready: true,
      pending: params.has("pending")
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
    };
  });
  const [playingId, setPlayingId] = useState<string | null>(demo?.id ?? null);
  const [pendingPlay, setPendingPlay] = useState<string | null>(null);
  const [lyrics, setLyrics] = useState<LyricLine[]>(demoSeed?.lyrics ?? []);
  const [volume, setVolumeState] = useState(1);
  const [muted, setMuted] = useState(false);
  const [mode, setMode] = useState<PlaybackMode>(initialMode);

  const source = useRef<SearchItem[]>(demoSeed?.results ?? []);
  const queue = useRef<string[]>(demoSeed?.results.map((item) => item.id) ?? []);
  const queueIndex = useRef(
    demo ? (demoSeed?.results.findIndex((item) => item.id === demo.id) ?? 0) : -1,
  );
  const lastVolume = useRef(1);
  const awaiting = useRef<string | null>(null);
  const operation = useRef(0);
  const transportIntent = useRef<{ playing: boolean; at: number } | null>(null);
  const handledEnd = useRef<string | null>(null);

  useEffect(
    () =>
      onPlayer((next) => {
        let incoming = next;
        const intent = transportIntent.current;
        if (intent) {
          if (next.playing === intent.playing) {
            transportIntent.current = null;
          } else if (Date.now() - intent.at < 750) {
            // Do not let an older 10Hz snapshot undo a click before the audio
            // command reaches the engine. The window is bounded on purpose.
            incoming = { ...next, playing: intent.playing };
          } else {
            transportIntent.current = null;
          }
        }

        const pendingId = awaiting.current;
        if (pendingId) {
          if (incoming.pending && incoming.pending.id !== pendingId) return;
          setSnapshot((prev) => {
            if (!incoming.pending) {
              return { ...prev, engine_ready: incoming.engine_ready };
            }
            return {
              ...incoming,
              title: prev.title,
              artist: prev.artist,
              cover: prev.cover,
              quality: prev.quality,
            };
          });
          return;
        }
        setSnapshot((prev) => {
          const transportLoaded = incoming.duration > 0 || incoming.position > 0;
          if (!incoming.title && prev.title && transportLoaded) {
            return {
              ...incoming,
              title: prev.title,
              artist: prev.artist,
              cover: prev.cover,
              quality: prev.quality,
            };
          }
          return incoming;
        });
      }),
    [],
  );

  // The browser fixture uses the same transport state machine without audio.
  // It makes pause, progress and end-of-track behaviour testable rather than a
  // static screenshot that only pretends to play.
  useEffect(() => {
    if (!demoSeed || !snapshot.playing || snapshot.pending || pendingPlay) return;
    const timer = window.setInterval(() => {
      setSnapshot((prev) => {
        if (!prev.playing || prev.duration <= 0) return prev;
        const position = Math.min(prev.duration, prev.position + 100);
        const ended = position >= prev.duration;
        return { ...prev, position, playing: !ended, ended };
      });
    }, 100);
    return () => window.clearInterval(timer);
  }, [demoSeed, pendingPlay, snapshot.pending, snapshot.playing]);

  useEffect(() => {
    localStorage.setItem(MODE_KEY, mode);
  }, [mode]);

  const refreshGlow = useCallback((cover: string | null) => {
    applyAmbient(cover);
  }, []);

  const cycleMode = useCallback(() => {
    setMode((current) => MODES[(MODES.indexOf(current) + 1) % MODES.length] ?? "sequential");
  }, []);

  const toggle = useCallback(() => {
    if (pendingPlay || snapshot.pending || !snapshot.title) return;
    const previous = snapshot;
    const target = snapshot.ended ? true : !snapshot.playing;
    transportIntent.current = { playing: target, at: Date.now() };
    setSnapshot((prev) => ({
      ...prev,
      playing: target,
      ended: false,
      position: prev.ended ? 0 : prev.position,
    }));
    void api
      .toggle(target)
      .then((playing) => {
        if (demoSeed) transportIntent.current = null;
        setSnapshot((prev) => ({ ...prev, playing }));
      })
      .catch((e) => {
        transportIntent.current = null;
        setSnapshot(previous);
        onError(String(e));
      });
  }, [demoSeed, onError, pendingPlay, snapshot]);

  const play = useCallback(
    async (item: SearchItem, list: SearchItem[]) => {
      const previousQueue = {
        source: source.current,
        ids: queue.current,
        index: queueIndex.current,
      };
      source.current = list;
      queue.current = list.map((r) => r.id);
      queueIndex.current = list.findIndex((r) => r.id === item.id);

      if (item.id === playingId) {
        if (!pendingPlay && !snapshot.pending) toggle();
        return;
      }

      const token = ++operation.current;
      const previousSnapshot = snapshot;
      const previousPlayingId = playingId;
      const previousLyrics = lyrics;

      setPlayingId(item.id);
      setPendingPlay(item.id);
      awaiting.current = item.id;
      setLyrics([]);
      setSnapshot((prev) => ({
        ...prev,
        title: item.name,
        artist: item.player,
        cover: item.cover || null,
        quality: null,
        pending: null,
        playing: true,
        ended: false,
        position: 0,
        duration: 0,
      }));

      try {
        const prepared = await api.prepare(item.id, quality);
        if (token !== operation.current) return;
        awaiting.current = null;
        setLyrics(prepared.lyrics ?? []);
        applyAccent(prepared.accent);
        refreshGlow(item.cover || null);
        // The browser fixture has no backend event to publish the new length.
        if (demoSeed) {
          setSnapshot((prev) => ({ ...prev, duration: 258_000, position: 0, playing: true }));
        }
      } catch (e) {
        if (token !== operation.current) return;
        awaiting.current = null;
        onError(String(e));
        source.current = previousQueue.source;
        queue.current = previousQueue.ids;
        queueIndex.current = previousQueue.index;
        setPlayingId(previousPlayingId);
        setLyrics(previousLyrics);
        setSnapshot(previousSnapshot);
      } finally {
        if (token === operation.current) setPendingPlay(null);
      }
    },
    [demoSeed, lyrics, onError, pendingPlay, playingId, quality, refreshGlow, snapshot, toggle],
  );

  const playPath = useCallback(
    async (path: string, cover?: string, title?: string, artist?: string) => {
      const token = ++operation.current;
      setPendingPlay(path);
      setLyrics([]);
      awaiting.current = path;
      setSnapshot((prev) => ({
        ...prev,
        title: title ?? path.split("/").pop() ?? path,
        artist: artist ?? "本地文件",
        cover: cover ?? null,
        quality: null,
        pending: null,
        position: 0,
        duration: 0,
        playing: true,
        ended: false,
      }));
      try {
        await api.playFile(path);
        if (token !== operation.current) return;
        awaiting.current = null;
        setPlayingId(path);
        source.current = [];
        queue.current = [];
        queueIndex.current = -1;
        applyAccent();
        refreshGlow(cover ?? null);
        if (demoSeed) setSnapshot((prev) => ({ ...prev, duration: 258_000 }));
      } catch (e) {
        if (token === operation.current) {
          awaiting.current = null;
          onError(String(e));
          setSnapshot((prev) => ({ ...prev, playing: false }));
        }
      } finally {
        if (token === operation.current) setPendingPlay(null);
      }
    },
    [demoSeed, onError, refreshGlow],
  );

  const restart = useCallback(() => {
    if (snapshot.pending || pendingPlay || !snapshot.title) return;
    transportIntent.current = { playing: true, at: Date.now() };
    setSnapshot((prev) => ({ ...prev, position: 0, playing: true, ended: false }));
    void api
      .restart()
      .then(() => {
        if (demoSeed) transportIntent.current = null;
      })
      .catch((e) => {
        transportIntent.current = null;
        onError(String(e));
      });
  }, [demoSeed, onError, pendingPlay, snapshot.pending, snapshot.title]);

  const step = useCallback(
    async (delta: number, automatic = false) => {
      if (!automatic && delta < 0 && snapshot.position > 3000) {
        restart();
        return;
      }

      const items = source.current;
      if (items.length === 0) {
        if (!automatic) onError("当前没有播放队列");
        return;
      }

      const current = Math.max(0, queueIndex.current);
      let next = current + delta;
      if (mode === "shuffle") {
        if (items.length > 1) {
          next = current;
          while (next === current) next = Math.floor(Math.random() * items.length);
        }
      } else if (next < 0 || next >= items.length) {
        if (mode === "repeat-all") {
          next = (next + items.length) % items.length;
        } else if (automatic) {
          return; // sequential reaches a real stop after the final track
        } else {
          onError(next < 0 ? "已经是第一首" : "已经是最后一首");
          return;
        }
      }

      const item = items[next];
      if (!item) return;
      if (item.id === playingId) {
        restart(); // a one-track loop/next must restart, not toggle/pause
        return;
      }
      await play(item, items);
    },
    [mode, onError, play, playingId, restart, snapshot.position],
  );

  // EOF is explicit in PlayerSnapshot. A paused song near its end must never
  // trigger queue advancement; only the engine's Finished phase does.
  useEffect(() => {
    if (!snapshot.ended) {
      handledEnd.current = null;
      return;
    }
    if (pendingPlay || snapshot.pending) return;
    const key = `${playingId ?? snapshot.title ?? "track"}:${mode}`;
    if (handledEnd.current === key) return;
    handledEnd.current = key;

    if (mode === "repeat-one") {
      restart();
      return;
    }
    if (source.current.length > 0) void step(1, true);
  }, [mode, pendingPlay, playingId, restart, snapshot.ended, snapshot.pending, snapshot.title, step]);

  const setVolume = useCallback(
    (value: number) => {
      const v = Math.max(0, Math.min(1, value));
      setVolumeState(v);
      if (v > 0.01) lastVolume.current = v;
      setMuted(v <= 0.01);
      void api.setVolume(v).catch((e) => onError(String(e)));
    },
    [onError],
  );

  const toggleMute = useCallback(() => {
    const next = !muted;
    setMuted(next);
    const level = next ? 0 : lastVolume.current <= 0.01 ? 0.5 : lastVolume.current;
    if (!next) setVolumeState(level);
    void api.mute(next, lastVolume.current).catch((e) => onError(String(e)));
  }, [muted, onError]);

  const seekFraction = useCallback(
    (fraction: number) => {
      if (snapshot.pending || pendingPlay || !snapshot.duration) return;
      const f = Math.max(0, Math.min(1, fraction));
      setSnapshot((prev) => ({
        ...prev,
        position: f * prev.duration,
        ended: false,
      }));
      void api.seek({ fraction: f }).catch((e) => onError(String(e)));
    },
    [onError, pendingPlay, snapshot.duration, snapshot.pending],
  );

  const seekTo = useCallback(
    (ms: number) => {
      if (snapshot.pending || pendingPlay || !snapshot.duration) return;
      const position = Math.max(0, Math.min(snapshot.duration, ms));
      setSnapshot((prev) => ({ ...prev, position, ended: false }));
      void api.seek({ seconds: position / 1000 }).catch((e) => onError(String(e)));
    },
    [onError, pendingPlay, snapshot.duration, snapshot.pending],
  );

  return {
    snapshot,
    playingId,
    pendingPlay,
    lyrics,
    playing: snapshot.playing,
    mode,
    cycleMode,
    toggle,
    step: (d) => void step(d),
    seekFraction,
    seekTo,
    restart,
    volume,
    muted,
    setVolume,
    toggleMute,
    play,
    playPath,
  };
};
