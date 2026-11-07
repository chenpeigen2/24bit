/** Line icons, drawn inline so they inherit currentColor and stay crisp. */

interface Props {
  size?: number;
}

const Svg = ({ size = 16, children }: Props & { children: React.ReactNode }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth={1.7}
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    {children}
  </svg>
);

export const SearchIcon = ({ size }: Props) => (
  <Svg size={size}>
    <circle cx="10.5" cy="10.5" r="5.5" />
    <path d="M15 15l4.5 4.5" />
  </Svg>
);

export const MusicIcon = ({ size }: Props) => (
  <Svg size={size}>
    <path d="M9 18V6l10-2v12" />
    <circle cx="6.5" cy="18" r="2.5" />
    <circle cx="16.5" cy="16" r="2.5" />
  </Svg>
);

export const LibraryIcon = ({ size }: Props) => (
  <Svg size={size}>
    <path d="M4 5v14M9 5v14M14 5v14M19 6.5l1.5 13" />
  </Svg>
);

export const GearIcon = ({ size }: Props) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="3.2" />
    <path d="M12 3v2.2M12 18.8V21M3 12h2.2M18.8 12H21M5.6 5.6l1.6 1.6M16.8 16.8l1.6 1.6M18.4 5.6l-1.6 1.6M7.2 16.8l-1.6 1.6" />
  </Svg>
);

export const PlayIcon = ({ size = 16 }: Props) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <path d="M9 6.5L18 12 9 17.5z" />
  </svg>
);

export const PauseIcon = ({ size = 16 }: Props) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <rect x="8.4" y="6.5" width="2.9" height="11" rx="1" />
    <rect x="12.9" y="6.5" width="2.9" height="11" rx="1" />
  </svg>
);

export const PrevIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M6 6v12M18 6.5v11L9.5 12z" />
  </Svg>
);

export const NextIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M18 6v12M6 6.5v11L14.5 12z" />
  </Svg>
);

export const SequentialIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M4 7h12M4 12h12M4 17h8" />
    <path d="M16 15l4 2-4 2z" fill="currentColor" stroke="none" />
  </Svg>
);

export const RepeatIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M17 4l3 3-3 3" />
    <path d="M20 7H7a4 4 0 00-4 4v1" />
    <path d="M7 20l-3-3 3-3" />
    <path d="M4 17h13a4 4 0 004-4v-1" />
  </Svg>
);

export const RepeatOneIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M17 4l3 3-3 3" />
    <path d="M20 7H7a4 4 0 00-4 4v1" />
    <path d="M7 20l-3-3 3-3" />
    <path d="M4 17h13a4 4 0 004-4v-1" />
    <path d="M12 9.5v5M10.5 10.5L12 9.5" strokeWidth={1.4} />
  </Svg>
);

export const ShuffleIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M17 4l3 3-3 3M17 14l3 3-3 3" />
    <path d="M3 7h2c5 0 6 10 11 10h4" />
    <path d="M3 17h2c2.5 0 3.8-2.5 5.2-5M12.8 9.5C14 8 15 7 16 7h4" />
  </Svg>
);

export const DownloadIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M12 5v10" />
    <path d="M7.5 11.5L12 16l4.5-4.5" />
    <path d="M5.5 19h13" />
  </Svg>
);

export const TrashIcon = ({ size }: Props) => (
  <Svg size={size}>
    <path d="M5.5 7h13" />
    <path d="M9 7V5.5h6V7" />
    <path d="M7 7l.8 11.5h8.4L17 7" />
  </Svg>
);

export const FolderIcon = ({ size }: Props) => (
  <Svg size={size}>
    <path d="M4 7.5h6l1.5 2H20v9H4z" />
  </Svg>
);

export const VolumeIcon = ({ size = 16 }: Props) => (
  <Svg size={size}>
    <path d="M4.5 9.5h3l4-3.5v12l-4-3.5h-3z" />
    <path d="M15 9.2a3.8 3.8 0 010 5.6" />
  </Svg>
);

/** A determinate ring, used while a track downloads. */
export const ProgressRing = ({ fraction }: { fraction: number }) => {
  const r = 9.5;
  const circumference = 2 * Math.PI * r;
  const clamped = Math.max(0, Math.min(1, fraction));
  return (
    <span className="ring" aria-label={`下载 ${Math.round(clamped * 100)}%`}>
      <svg width="24" height="24" viewBox="0 0 24 24">
        <circle className="ring-track" cx="12" cy="12" r={r} />
        <circle
          className="ring-value"
          cx="12"
          cy="12"
          r={r}
          strokeDasharray={circumference}
          strokeDashoffset={circumference * (1 - clamped)}
        />
      </svg>
      <span className="ring-label">{Math.round(clamped * 100)}</span>
    </span>
  );
};

export const SunIcon = ({ size = 13 }: Props) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.4 5.4l1.6 1.6M17 17l1.6 1.6M18.6 5.4L17 7M7 17l-1.6 1.6" />
  </Svg>
);

export const MoonIcon = ({ size = 13 }: Props) => (
  <Svg size={size}>
    <path d="M20 14.5A8.5 8.5 0 019.5 4a8.5 8.5 0 1010.5 10.5z" />
  </Svg>
);
