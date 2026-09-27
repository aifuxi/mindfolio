import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  workers: 1,
  use: {
    baseURL: "http://127.0.0.1:5173",
    browserName: "chromium",
  },
  webServer: [
    {
      command: "mise run dev:api",
      cwd: "../..",
      url: "http://127.0.0.1:3001/health/live",
      reuseExistingServer: false,
      timeout: 60_000,
    },
    {
      command: "pnpm dev",
      url: "http://127.0.0.1:5173/login",
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
