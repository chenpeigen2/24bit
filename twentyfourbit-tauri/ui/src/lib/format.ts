/** Presentation helpers. Every string the user reads is shaped here. */

export const duration = (ms: number): string => {
  if (!Number.isFinite(ms) || ms <= 0) return "0:00";
  const total = Math.floor(ms / 1000);
  return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, "0")}`;
};

export const bytes = (n: number): string => {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB"] as const;
  let value = n;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${n} B` : `${value.toFixed(1)} ${units[unit]}`;
};

export const qualityName = (q: string): string =>
  q === "surround" ? "高清环绕声" : "至臻母带";

/** `rgb(...)` from the accent triple, with an alpha. */
export const tint = (rgb: [number, number, number], alpha = 1): string =>
  `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
