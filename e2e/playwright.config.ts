import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  retries: 0,
  workers: 1,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4173",
    channel: "msedge",
    headless: process.env.PLAYWRIGHT_HEADLESS !== "false",
    viewport: { width: 1280, height: 720 },
    launchOptions: {
      executablePath:
        "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"
    }
  },
  webServer: {
    command: "pnpm exec http-server ../dist -a 127.0.0.1 -p 4173 -c-1",
    url: "http://127.0.0.1:4173",
    reuseExistingServer: false,
    timeout: 15_000
  }
});
