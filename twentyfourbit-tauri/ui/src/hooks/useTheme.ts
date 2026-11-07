import { useCallback, useEffect, useState } from "react";
import { refreshAccent } from "../lib/theme";

export type Theme = "light" | "dark";

const KEY = "24bit.theme";

const systemTheme = (): Theme =>
  window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";

const forcedTheme = (): Theme | null => {
  const forced = new URLSearchParams(location.search).get("theme");
  return forced === "light" || forced === "dark" ? forced : null;
};

/**
 * Light or dark, following the system unless the user has chosen.
 *
 * A system-derived value is deliberately not persisted. Saving it on first
 * paint would turn "follow system" into a permanent explicit preference.
 */
export const useTheme = () => {
  const [theme, setThemeState] = useState<Theme>(() => {
    const forced = forcedTheme();
    if (forced) return forced;
    const saved = localStorage.getItem(KEY);
    return saved === "light" || saved === "dark" ? saved : systemTheme();
  });

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    refreshAccent();
  }, [theme]);

  // Follow the system while the user has made no explicit choice. A `?theme=`
  // review override takes precedence without becoming a saved preference.
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => {
      if (!localStorage.getItem(KEY) && !forcedTheme()) {
        setThemeState(e.matches ? "dark" : "light");
      }
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);

  const setTheme = useCallback((next: Theme) => {
    localStorage.setItem(KEY, next);
    setThemeState(next);
  }, []);

  const toggle = useCallback(() => {
    setThemeState((current) => {
      const next = current === "dark" ? "light" : "dark";
      localStorage.setItem(KEY, next);
      return next;
    });
  }, []);

  return { theme, setTheme, toggle };
};
