import { useCallback, useEffect, useState } from "react";

export type Theme = "light" | "dark";

const KEY = "24bit.theme";

/**
 * Light or dark, following the system unless the user has chosen.
 *
 * The media query is the default rather than the only option because a music
 * app is often the one thing left open at night, and a white window at 2am is
 * hostile.
 */
export const useTheme = () => {
  const [theme, setTheme] = useState<Theme>(() => {
    // `?theme=` lets a headless render pick a theme for review, and lets a user
    // force one without visiting settings.
    const forced = new URLSearchParams(location.search).get("theme");
    if (forced === "light" || forced === "dark") return forced;
    const saved = localStorage.getItem(KEY);
    if (saved === "light" || saved === "dark") return saved;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  });

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(KEY, theme);
  }, [theme]);

  // Follow the system while the user has made no explicit choice.
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => {
      if (!localStorage.getItem(KEY)) setTheme(e.matches ? "dark" : "light");
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const toggle = useCallback(() => {
    setTheme((t) => {
      const next = t === "dark" ? "light" : "dark";
      localStorage.setItem(KEY, next);
      return next;
    });
  }, []);

  return { theme, setTheme, toggle };
};
