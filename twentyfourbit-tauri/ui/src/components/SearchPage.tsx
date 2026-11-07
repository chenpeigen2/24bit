import { useEffect, useRef, useState } from "react";
import { api, demoSeed } from "../lib/api";
import type { SearchItem } from "../lib/types";
import { DownloadIcon, PauseIcon, PlayIcon, ProgressRing, SearchIcon } from "./Icons";
import { Inspector } from "./Inspector";
import type { LyricLine } from "../lib/types";

interface Props {
  onPlay: (item: SearchItem, list: SearchItem[]) => void;
  onDownload: (item: SearchItem) => void;
  onShowCharts: () => void;
  playingId: string | null;
  pendingId: string | null;
  progress: number;
  lyrics: LyricLine[];
  position: number;
  onSeek: (ms: number) => void;
  /** A chart entry supplies the first query without mutating the DOM behind React. */
  initialQuery?: string;
}

/** Debounced search over both catalogues, with a two-pane result view. */
export const SearchPage = ({
  onPlay,
  onDownload,
  onShowCharts,
  playingId,
  pendingId,
  progress,
  lyrics,
  position,
  onSeek,
  initialQuery,
}: Props) => {
  // In a plain browser the page opens pre-populated so the layout can be
  // reviewed without typing; the packaged app starts empty.
  const [query, setQuery] = useState(
    initialQuery ?? (demoSeed?.results[0] ? "富士山下" : ""),
  );
  const [results, setResults] = useState<SearchItem[]>(
    initialQuery ? [] : (demoSeed?.results ?? []),
  );
  const [selected, setSelected] = useState(0);
  const [searching, setSearching] = useState(Boolean(initialQuery));
  const [hasMore, setHasMore] = useState(false);
  const [page, setPage] = useState(1);
  const [error, setError] = useState<string | null>(null);

  const timer = useRef<number | undefined>(undefined);
  // Every new intent invalidates older requests. A network response is only
  // allowed to render while it still belongs to the current query.
  const request = useRef(0);

  const run = async (keyword: string) => {
    const id = ++request.current;
    if (!keyword) {
      setSearching(false);
      setError(null);
      setResults([]);
      setSelected(0);
      setHasMore(false);
      setPage(1);
      return;
    }

    setSearching(true);
    setError(null);
    setHasMore(false);
    try {
      const found = await api.search(keyword);
      if (id !== request.current) return;
      setResults(found);
      setSelected(0);
      setPage(1);
      setHasMore(found.length >= 30);
    } catch (e) {
      if (id !== request.current) return;
      setResults([]);
      setSelected(0);
      setError(String(e));
      setHasMore(false);
    } finally {
      if (id === request.current) setSearching(false);
    }
  };

  // This component is keyed by the chart query in App. StrictMode may run the
  // effect twice in development; the request generation keeps the first result
  // from winning, and cleanup also prevents updates after unmount.
  useEffect(() => {
    if (initialQuery?.trim()) void run(initialQuery.trim());
    return () => {
      request.current += 1;
      window.clearTimeout(timer.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const clear = () => {
    request.current += 1;
    window.clearTimeout(timer.current);
    setQuery("");
    setResults([]);
    setSelected(0);
    setSearching(false);
    setHasMore(false);
    setPage(1);
    setError(null);
  };

  const loadMore = async () => {
    if (searching) return;
    const id = ++request.current;
    setSearching(true);
    setError(null);
    try {
      const next = await api.searchPage(query, page + 1);
      if (id !== request.current) return;
      // The two catalogues overlap between pages, so de-duplicate on the pair
      // the site itself treats as identity.
      const seen = new Set(results.map((r) => `${r.name}\u0000${r.player}`));
      const merged = [...results];
      for (const item of next) {
        const key = `${item.name}\u0000${item.player}`;
        if (!seen.has(key)) {
          seen.add(key);
          merged.push(item);
        }
      }
      setResults(merged);
      setPage((p) => p + 1);
      setHasMore(next.length >= 30);
    } catch (e) {
      if (id === request.current) setError(String(e));
    } finally {
      if (id === request.current) setSearching(false);
    }
  };

  const current = results[selected];
  const upNext = [
    ...results.slice(selected + 1),
    ...results.slice(0, Math.max(0, selected)),
  ].slice(0, 3);

  return (
    <section className="page">
      <header className="page-head" data-tauri-drag-region>
        <h1>搜索</h1>
        <p>同时检索两个曲库，结果自动合并去重</p>
      </header>

      <div className="search-row">
        <div className="field">
          <SearchIcon size={14} />
          <input
            id="q"
            aria-label="搜索歌曲或歌手"
            autoFocus={Boolean(initialQuery)}
            value={query}
            placeholder="歌曲 / 歌手"
            spellCheck={false}
            onChange={(e) => {
              const value = e.target.value;
              request.current += 1;
              window.clearTimeout(timer.current);
              setQuery(value);
              setError(null);
              setSearching(false);
              if (!value.trim()) {
                setResults([]);
                setSelected(0);
                setHasMore(false);
                setPage(1);
                return;
              }
              timer.current = window.setTimeout(() => void run(value.trim()), 320);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                window.clearTimeout(timer.current);
                void run(query.trim());
              }
              if (e.key === "Escape") {
                clear();
                e.currentTarget.blur();
              }
            }}
          />
          {query && (
            <button className="field-clear" type="button" aria-label="清空搜索" onClick={clear}>
              ×
            </button>
          )}
        </div>
      </div>

      <div className="split">
        <div className="list-wrap">
          <div className="list-meta" aria-live="polite">
            {searching ? "正在搜索…" : results.length > 0 ? `${results.length} 个结果` : ""}
          </div>

          {results.length === 0 ? (
            <div className="empty">
              <div className="empty-icon">
                <SearchIcon size={21} />
              </div>
              <h2>
                {searching
                  ? "正在搜索…"
                  : error
                    ? "搜索失败"
                    : query
                      ? "没有找到匹配的歌曲"
                      : "输入关键词开始搜索"}
              </h2>
              <p>
                {searching
                  ? "正在合并两个曲库的结果"
                  : error
                    ? error
                    : query
                      ? "换个写法试试，或者直接浏览本周榜单"
                      : "试试歌名或歌手，例如「富士山下」「陈奕迅」"}
              </p>
              {!searching && (
                <button
                  className="pill"
                  onClick={error ? () => void run(query.trim()) : onShowCharts}
                >
                  {error ? "重试" : "浏览本周榜单"}
                </button>
              )}
            </div>
          ) : (
            <div className="list">
              {results.map((item, index) => (
                <Row
                  key={item.id}
                  item={item}
                  selected={index === selected}
                  playing={item.id === playingId}
                  pending={item.id === pendingId}
                  progress={progress}
                  onSelect={() => setSelected(index)}
                  onPlay={() => onPlay(item, results)}
                  onDownload={() => onDownload(item)}
                />
              ))}
              {hasMore && (
                <button className="load-more" onClick={loadMore} disabled={searching}>
                  {searching ? "加载中…" : "加载更多结果"}
                </button>
              )}
            </div>
          )}
        </div>

        {current && (
          <Inspector
            item={current}
            playing={current.id === playingId}
            lyrics={current.id === playingId ? lyrics : []}
            position={position}
            onPlay={() => onPlay(current, results)}
            onDownload={() => onDownload(current)}
            onSeek={onSeek}
            upNext={upNext}
            onPick={(item) => {
              const index = results.findIndex((result) => result.id === item.id);
              if (index >= 0) setSelected(index);
            }}
          />
        )}
      </div>
    </section>
  );
};

interface RowProps {
  item: SearchItem;
  selected: boolean;
  playing: boolean;
  pending: boolean;
  progress: number;
  onSelect: () => void;
  onPlay: () => void;
  onDownload: () => void;
}

const Row = ({
  item,
  selected,
  playing,
  pending,
  progress,
  onSelect,
  onPlay,
  onDownload,
}: RowProps) => (
  <div
    className={[
      "row",
      selected && "is-selected",
      playing && "is-playing",
      pending && "is-pending",
    ]
      .filter(Boolean)
      .join(" ")}
    onClick={onSelect}
    onDoubleClick={onPlay}
  >
    <div
      className="row-art"
      aria-hidden="true"
      style={item.cover ? { backgroundImage: `url("${item.cover}")` } : undefined}
    />
    <div className="row-text">
      <div className="row-title">{item.name}</div>
      <div className="row-sub">
        {item.player} · {item.album}
      </div>
    </div>
    <div className="row-actions">
      {pending ? (
        <ProgressRing fraction={progress} />
      ) : (
        <button
          type="button"
          className={`icon-btn${playing ? "" : " solid"}`}
          title={playing ? "暂停" : "播放"}
          onClick={(e) => {
            e.stopPropagation();
            onSelect();
            onPlay();
          }}
        >
          {playing ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
        </button>
      )}
      <button
        type="button"
        className="icon-btn"
        title="下载到本地"
        disabled={pending}
        onClick={(e) => {
          e.stopPropagation();
          onDownload();
        }}
      >
        <DownloadIcon size={14} />
      </button>
    </div>
  </div>
);
