import { useCallback, useEffect, useState } from "react";
import { api, demoSeed, onProgress } from "./lib/api";
import { bytes } from "./lib/format";
import { applyAccent, applyAmbient } from "./lib/theme";
import type { LibraryRow, Page, SearchItem } from "./lib/types";
import { usePlayer } from "./hooks/usePlayer";
import { useToast } from "./hooks/useToast";
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
  const [page, setPage] = useState<Page>(initialPage);
  const [quality, setQuality] = useState("master");
  const [account, setAccount] = useState<string | null>(null);
  const [cache, setCache] = useState({ files: 0, bytes: "0 B" });
  const [pending, setPending] = useState<string | null>(null);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState("");
  const [libraryKey, setLibraryKey] = useState(0);
  const [searchSeed, setSearchSeed] = useState<string | null>(null);

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
      if (demoSeed) {
        applyAccent(demoSeed.accent);
        applyAmbient(demoSeed.cover);
      } else {
        applyAccent();
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
      setPending(item.id);
      setProgress(0);
      setStatus("准备中…");
      try {
        await player.play(item, list);
      } finally {
        setPending(null);
        setStatus("");
        void refreshCache();
      }
    },
    [player, refreshCache],
  );

  const handleDownload = useCallback(
    async (item: SearchItem) => {
      setPending(item.id);
      setProgress(0);
      setStatus("准备下载…");
      try {
        const path = await api.export(item.id, quality);
        push(`已保存 ${path.split("/").pop() ?? ""}`);
        setLibraryKey((k) => k + 1);
      } catch (e) {
        push(String(e), "err");
      } finally {
        setPending(null);
        setStatus("");
        void refreshCache();
      }
    },
    [quality, push, refreshCache],
  );

  const handleLibraryPlay = useCallback(
    async (row: LibraryRow) => {
      await player.playPath(row.path, row.cover);
      push(`正在播放 ${row.title}`);
    },
    [player, push],
  );

  // ---- global shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = document.activeElement;
      const typing =
        el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
      if (typing) return;

      if (e.code === "Space") {
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
        document.querySelector<HTMLInputElement>(".field input")?.focus();
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
            />
          )}
          {page === "charts" && (
            <ChartsPage
              onPick={(name) => {
                setSearchSeed(name);
                setPage("search");
                // Let the search page mount before driving its input.
                requestAnimationFrame(() => {
                  const input = document.querySelector<HTMLInputElement>(".field input");
                  if (input) {
                    input.value = name;
                    input.dispatchEvent(new Event("input", { bubbles: true }));
                    input.focus();
                  }
                });
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
            />
          )}
        </main>

        <PlayerBar
          snapshot={player.snapshot}
          playing={player.playing}
          volume={player.volume}
          muted={player.muted}
          onToggle={player.toggle}
          onStep={player.step}
          onSeek={player.seekFraction}
          onRestart={player.restart}
          onVolume={player.setVolume}
          onMute={player.toggleMute}
        />

        <div className="status">
          {status ? <span className="dot" /> : null}
          <span>{status}</span>
          <span className="grow" />
          <span className="ver">v0.1.0</span>
        </div>
      </div>

      <Toasts items={toasts} />
    </>
  );
}
