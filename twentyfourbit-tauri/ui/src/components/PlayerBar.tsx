import { useRef } from "react";
import { duration } from "../lib/format";
import type { PlayerSnapshot } from "../lib/types";
import {
  NextIcon,
  PauseIcon,
  PlayIcon,
  PrevIcon,
  VolumeIcon,
} from "./Icons";

interface Props {
  snapshot: PlayerSnapshot;
  playing: boolean;
  volume: number;
  muted: boolean;
  onToggle: () => void;
  onStep: (delta: number) => void;
  onSeek: (fraction: number) => void;
  onRestart: () => void;
  onVolume: (v: number) => void;
  onMute: () => void;
}

/**
 * Transport. The scrubber rides the top edge so the row below is purely for
 * transport — a progress bar competing with controls reads as clutter.
 */
export const PlayerBar = ({
  snapshot,
  playing,
  volume,
  muted,
  onToggle,
  onStep,
  onSeek,
  onRestart,
  onVolume,
  onMute,
}: Props) => {
  const bar = useRef<HTMLDivElement | null>(null);

  const fractionOf = (clientX: number): number => {
    const rect = bar.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return 0;
    return Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
  };

  // While a track is being fetched there is no transport yet; the download is
  // the progress the user cares about, so the same bar shows it.
  const fetching = snapshot.pending;
  const fetchingPct =
    fetching && fetching.total ? (fetching.done / fetching.total) * 100 : 0;

  const progress = fetching
    ? fetchingPct
    : snapshot.duration
      ? (snapshot.position / snapshot.duration) * 100
      : 0;

  const title = fetching?.title ?? snapshot.title;
  const artist = fetching?.artist ?? snapshot.artist;
  const cover = fetching?.cover || snapshot.cover;
  const quality = fetching?.quality ?? snapshot.quality;

  return (
    <footer className={`player${title ? " has-track" : ""}${fetching ? " is-fetching" : ""}`}>
      <div className="progress">
        <span className="time">{duration(snapshot.position)}</span>
        <div
          className="bar"
          ref={bar}
          onPointerDown={(e) => {
            e.currentTarget.setPointerCapture(e.pointerId);
            onSeek(fractionOf(e.clientX));
          }}
          onPointerMove={(e) => {
            if (e.buttons === 1) onSeek(fractionOf(e.clientX));
          }}
        >
          <div className="bar-fill" style={{ width: `${progress}%` }} />
          <div className="bar-knob" style={{ left: `${progress}%` }} />
        </div>
        <span className="time">{duration(snapshot.duration)}</span>
      </div>

      <div className="player-row">
        <div className="playing">
          <div
            className={`playing-art${fetching ? " is-fetching" : ""}`}
            style={cover ? { backgroundImage: `url("${cover}")` } : undefined}
          />
          <div className="playing-text">
            <div className="playing-title">{title ?? "选一首歌开始播放"}</div>
            <div className="playing-artist">
              {fetching
                ? fetching.total
                  ? `准备中 · ${Math.round(fetchingPct)}%`
                  : "准备中…"
                : title
                  ? artist
                  : "从搜索或榜单开始"}
            </div>
          </div>
        </div>

        <div className="controls">
          <button className="ctl" title="重新开始" onClick={onRestart}>
            <PrevIcon size={16} />
          </button>
          <button
            className="ctl ctl-play"
            title={playing ? "暂停（空格）" : "播放（空格）"}
            onClick={onToggle}
          >
            {playing ? <PauseIcon size={16} /> : <PlayIcon size={16} />}
          </button>
          <button className="ctl" title="下一首" onClick={() => onStep(1)}>
            <NextIcon size={16} />
          </button>
        </div>

        <div className="right">
          <span className="quality">{quality ?? ""}</span>
          <button
            className={`ctl ctl-small${muted ? " is-muted" : ""}`}
            title={muted ? "取消静音" : "静音"}
            onClick={onMute}
          >
            <VolumeIcon size={13} />
          </button>
          <input
            className="volume"
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={muted ? 0 : volume}
            aria-label="音量"
            onChange={(e) => onVolume(Number(e.target.value))}
          />
        </div>
      </div>
    </footer>
  );
};
