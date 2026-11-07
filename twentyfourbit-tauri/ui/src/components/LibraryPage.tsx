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

/** Cached tracks as an artwork grid, rather than three rows lost in blank space. */
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
    <section className="page library-page">
      <header className="page-head" data-tauri-drag-region>
        <h1>音乐库</h1>
        <p>已缓存的曲目，可离线播放</p>
      </header>

      {rows !== null && rows.length > 0 && (
        <div className="library-summary">
          <strong>{rows.length} 首曲目</strong>
          <span>保存在本机，无需联网即可播放</span>
        </div>
      )}

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
        <div className="library-grid">
          {rows.map((row) => (
            <article key={row.path} className="library-card">
              <button
                type="button"
                className="library-cover"
                style={row.cover ? { backgroundImage: `url("${row.cover}")` } : undefined}
                aria-label={`播放 ${row.title}`}
                onClick={() => onPlay(row)}
              >
                <span className="library-play"><PlayIcon size={20} /></span>
              </button>
              <div className="library-card-text">
                <div className="row-title" title={row.title}>{row.title}</div>
                <div className="row-sub" title={row.subtitle}>{row.subtitle}</div>
              </div>
              <div className="library-card-actions">
                <button
                  type="button"
                  onClick={() => api.reveal(row.path).catch((e) => onToast(String(e), "err"))}
                >
                  <FolderIcon size={13} /> 显示
                </button>
                <button
                  type="button"
                  className={armed === row.path ? "danger" : ""}
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
                  <TrashIcon size={13} /> {armed === row.path ? "确认" : "删除"}
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
};
