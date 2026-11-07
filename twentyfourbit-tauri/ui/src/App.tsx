import { useCallback, useEffect, useRef, useState } from "react";
import { api, demoSeed, onProgress } from "./lib/api";
import { bytes } from "./lib/format";
import { applyAmbient } from "./lib/theme";
import type { LibraryRow, Page, SearchItem } from "./lib/types";
import { usePlayer } from "./hooks/usePlayer";
import { useToast } from "./hooks/useToast";
import { useTheme } from "./hooks/useTheme";
import { Sidebar } from "./components/Sidebar";
import { SearchPage } from "./components/SearchPage";
import { ChartsPage } from "./components/ChartsPage";
import { LibraryPage } from "./components/LibraryPage";
import { SettingsPage } from "./components/SettingsPage";
import { PlayerBar } from "./components/PlayerBar";
import { Toasts } from "./components/Toasts";

const initialPage = (): Page => {
  const want = new URLSearchParams(location.search).get("page");
  return want === "charts" || want === "library" || want === "settings" ? want : "search";
};

export default function App() {
  const { toasts, push } = useToast();
  const { theme, setTheme } = useTheme();
  const [page, setPage] = useState<Page>(initialPage);
  const [quality, setQuality] = useState("master");
  const [account, setAccount] = useState<string | null>(null);
  const [cache, setCache] = useState({ files: 0, bytes: "0 B" });
  const [pending, setPending] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState("");
  const [libraryKey, setLibraryKey] = useState(0);
  const [searchSeed, setSearchSeed] = useState<string | null>(null);
  const pendingOperation = useRef(0);

  const player = usePlayer(quality, (m) => push(m, "err"));

  // ---- boot: config, account, cache size
  useEffect(() => {
    void (async () => {
      try {
        const s = await api.settings();
        setQuality(s.quality);
        setCache({ files: s.files, bytes: s.bytes });
      } catch {
        /* first run: nothing cached yet */
      }
      try {
        setAccount(await api.whoami());
      } catch {
        setAccount(null);
      }
      // The first paint already carries the accent (see main.tsx); this only
      // fills in an empty state when nothing is loaded.
      if (!demoSeed) {
        applyAmbient(null);
      }
    })();
  }, []);

  // ---- download progress
  useEffect(
    () =>
      onProgress((p) => {
        if (p.id !== pending) return;
        const fraction = p.total ? p.done / p.total : 0;
        setProgress(fraction);
        setStatus(
          p.total
            ? `下载中 ${Math.round(fraction * 100)}% · ${bytes(p.done)} / ${bytes(p.total)}`
            : `下载中 ${bytes(p.done)}`,
        );
      }),
    [pending],
  );

  const refreshCache = useCallback(async () => {
    try {
      const s = await api.settings();
      setCache({ files: s.files, bytes: s.bytes });
    } catch {
      /* ignore */
    }
  }, []);

  const handlePlay = useCallback(
    async (item: SearchItem, list: SearchItem[]) => {
      const operation = ++pendingOperation.current;
      setPending(item.id);
      setProgress(0);
      setStatus("准备中…");
      try {
        await player.play(item, list);
      } finally {
        if (operation === pendingOperation.current) {
          setPending(null);
          setStatus("");
        }
        void refreshCache();
      }
    },
    [player, refreshCache],
  );

  const handleDownload = useCallback(
    async (item: SearchItem) => {
      const operation = ++pendingOperation.current;
      setPending(item.id);
      setProgress(0);
      setStatus("准备下载…");
      try {
        const path = await api.export(item.id, quality);
        if (operation !== pendingOperation.current) return;
        push(`已保存 ${path.split("/").pop() ?? ""}`);
        setLibraryKey((k) => k + 1);
      } catch (e) {
        if (operation === pendingOperation.current) push(String(e), "err");
      } finally {
        if (operation === pendingOperation.current) {
          setPending(null);
          setStatus("");
        }
        void refreshCache();
      }
    },
    [quality, push, refreshCache],
  );

  const handleLibraryPlay = useCallback(
    async (row: LibraryRow) => {
      await player.playPath(row.path, row.cover, row.title, row.subtitle);
      push(`正在播放 ${row.title}`);
    },
    [player, push],
  );

  // ---- global shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = document.activeElement;
      const typing =
        el instanceof HTMLElement &&
        (el.matches("input, textarea, select") || el.isContentEditable);
      if (typing) return;

      // Space on a focused control belongs to that control. Treating it as a
      // global transport shortcut also cancels the button's native click.
      const interactive = el instanceof HTMLElement && el.matches("button, a, [role='slider']");
      if (e.code === "Space" && !interactive) {
        e.preventDefault();
        player.toggle();
      } else if (e.key === "ArrowRight" && e.shiftKey) {
        e.preventDefault();
        player.step(1);
      } else if (e.key === "ArrowLeft" && e.shiftKey) {
        e.preventDefault();
        player.step(-1);
      } else if (e.key === "/") {
        e.preventDefault();
        setPage("search");
        // The input is mounted by the state update, not before it.
        requestAnimationFrame(() => {
          document.querySelector<HTMLInputElement>(".field input")?.focus();
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [player]);

  const navigate = (next: Page) => {
    setPage(next);
    if (next === "library") setLibraryKey((k) => k + 1);
  };

  return (
    <>
      <div className="ambient ambient-far" id="ambient-far" aria-hidden="true" />
      <div className="ambient ambient-near" id="ambient-near" aria-hidden="true" />

      <div className="window">
        <Sidebar
          page={page}
          onNavigate={navigate}
          cacheLine={`${cache.files} 个文件 · ${cache.bytes}`}
          account={account}
          snapshot={player.snapshot}
        />

        <main className="content">
          {page === "search" && (
            <SearchPage
              key={searchSeed ?? "search"}
              onPlay={(item, list) => void handlePlay(item, list)}
              onDownload={(item) => void handleDownload(item)}
              onShowCharts={() => navigate("charts")}
              playingId={player.playingId}
              pendingId={pending}
              progress={progress}
              lyrics={player.lyrics}
              position={player.snapshot.position}
              onSeek={player.seekTo}
              initialQuery={searchSeed ?? undefined}
            />
          )}
          {page === "charts" && (
            <ChartsPage
              onPick={(name) => {
                setSearchSeed(name);
                setPage("search");
              }}
            />
          )}
          {page === "library" && (
            <LibraryPage
              onPlay={(row) => void handleLibraryPlay(row)}
              onToast={push}
              onSearch={() => setPage("search")}
              refreshKey={libraryKey}
            />
          )}
          {page === "settings" && (
            <SettingsPage
              quality={quality}
              onQuality={setQuality}
              onToast={push}
              account={account}
              onAccount={setAccount}
              theme={theme}
              onTheme={setTheme}
              playbackMode={player.mode}
              onCyclePlaybackMode={player.cycleMode}
            />
          )}
        </main>

        <PlayerBar
          snapshot={player.snapshot}
          playing={player.playing}
          mode={player.mode}
          volume={player.volume}
          muted={player.muted}
          onToggle={player.toggle}
          onStep={player.step}
          onCycleMode={player.cycleMode}
          onSeek={player.seekFraction}
          onVolume={player.setVolume}
          onMute={player.toggleMute}
        />

        <div className="status">
          {status ? <span className="dot" /> : null}
          {!player.snapshot.engine_ready && (
            <span className="status-error">没有可用的音频输出设备</span>
          )}
          <span>{status}</span>
          <span className="grow" />
          <span className="ver">v0.1.0</span>
        </div>
      </div>

      <Toasts items={toasts} />
    </>
  );
}
