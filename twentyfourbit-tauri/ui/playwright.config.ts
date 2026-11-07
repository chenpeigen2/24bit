import { defineConfig } from "@playwright/test";
import { existsSync } from "node:fs";

const systemChrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  timeout: 15_000,
  expect: { timeout: 4_000 },
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:5183",
    viewport: { width: 1120, height: 760 },
    launchOptions: {
      executablePath: process.env.CHROME_PATH ??
        (existsSync(systemChrome) ? systemChrome : undefined),
    },
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev -- --host 127.0.0.1",
    url: "http://127.0.0.1:5183",
    reuseExistingServer: !process.env.CI,
  },
});
