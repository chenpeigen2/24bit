import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri serves the built files from disk, so relative asset paths are required:
// an absolute "/assets/..." would resolve against the webview's origin.
export default defineConfig({
  plugins: [react()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "safari15",
    sourcemap: false,
  },
  server: { port: 5183, strictPort: true },
});
