/**
 * Design tokens, applied as CSS custom properties.
 *
 * The romantic register comes from three decisions:
 *  - a serif for anything that is a title (宋体 / Songti), which reads as
 *    editorial rather than as software;
 *  - a warm near-black instead of pure black, so the light feels candle-like;
 *  - the artwork's own colour driving every "active" signal.
 */

export interface Accent {
  rgb: [number, number, number];
}

const NEUTRAL: [number, number, number] = [96, 88, 104];

export const applyAccent = (accent?: [number, number, number]): void => {
  const [r, g, b] = accent ?? NEUTRAL;
  const root = document.documentElement.style;
  root.setProperty("--accent", `rgb(${r} ${g} ${b})`);
  root.setProperty("--accent-15", `rgb(${r} ${g} ${b} / .15)`);
  root.setProperty("--accent-35", `rgb(${r} ${g} ${b} / .35)`);
  root.setProperty("--accent-60", `rgb(${r} ${g} ${b} / .60)`);

  // A slightly brighter sibling for text: the raw accent is usually too dark to
  // read on paper.
  const lift = (v: number) => Math.min(255, Math.round(v * 1.12 + 14));
  root.setProperty("--accent-ink", `rgb(${lift(r)} ${lift(g)} ${lift(b)})`);
};

/** Set the two ambient layers to the given artwork. */
export const applyAmbient = (cover: string | null): void => {
  const value = cover ? `url("${cover}")` : "";
  for (const id of ["ambient-far", "ambient-near"]) {
    const node = document.getElementById(id);
    if (node) node.style.backgroundImage = value;
  }
  document.body.classList.toggle("has-art", Boolean(cover));
};
