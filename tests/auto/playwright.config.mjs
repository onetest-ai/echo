// Echo's autotests: run locally (`npm test` here), like the functional cases — never in CI. One worker: each spec file
// builds its own made-up data and serves the window on its own port; the files run one after another.
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: '.',
  testMatch: '*.spec.mjs',
  workers: 1,
  timeout: 90_000,
  expect: { timeout: 10_000 },
  reporter: [['list'], ['json', { outputFile: 'test-results/results.json' }]],
  use: { browserName: 'chromium', headless: true, viewport: { width: 1300, height: 850 }, trace: 'retain-on-failure' },
})
