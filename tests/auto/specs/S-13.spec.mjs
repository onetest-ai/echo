// S-13 Find your way around — its stages with fixed checks (small data).
import { test, expect } from '@playwright/test'
import { useData } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { AppPage } from '../pages/app.page.mjs'

const data = useData(test, { set: 'small' })

scenario(test, 'S-13', 'Find your way around', () => {
  test('15 · The status bar shows Actions and Claude, and no AI profile', covers('shell/TC-014', 'F-WS-07'), async ({ page }) => {
    const app = new AppPage(page, data)
    await app.goto('#today')
    await expect(app.statusBar.getByRole('button', { name: /^Actions/ })).toBeVisible()
    await expect(app.statusBar.getByRole('button', { name: /Claude/ })).toBeVisible()
    await expect(app.statusBar).not.toContainText('AI:')
  })
})
