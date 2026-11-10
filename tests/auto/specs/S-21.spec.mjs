// S-21 Living with Echo — its one stage with fixed checks that needs no Echo app (small data, the fake Claude Code):
// the window following the system's light and dark (stage 4).
// Not here: stages 1–3 (lifecycle/TC-004 a second start refused, TC-003 quit and start again, TC-005 another data
// folder) need the Echo app itself (`echoctl app run`, its restart through `open`); stages 5–12 are a person's
// (start at login, `app start`, the native window and its Dock icon, updates from GitHub, offline, the network).
import { test, expect } from '@playwright/test'
import { useData } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'

const data = useData(test, { set: 'small', ai: 'fake' })

scenario(test, 'S-21', 'Living with Echo: quit, one instance, another folder, appearance; login, updates, the native window, offline, the network', () => {
  test('4 · With Appearance on System, the window follows the system from light to dark and back, live', covers('window/TC-015', 'F-SET-04'), async ({ page }) => {
    const s = data.text.settings
    const a = s.appearance
    const settings = new SettingsPage(page, data)
    const heading = settings.heading(s.general.heading)
    /** The window and its headings drawn as the system's look, within a second of the system's change. */
    const drawn = async (look) => {
      await expect.poll(() => settings.windowColour(), { timeout: 1_000 }).toBe(a.window[look])
      await expect(heading).toHaveCSS('color', a.text[look], { timeout: 1_000 })
    }
    await page.emulateMedia({ colorScheme: 'light' })
    await settings.open(s.general.route)
    await expect(settings.choice(a.group, a.system)).toHaveAttribute('aria-pressed', 'true')
    expect(await settings.themeChosen()).toBeNull()
    await drawn('light')
    // No reload: the open page follows.
    await page.emulateMedia({ colorScheme: 'dark' })
    await drawn('dark')
    await page.emulateMedia({ colorScheme: 'light' })
    await drawn('light')
    await expect(settings.choice(a.group, a.system)).toHaveAttribute('aria-pressed', 'true')
  })
})
