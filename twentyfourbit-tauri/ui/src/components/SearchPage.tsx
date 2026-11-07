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
}: Props) => {
  // In a plain browser the page opens pre-populated so the layout can be
  // reviewed without typing; the packaged app starts empty.
  const [query, setQuery] = useState(demoSeed?.results[0] ? "富士山下" : "");
  const [results, setResults] = useState<SearchItem[]>(demoSeed?.results ?? []);
  const [selected, setSelected] = useState(0);
  const [searching, setSearching] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [page, setPage] = useState(1);
  const [error, setError] = useState<string | null>(null);

  const timer = useRef<number | undefined>(undefined);

  const run = async (keyword: string) => {
    if (!keyword) return;
    setSearching(true);
    setError(null);
    setResults([]);
    setSelected(0);
    setPage(1);
    try {
      const found = await api.search(keyword);
      setResults(found);
      setHasMore(found.length >= 30);
    } catch (e) {
      setError(String(e));
      setHasMore(false);
    } finally {
      setSearching(false);
    }
  };

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const loadMore = async () => {
    setSearching(true);
    try {
      const next = await api.searchPage(query, page + 1);
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
      setError(String(e));
    } finally {
      setSearching(false);
    }
  };

  const current = results[selected];

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
            value={query}
            placeholder="歌曲 / 歌手"
            spellCheck={false}
            onChange={(e) => {
              const value = e.target.value;
              setQuery(value);
              window.clearTimeout(timer.current);
              if (!value.trim()) {
                setResults([]);
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
                setQuery("");
                setResults([]);
                e.currentTarget.blur();
              }
            }}
          />
        </div>
      </div>

      <div className="split">
        <div className="list-wrap">
          <div className="list-meta">
            {results.length > 0 && `${results.length} 个结果`}
          </div>

          {results.length === 0 ? (
            <div className="empty">
              <div className="empty-icon">
                <SearchIcon size={21} />
              </div>
              <h2>
                {searching ? "正在搜索…" : query ? "没有找到匹配的歌曲" : error ? "搜索失败" : "输入关键词开始搜索"}
              </h2>
              <p>
                {searching
                  ? ""
                  : query
                    ? "换个写法试试，或者直接浏览本周榜单"
                    : "试试歌名或歌手，例如「富士山下」「陈奕迅」"}
              </p>
              {!searching && (
                <button className="pill" onClick={onShowCharts}>
                  浏览本周榜单
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
          className={`icon-btn${playing ? "" : " solid"}`}
          title={playing ? "暂停" : "播放"}
          onClick={(e) => {
            e.stopPropagation();
            onPlay();
          }}
        >
          {playing ? <PauseIcon size={14} /> : <PlayIcon size={14} />}
        </button>
      )}
      <button
        className="icon-btn"
        title="下载到本地"
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
