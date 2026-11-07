import { useRef, useState } from "react";
import { duration } from "../lib/format";
import type { PlaybackMode, PlayerSnapshot } from "../lib/types";
import {
  NextIcon,
  PauseIcon,
  PlayIcon,
  PrevIcon,
  RepeatIcon,
  RepeatOneIcon,
  SequentialIcon,
  ShuffleIcon,
  VolumeIcon,
} from "./Icons";

const MODE_LABEL: Record<PlaybackMode, string> = {
  sequential: "顺序播放",
  "repeat-all": "列表循环",
  "repeat-one": "单曲循环",
  shuffle: "随机播放",
};

const MODE_SHORT_LABEL: Record<PlaybackMode, string> = {
  sequential: "顺序",
  "repeat-all": "循环",
  "repeat-one": "单曲",
  shuffle: "随机",
};

const ModeIcon = ({ mode }: { mode: PlaybackMode }) => {
  if (mode === "repeat-all") return <RepeatIcon size={15} />;
  if (mode === "repeat-one") return <RepeatOneIcon size={15} />;
  if (mode === "shuffle") return <ShuffleIcon size={15} />;
  return <SequentialIcon size={15} />;
};

interface Props {
  snapshot: PlayerSnapshot;
  playing: boolean;
  mode: PlaybackMode;
  volume: number;
  muted: boolean;
  onToggle: () => void;
  onStep: (delta: number) => void;
  onCycleMode: () => void;
  onSeek: (fraction: number) => void;
  onVolume: (v: number) => void;
  onMute: () => void;
}

/** Transport. Dragging is local preview; exactly one seek is sent on release. */
export const PlayerBar = ({
  snapshot,
  playing,
  mode,
  volume,
  muted,
  onToggle,
  onStep,
  onCycleMode,
  onSeek,
  onVolume,
  onMute,
}: Props) => {
  const drag = useRef<number | null>(null);
  const [preview, setPreview] = useState<number | null>(null);
  const updatePreview = (fraction: number) => {
    drag.current = fraction;
    setPreview(fraction);
  };
  const cancelDrag = () => {
    drag.current = null;
    setPreview(null);
  };

  const fetching = snapshot.pending;
  const fetchingFraction = fetching?.total
    ? Math.max(0, Math.min(1, fetching.done / fetching.total))
    : 0;
  const canSeek = !fetching && snapshot.duration > 0;
  const actualFraction = snapshot.duration
    ? Math.max(0, Math.min(1, snapshot.position / snapshot.duration))
    : 0;
  const fraction = fetching ? fetchingFraction : (preview ?? actualFraction);
  const fetchingPct = fetchingFraction * 100;
  const displayedPosition = preview !== null ? preview * snapshot.duration : snapshot.position;
  const durationSeconds = Math.max(1, Math.floor(snapshot.duration / 1000));

  const title = fetching?.title ?? snapshot.title;
  const artist = fetching?.artist ?? snapshot.artist;
  const cover = fetching?.cover || snapshot.cover;
  const quality = fetching?.quality ?? snapshot.quality;

  return (
    <footer className={`player${title ? " has-track" : ""}${fetching ? " is-fetching" : ""}`}>
      <div className="progress">
        <span className="time">{fetching ? "下载" : duration(displayedPosition)}</span>
        <div
          className={`bar${preview !== null ? " is-dragging" : ""}`}
          role={fetching ? "progressbar" : undefined}
          aria-label={fetching ? "下载进度" : undefined}
          aria-valuemin={fetching ? 0 : undefined}
          aria-valuemax={fetching ? 100 : undefined}
          aria-valuenow={fetching ? Math.round(fetchingPct) : undefined}
          aria-valuetext={fetching ? `准备中 ${Math.round(fetchingPct)}%` : undefined}
        >
          <div className="bar-fill" style={{ width: `${fraction * 100}%` }} />
          <div className="bar-knob" style={{ left: `${fraction * 100}%` }} />
          {!fetching && (
            <input
              className="range-native"
              type="range"
              min={0}
              max={durationSeconds}
              step={1}
              value={Math.floor(displayedPosition / 1000)}
              disabled={!canSeek}
              tabIndex={canSeek ? 0 : -1}
              aria-label="播放进度"
              aria-valuetext={`${duration(displayedPosition)} / ${duration(snapshot.duration)}`}
              onPointerDown={(e) => {
                if (e.button !== 0) return;
                updatePreview(Number(e.currentTarget.value) / durationSeconds);
              }}
              onChange={(e) => {
                const pointerActive = drag.current !== null;
                const next = Number(e.currentTarget.value) / durationSeconds;
                updatePreview(next);
                // With no pointer active this is an assistive-technology or
                // native input change, so it is already a complete intent.
                if (!pointerActive) {
                  cancelDrag();
                  onSeek(next);
                }
              }}
              onPointerUp={(e) => {
                if (drag.current === null) return;
                const next = Number(e.currentTarget.value) / durationSeconds;
                cancelDrag();
                onSeek(next);
              }}
              onPointerCancel={cancelDrag}
              onKeyDown={(e) => {
                let next: number | undefined;
                if (e.key === "ArrowRight" || e.key === "ArrowUp") {
                  next = (snapshot.position + 5000) / snapshot.duration;
                } else if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
                  next = (snapshot.position - 5000) / snapshot.duration;
                } else if (e.key === "Home") {
                  next = 0;
                } else if (e.key === "End") {
                  next = 1;
                }
                if (next !== undefined) {
                  e.preventDefault();
                  onSeek(Math.max(0, Math.min(1, next)));
                }
              }}
            />
          )}
        </div>
        <span className="time">
          {fetching ? `${Math.round(fetchingPct)}%` : duration(snapshot.duration)}
        </span>
      </div>

      <div className="player-row">
        <div className="playing">
          <div
            className={`playing-art${fetching ? " is-fetching" : ""}`}
            aria-hidden="true"
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
          <button className="ctl" title="上一首（Shift + ←）" aria-label="上一首" disabled={!title} onClick={() => onStep(-1)}>
            <PrevIcon size={16} />
          </button>
          <button
            className="ctl ctl-play"
            title={playing ? "暂停（空格）" : "播放（空格）"}
            aria-label={playing ? "暂停（空格）" : "播放（空格）"}
            disabled={!title || Boolean(fetching)}
            onClick={onToggle}
          >
            {playing ? <PauseIcon size={16} /> : <PlayIcon size={16} />}
          </button>
          <button className="ctl" title="下一首（Shift + →）" aria-label="下一首" disabled={!title} onClick={() => onStep(1)}>
            <NextIcon size={16} />
          </button>
        </div>

        <div className="right">
          <button
            type="button"
            className={`playback-mode mode-${mode}`}
            aria-label={`播放模式：${MODE_LABEL[mode]}，点击切换`}
            title={`${MODE_LABEL[mode]} · 点击切换`}
            onClick={onCycleMode}
          >
            <ModeIcon mode={mode} />
            <span>{MODE_SHORT_LABEL[mode]}</span>
          </button>
          <span className="quality">{quality ?? ""}</span>
          <button
            className={`ctl ctl-small${muted ? " is-muted" : ""}`}
            title={muted ? "取消静音" : "静音"}
            aria-label={muted ? "取消静音" : "静音"}
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
