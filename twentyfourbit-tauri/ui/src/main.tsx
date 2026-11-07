import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { demoSeed } from "./lib/api";
import { applyAccent, applyAmbient } from "./lib/theme";
import "./styles.css";

// Paint the theme and accent before the first frame. Theme-dependent accent
// text must not be computed against an uninitialised root element.
const storedTheme = localStorage.getItem("24bit.theme");
const forcedTheme = new URLSearchParams(location.search).get("theme");
if (forcedTheme === "light" || forcedTheme === "dark") {
  document.documentElement.dataset.theme = forcedTheme;
} else if (storedTheme === "light" || storedTheme === "dark") {
  document.documentElement.dataset.theme = storedTheme;
} else {
  document.documentElement.dataset.theme = window.matchMedia(
    "(prefers-color-scheme: dark)",
  ).matches
    ? "dark"
    : "light";
}

if (demoSeed) {
  applyAccent(demoSeed.accent);
  applyAmbient(demoSeed.cover);
} else {
  applyAccent();
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
