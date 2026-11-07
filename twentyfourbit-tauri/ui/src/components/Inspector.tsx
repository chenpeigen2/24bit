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
}: Props) => {
  const activeRef = useRef<HTMLDivElement | null>(null);

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

      {lyrics.length > 0 && (
        <div className="lyrics">
          {lyrics.map((line, i) => (
            <div
              key={`${line.at}-${i}`}
              ref={i === currentIndex ? activeRef : undefined}
              className={i === currentIndex ? "is-current" : ""}
              onClick={() => onSeek(line.at)}
            >
              {line.text}
            </div>
          ))}
        </div>
      )}
    </aside>
  );
};
