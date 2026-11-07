import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { ChartEntry, WeeklyCharts } from "../lib/types";

interface Props {
  onPick: (name: string) => void;
}

/** Weekly songs and artists, side by side, click to search. */
export const ChartsPage = ({ onPick }: Props) => {
  const [charts, setCharts] = useState<WeeklyCharts | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .charts()
      .then((c) => alive && setCharts(c))
      .catch(() => alive && setFailed(true));
    return () => {
      alive = false;
    };
  }, []);

  return (
    <section className="page">
      <header className="page-head" data-tauri-drag-region>
        <h1>本周榜单</h1>
        <p>24bit.net 每周播放统计</p>
      </header>

      {!charts ? (
        <div className="list-meta">{failed ? "榜单加载失败" : "加载中…"}</div>
      ) : (
        <div className="charts">
          <Column title="歌曲" rows={charts.songs} onPick={onPick} />
          <Column title="歌手" rows={charts.artists} onPick={onPick} />
        </div>
      )}
    </section>
  );
};

const Column = ({
  title,
  rows,
  onPick,
}: {
  title: string;
  rows: ChartEntry[];
  onPick: (name: string) => void;
}) => (
  <div className="chart">
    <h3>{title}</h3>
    {rows.map((row, i) => (
      <button
        type="button"
        key={`${row.name}-${i}`}
        className="chart-row"
        onClick={() => onPick(row.name)}
        aria-label={`搜索${title === "歌手" ? "歌手" : "歌曲"}${row.name}，本周播放 ${row.plays} 次`}
      >
        <span className="chart-rank">{i + 1}</span>
        <span className="chart-name">{row.name}</span>
        <span className="chart-plays">{row.plays.toLocaleString()}</span>
      </button>
    ))}
  </div>
);
