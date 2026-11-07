import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { LibraryRow } from "../lib/types";
import { FolderIcon, PlayIcon, TrashIcon } from "./Icons";

interface Props {
  onPlay: (row: LibraryRow) => void;
  onToast: (text: string, kind?: "ok" | "err") => void;
  onSearch: () => void;
  refreshKey: number;
}

/** Cached tracks. Deletion needs a second click; everything else is one. */
export const LibraryPage = ({ onPlay, onToast, onSearch, refreshKey }: Props) => {
  const [rows, setRows] = useState<LibraryRow[] | null>(null);
  const [armed, setArmed] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.library());
    } catch (e) {
      onToast(String(e), "err");
      setRows([]);
    }
  }, [onToast]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(null), 3000);
    return () => window.clearTimeout(t);
  }, [armed]);

  return (
    <section className="page">
      <header className="page-head" data-tauri-drag-region>
        <h1>音乐库</h1>
        <p>已缓存的曲目，可离线播放</p>
      </header>

      {rows === null ? (
        <div className="list-meta">加载中…</div>
      ) : rows.length === 0 ? (
        <div className="empty">
          <div className="empty-icon">
            <FolderIcon size={21} />
          </div>
          <h2>还没有缓存的曲目</h2>
          <p>在搜索页点下载或直接播放，曲目会留在本地</p>
          <button className="pill" onClick={onSearch}>
            去搜索
          </button>
        </div>
      ) : (
        <div className="list">
          {rows.map((row) => (
            <div key={row.path} className="row">
              <div
                className="row-art"
                style={row.cover ? { backgroundImage: `url("${row.cover}")` } : undefined}
              />
              <div className="row-text">
                <div className="row-title">{row.title}</div>
                <div className="row-sub">{row.subtitle}</div>
              </div>
              <div className="row-actions">
                <button className="icon-btn solid" title="播放" onClick={() => onPlay(row)}>
                  <PlayIcon size={14} />
                </button>
                <button
                  className="icon-btn"
                  title="在访达中显示"
                  onClick={() =>
                    api.reveal(row.path).catch((e) => onToast(String(e), "err"))
                  }
                >
                  <FolderIcon size={14} />
                </button>
                <button
                  className={`icon-btn${armed === row.path ? " danger" : ""}`}
                  title={armed === row.path ? "再点一次删除" : "从缓存删除"}
                  onClick={async () => {
                    if (armed !== row.path) {
                      setArmed(row.path);
                      return;
                    }
                    try {
                      await api.deleteTrack(row.path);
                      onToast("已删除");
                      void load();
                    } catch (e) {
                      onToast(String(e), "err");
                    } finally {
                      setArmed(null);
                    }
                  }}
                >
                  <TrashIcon size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
};
