import { defineConfig } from '@playwright/test';

const base = process.env.DOCS_BASE || '/';

export default defineConfig({
  testDir: './tests/docs',
  use: { baseURL: `http://127.0.0.1:4173${base}` },
  webServer: {
    command: 'npm run docs:preview',
    url: `http://127.0.0.1:4173${base}`,
    reuseExistingServer: !process.env.CI,
  },
});
