/**
 * Design tokens, applied as CSS custom properties.
 *
 * The artwork drives decoration, while text and controls retain enough
 * contrast to remain readable in either theme.
 */

export interface Accent {
  rgb: [number, number, number];
}

type Rgb = [number, number, number];

const NEUTRAL: Rgb = [96, 88, 104];
const LIGHT_PAPER: Rgb = [252, 251, 252];
const DARK_PAPER: Rgb = [22, 18, 24];
let currentAccent: Rgb = NEUTRAL;

const channel = (value: number): number => {
  const srgb = value / 255;
  return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
};

const luminance = (rgb: Rgb): number =>
  0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);

const contrast = (a: Rgb, b: Rgb): number => {
  const first = luminance(a);
  const second = luminance(b);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
};

const cssRgb = (rgb: Rgb): string => `rgb(${rgb[0]} ${rgb[1]} ${rgb[2]})`;

/** Move an artwork colour away from the paper until it is readable text. */
const readableInk = (accent: Rgb, dark: boolean): Rgb => {
  const paper = dark ? DARK_PAPER : LIGHT_PAPER;
  let candidate = [...accent] as Rgb;
  for (let step = 0; step < 40 && contrast(candidate, paper) < 4.5; step += 1) {
    candidate = candidate.map((value) =>
      dark
        ? Math.round(value + (255 - value) * 0.09)
        : Math.round(value * 0.89),
    ) as Rgb;
  }
  return candidate;
};

/** Recompute theme-dependent tones without forgetting the current artwork. */
export const refreshAccent = (): void => {
  const [r, g, b] = currentAccent;
  const root = document.documentElement.style;
  const dark = document.documentElement.dataset.theme === "dark";
  const ink = readableInk(currentAccent, dark);
  const white: Rgb = [255, 255, 255];
  const nearBlack: Rgb = [26, 21, 32];

  root.setProperty("--accent", cssRgb(currentAccent));
  root.setProperty("--accent-15", `rgb(${r} ${g} ${b} / .15)`);
  root.setProperty("--accent-35", `rgb(${r} ${g} ${b} / .35)`);
  root.setProperty("--accent-60", `rgb(${r} ${g} ${b} / .60)`);
  root.setProperty("--accent-ink", cssRgb(ink));
  root.setProperty(
    "--accent-on-accent",
    contrast(currentAccent, nearBlack) >= contrast(currentAccent, white) ? "#1a1520" : "#fff",
  );
};

export const applyAccent = (accent?: Rgb): void => {
  currentAccent = accent ?? NEUTRAL;
  refreshAccent();
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
