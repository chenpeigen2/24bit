import { useEffect, useRef } from "react";
import type { LyricLine, SearchItem } from "../lib/types";

interface Props {
  item: SearchItem;
  playing: boolean;
  lyrics: LyricLine[];
  position: number;
  onPlay: () => void;
  onDownload: () => void;
  onSeek: (ms: number) => void;
  upNext: SearchItem[];
  onPick: (item: SearchItem) => void;
}

/**
 * Artwork, actions and — while this track plays — its lyrics.
 *
 * The lyrics follow playback and are click-to-seek. Keeping them here rather
 * than in a second panel is what lets the page have one focal point.
 */
export const Inspector = ({
  item,
  playing,
  lyrics,
  position,
  onPlay,
  onDownload,
  onSeek,
  upNext,
  onPick,
}: Props) => {
  const activeRef = useRef<HTMLButtonElement | null>(null);

  const currentIndex = lyrics.findIndex(
    (line, i) =>
      line.at <= position && (i + 1 === lyrics.length || (lyrics[i + 1]?.at ?? 0) > position),
  );

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [currentIndex]);

  return (
    <aside className="inspector">
      <div
        className={`art${playing ? " is-playing" : ""}`}
        style={item.cover ? { backgroundImage: `url("${item.cover}")` } : undefined}
      >
        {playing && <span className="art-breathe" aria-hidden="true" />}
      </div>

      <h2 className="insp-title">{item.name}</h2>
      <div className="insp-artist">{item.player}</div>
      <div className="insp-album">{item.album}</div>

      <div className="insp-actions">
        <button className="btn-primary" onClick={onPlay}>
          {playing ? "暂停" : "播放"}
        </button>
        <button className="btn-secondary" onClick={onDownload}>
          下载
        </button>
      </div>

      {lyrics.length > 0 ? (
        <div className="lyrics">
          {lyrics.map((line, i) => (
            <button
              type="button"
              key={`${line.at}-${i}`}
              ref={i === currentIndex ? activeRef : undefined}
              className={i === currentIndex ? "is-current" : ""}
              aria-current={i === currentIndex ? "true" : undefined}
              onClick={() => onSeek(line.at)}
            >
              {line.text}
            </button>
          ))}
        </div>
      ) : upNext.length > 0 ? (
        <div className="up-next">
          <h3>接下来播放</h3>
          {upNext.map((next) => (
            <button type="button" key={next.id} onClick={() => onPick(next)}>
              <span
                className="up-next-art"
                style={next.cover ? { backgroundImage: `url("${next.cover}")` } : undefined}
              />
              <span className="up-next-text">
                <strong>{next.name}</strong>
                <small>{next.player}</small>
              </span>
            </button>
          ))}
        </div>
      ) : null}
    </aside>
  );
};
