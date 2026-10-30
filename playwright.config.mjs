import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/browser',
  use: { baseURL: 'http://127.0.0.1:8080' },
  webServer: {
    command: 'trunk serve --address 127.0.0.1 --port 8080',
    cwd: './examples/yew-counter',
    url: 'http://127.0.0.1:8080',
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
  },
});
