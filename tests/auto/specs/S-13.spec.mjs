// S-13 Find your way around — its stages with fixed checks (small data).
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { AppPage } from '../pages/app.page.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { PalettePage } from '../pages/palette.page.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'

const data = useData(test, { set: 'small' })

/** The window's layout file (`window-layout.json`), as it is now (empty until the window first writes it). */
const layout = () => (data.exists(data.env.ECHOCTL_CONFIG_DIR, data.text.shell.layoutFile) ? data.json(data.env.ECHOCTL_CONFIG_DIR, data.text.shell.layoutFile) : {})
/** A key of the window's shortcuts, by its name in texts.json (`activity` takes the activity's number). */
const key = (name, n) => fill(data.text.shell.keys[name], { n })

scenario(test, 'S-13', 'Find your way around', () => {
  test('1 · The activity bar lists Today, Calls, Calendar, Threads, Tasks, then Settings, folded to icons', covers('shell/TC-001', 'F-WS-01'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await app.goto(sh.routes.today)
    await expect(app.text(sh.yourDay)).toBeVisible()
    await expect(app.activities.getByRole('button')).toHaveCount(sh.activities.length)
    for (const [i, name] of sh.activities.entries()) await expect(app.activities.getByRole('button').nth(i)).toHaveAccessibleName(name)
    for (const name of sh.notActivities) await expect(app.activity(name)).toHaveCount(0)
    await expect(app.expandSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeHidden()
  })

  test("2 · Expand and fold the sidebar; the choice is saved and kept after a reload", covers('shell/TC-002', 'F-WS-01'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await app.goto(sh.routes.today)
    await app.expandSidebar.click()
    await expect(app.foldSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeVisible()
    await expect.poll(() => layout().bar).toEqual({ open: true })
    await page.reload()
    await expect(app.foldSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeVisible()
    await app.foldSidebar.click()
    await expect(app.expandSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeHidden()
    await expect.poll(() => layout().bar).toEqual({ open: false })
  })

  test('3 · The sidebar flips with the keyboard', covers('shell/TC-003', 'F-WS-01'), async ({ page }) => {
    const app = new AppPage(page, data)
    await app.goto(data.text.shell.routes.today)
    await expect(app.expandSidebar).toBeVisible()
    await page.keyboard.press(key('sidebar'))
    await expect(app.foldSidebar).toBeVisible()
    await page.keyboard.press(key('sidebar'))
    await expect(app.expandSidebar).toBeVisible()
  })

  test("4 · The number shortcuts open the activities in the bar's order", covers('shell/TC-004', 'F-WS-02'), async ({ page }) => {
    const sh = data.text.shell
    const r = sh.routes
    const app = new AppPage(page, data)
    await app.goto(r.today)
    await expect(app.text(sh.yourDay)).toBeVisible()
    await page.keyboard.press(key('activity', 2))
    await expect.poll(() => app.hash()).toBe(r.calls)
    await expect(app.activity(sh.activities[1])).toHaveAttribute('aria-current', 'page')
    await page.keyboard.press(key('activity', 3))
    await expect.poll(() => app.hash()).toMatch(new RegExp(`^${r.calendar}`))
    await page.keyboard.press(key('activity', 4))
    await expect.poll(() => app.hash()).toMatch(new RegExp(`^${r.project}`))
    await expect(app.main.getByRole('heading', { name: sh.threads, level: 1 })).toBeVisible()
    await page.keyboard.press(key('activity', 5))
    await expect.poll(() => app.hash()).toBe(r.tasks)
    await page.keyboard.press(key('activity', 1))
    await expect.poll(() => app.hash()).toBe(r.today)
    await expect(app.text(sh.yourDay)).toBeVisible()
  })

  test('5 · Inbox in the top bar shows and hides the right panel', covers('shell/TC-011', 'F-WS-04'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await app.goto(sh.routes.today)
    await expect(app.text(sh.yourDay)).toBeVisible()
    await expect(app.closePanel(app.rightPanel)).toHaveCount(0)
    await app.inbox.click()
    await expect(app.panelTab(app.rightPanel, sh.inbox)).toHaveAttribute('aria-selected', 'true')
    await expect(app.rightPanel.getByText(sh.nothingNeedsYou)).toBeVisible()
    await expect(app.movePanel(app.rightPanel, sh.bottom)).toBeVisible()
    await expect(app.closePanel(app.rightPanel)).toBeVisible()
    await app.panelTab(app.rightPanel, sh.vocabulary).click()
    await expect(app.rightPanel.getByText(sh.vocabularyLine)).toBeVisible()
    await app.panelTab(app.rightPanel, sh.inbox).click()
    await app.inbox.click()
    await expect(app.closePanel(app.rightPanel)).toHaveCount(0)
  })

  test('6 · The right panel starts closed after a reload; Inbox opens it on Inbox, not its last tab', covers('shell/TC-012', 'F-WS-04'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await app.goto(sh.routes.today)
    await app.inbox.click()
    await app.panelTab(app.rightPanel, sh.vocabulary).click()
    await expect(app.panelTab(app.rightPanel, sh.vocabulary)).toHaveAttribute('aria-selected', 'true')
    await expect.poll(() => layout().right?.active).toBe(sh.vocabularyView)
    await page.reload()
    await expect(app.text(sh.yourDay)).toBeVisible()
    await expect(app.closePanel(app.rightPanel)).toHaveCount(0)
    await app.inbox.click()
    await expect(app.panelTab(app.rightPanel, sh.inbox)).toHaveAttribute('aria-selected', 'true')
    await expect(app.panelTab(app.rightPanel, sh.vocabulary)).toHaveAttribute('aria-selected', 'false')
    await expect(app.rightPanel.getByText(sh.nothingNeedsYou)).toBeVisible()
    await expect.poll(() => layout().right?.active).toBe(sh.needsView)
    await app.closePanel(app.rightPanel).click()
  })

  test('7 · A panel view moves to the bottom panel and back', covers('shell/TC-013', 'F-WS-04', 'F-SET-16'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await app.goto(sh.routes.today)
    await app.inbox.click()
    await expect(app.panelTab(app.rightPanel, sh.inbox)).toBeVisible()
    await app.movePanel(app.rightPanel, sh.bottom).click()
    await expect(app.panelTab(app.bottomPanel, sh.inbox)).toHaveAttribute('aria-selected', 'true')
    await expect(app.movePanel(app.bottomPanel, sh.right)).toBeVisible()
    await expect.poll(() => layout().views?.[sh.needsView]).toEqual({ area: sh.bottom })
    await app.movePanel(app.bottomPanel, sh.right).click()
    await expect(app.panelTab(app.rightPanel, sh.inbox)).toBeVisible()
    await expect.poll(() => layout().views?.[sh.needsView]).toEqual({ area: sh.right })
    await app.closePanel(app.rightPanel).click()
  })

  test('8 · Settings → Layout lists every panel view; Vocabulary moves to the bottom panel', covers('settings/TC-023', 'F-SET-16', 'F-WS-04'), async ({ page }) => {
    const { shell: sh, settings: { layout: l } } = data.text
    const settings = new SettingsPage(page, data)
    await settings.open(sh.routes.layout)
    await expect(settings.heading(l.heading)).toBeVisible()
    await expect(settings.layoutRows).toHaveCount(l.views.length)
    for (const [i, view] of l.views.entries()) await expect(settings.layoutRows.nth(i)).toHaveAccessibleName(view)
    await expect(settings.place(sh.vocabulary, l.right)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.place(l.views[3], l.bottom)).toHaveAttribute('aria-pressed', 'true')
    await settings.place(sh.vocabulary, l.bottom).click()
    await expect(settings.place(sh.vocabulary, l.bottom)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.panelTab(settings.bottomPanel, sh.vocabulary)).toHaveAttribute('aria-selected', 'true')
    await expect.poll(() => layout().views?.[sh.vocabularyView]).toEqual({ area: sh.bottom })
  })

  test('9 · Reset puts every view back and folds the sidebar', covers('settings/TC-024', 'F-SET-16', 'F-WS-04'), async ({ page }) => {
    const { shell: sh, settings: { layout: l } } = data.text
    const settings = new SettingsPage(page, data)
    await settings.open(sh.routes.layout)
    await expect(settings.place(sh.vocabulary, l.bottom)).toHaveAttribute('aria-pressed', 'true')
    await settings.reset.click()
    await expect(settings.place(sh.vocabulary, l.right)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.place(l.views[3], l.bottom)).toHaveAttribute('aria-pressed', 'true')
    await expect(settings.expandSidebar).toBeVisible()
    await expect.poll(() => layout().views).toEqual({})
    expect(layout().bar).toEqual({ open: false })
  })

  test('10 · The palette greys out what cannot run on a call, says why, and does nothing; Backspace drops the target', covers('shell/TC-010', 'F-WS-03'), async ({ page }) => {
    const p = data.text.palette
    const title = data.set.calls.noMinutes.title
    const calls = new CallsPage(page, data)
    const palette = new PalettePage(page, data)
    await calls.open()
    await calls.openCall(title)
    await palette.open(key('palette'))
    await expect(palette.target(fill(p.target, { label: title }))).toBeVisible()
    const item = palette.item(p.inBrowser, p.groups.call)
    await expect(item).toHaveAttribute('aria-disabled', 'true')
    await expect(item).toHaveAccessibleName(`${p.inBrowser} ${p.noMinutes}`)
    const url = page.url()
    for (let i = 0; i < 20 && (await item.getAttribute('aria-selected')) !== 'true'; i++) await page.keyboard.press(key('down'))
    await expect(item).toHaveAttribute('aria-selected', 'true')
    await page.keyboard.press(key('enter'))
    await expect(palette.dialog).toBeVisible()
    expect(page.url()).toBe(url)
    await page.keyboard.press(key('backspace'))
    await expect(palette.targetChip).toHaveCount(0)
    await expect(palette.group(p.groups.call)).toHaveCount(0)
    await page.keyboard.press(key('escape'))
    await expect(palette.dialog).toBeHidden()
  })

  test("11 · The palette on an open call lists the call's own actions and the threads", covers('shell/TC-006', 'F-WS-03'), async ({ page }) => {
    const p = data.text.palette
    const title = data.set.calls.withMinutes.title
    const calls = new CallsPage(page, data)
    const palette = new PalettePage(page, data)
    await calls.open()
    await calls.openCall(title)
    await expect.poll(() => calls.hash()).toMatch(/\/minutes$/)
    await palette.open(key('palette'))
    await expect(palette.box).toBeFocused()
    await expect(palette.target(fill(p.target, { label: title }))).toBeVisible()
    for (const g of p.callGroups) await expect(palette.group(g)).toBeVisible()
    await expect(palette.items(p.groups.call)).toHaveCount(p.callItems.length)
    for (const [i, label] of p.callItems.entries()) await expect(palette.items(p.groups.call).nth(i)).toHaveAccessibleName(new RegExp(`^${literal(label)}( |$)`))
    for (const thread of data.set.threads) await expect(palette.item(thread, p.groups.threads)).toHaveAccessibleName(`${thread} ${p.active}`)
    const hash = calls.hash()
    await page.keyboard.press(key('escape'))
    await expect(palette.dialog).toBeHidden()
    expect(calls.hash()).toBe(hash)
  })

  test('12 · The Settings shortcut opens General; the activity keys go back to where you were; Settings reopens on the section left last', covers('shell/TC-005', 'F-WS-02'), async ({ page }) => {
    const { shell: sh, settings: s } = data.text
    const calls = new CallsPage(page, data)
    const settings = new SettingsPage(page, data)
    await calls.open()
    await calls.openCall(data.set.calls.withMinutes.title)
    await expect.poll(() => calls.hash()).toMatch(new RegExp(`^#call/[^/]+-${data.set.calls.withMinutes.folder}/minutes$`))
    await page.reload()
    await expect(calls.heading(data.set.calls.withMinutes.title)).toBeVisible()
    const call = calls.hash()
    await page.keyboard.press(key('activity', 1))
    await expect.poll(() => calls.hash()).toBe(sh.routes.today)
    await expect(calls.text(sh.yourDay)).toBeVisible()
    await page.keyboard.press(key('activity', 2))
    await expect.poll(() => calls.hash()).toBe(call)
    await expect(calls.heading(data.set.calls.withMinutes.title)).toBeVisible()
    await page.keyboard.press(key('settings'))
    await expect.poll(() => calls.hash()).toBe(s.general.route)
    await expect(settings.heading(s.general.heading)).toBeVisible()
    await expect(settings.textStarting(s.general.line)).toBeVisible()
    await settings.section(s.recording.section).click()
    await expect(settings.heading(s.recording.section)).toBeVisible()
    await page.keyboard.press(key('activity', 1))
    await expect.poll(() => calls.hash()).toBe(sh.routes.today)
    await page.keyboard.press(key('settings'))
    await expect.poll(() => calls.hash()).toBe(s.recording.route)
  })

  test("13 · The palette on Today lists the day's actions", covers('shell/TC-007', 'F-WS-03'), async ({ page }) => {
    const { shell: sh, palette: p } = data.text
    const palette = new PalettePage(page, data)
    await palette.goto(sh.routes.today)
    await expect(palette.text(sh.yourDay)).toBeVisible()
    await palette.open(key('palette'))
    await expect(palette.target(fill(p.target, { label: p.today }))).toBeVisible()
    for (const label of p.todayWork) await expect(palette.item(fill(label, { task: data.set.myTask }), p.groups.myWork)).toBeVisible()
    for (const label of p.todayEcho) await expect(palette.item(label, p.groups.echo)).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(palette.dialog).toBeHidden()
  })

  test("14 · The status bar's Actions opens the palette; the shortcut opens and closes it", covers('shell/TC-009', 'F-WS-03', 'F-WS-07'), async ({ page }) => {
    const palette = new PalettePage(page, data)
    await palette.goto(data.text.shell.routes.today)
    await palette.statusBarButton(data.text.statusBar.actions).click()
    await expect(palette.dialog).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(palette.dialog).toBeHidden()
    await page.keyboard.press(key('palette'))
    await expect(palette.dialog).toBeVisible()
    await page.keyboard.press(key('palette'))
    await expect(palette.dialog).toBeHidden()
  })

  test('15 · The status bar shows Actions and Claude, and no AI profile', covers('shell/TC-014', 'F-WS-07'), async ({ page }) => {
    const t = data.text.statusBar
    const app = new AppPage(page, data)
    await app.goto(data.text.shell.routes.today)
    await expect(app.statusBarButton(t.actions)).toBeVisible()
    await expect(app.statusBarButton(t.claude)).toBeVisible()
    await expect(app.statusBar).not.toContainText(t.noProfile)
    await expect(app.statusBar).not.toContainText(t.liveNotes)
    await expect(app.statusBar).not.toContainText(t.minutesFor)
  })

  test('16 · Typing in the palette ranks the items; Enter goes to Settings: Plugins', covers('shell/TC-008', 'F-WS-03'), async ({ page }) => {
    const { shell: sh, palette: p } = data.text
    const palette = new PalettePage(page, data)
    await palette.goto(sh.routes.today)
    await palette.open(key('palette'))
    await palette.box.fill(p.plugins.typed)
    await expect(palette.first).toHaveAccessibleName(p.plugins.item)
    await expect(palette.item(p.plugins.item, p.groups.goTo)).toHaveAttribute('aria-selected', 'true')
    await page.keyboard.press(key('enter'))
    await expect(palette.dialog).toBeHidden()
    await expect.poll(() => palette.hash()).toBe(p.plugins.route)
    await expect(palette.heading(p.plugins.heading)).toBeVisible()
    await palette.open(key('palette'))
    await palette.box.fill(data.set.palette.typed)
    await expect(palette.item(data.set.palette.thread, p.groups.threads)).toBeVisible()
    await expect(palette.item(data.set.kickoff, p.groups.calls)).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(palette.dialog).toBeHidden()
  })

  test('17 · Vocabulary opens in the right panel from the palette and from the old Settings link', covers('vocabulary/TC-002', 'F-VOC-01', 'F-WS-03'), async ({ page }) => {
    const { shell: sh, palette: p } = data.text
    const palette = new PalettePage(page, data)
    await palette.goto(sh.routes.calls)
    await palette.open(key('palette'))
    await palette.box.fill(p.vocabulary.typed)
    await expect(palette.first).toHaveAccessibleName(p.vocabulary.item)
    await expect(palette.item(p.vocabulary.item, p.groups.echo)).toBeVisible()
    await page.keyboard.press(key('enter'))
    await expect(palette.dialog).toBeHidden()
    await expect(palette.panelTab(palette.rightPanel, sh.vocabulary)).toHaveAttribute('aria-selected', 'true')
    await palette.closePanel(palette.rightPanel).click()
    await expect(palette.closePanel(palette.rightPanel)).toHaveCount(0)
    await palette.goto(sh.routes.vocabulary)
    await expect(palette.panelTab(palette.rightPanel, sh.vocabulary)).toHaveAttribute('aria-selected', 'true')
  })

  test('18 · The top bar holds the sidebar button, Record, the account and Inbox on one line at every width', covers('shell/TC-017', 'F-WS-09'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    const line = { missing: [], overlap: false, sameLine: true, inOrder: true, inside: true }
    const { sizes } = data.set
    await page.setViewportSize(sizes.start)
    await app.goto(sh.routes.today)
    await expect(app.text(sh.yourDay)).toBeVisible()
    await expect(app.account).toHaveText(data.set.account)
    if (await app.closePanel(app.rightPanel).count()) await app.closePanel(app.rightPanel).click()
    for (const { width, height } of sizes.topBar) {
      await page.setViewportSize({ width, height })
      expect(await app.topBarLine(), `${width} x ${height}`).toEqual(line)
    }
    await app.inbox.click()
    await expect(app.panelTab(app.rightPanel, sh.inbox)).toHaveAttribute('aria-selected', 'true')
    await app.inbox.click()
    await expect(app.closePanel(app.rightPanel)).toHaveCount(0)
  })

  test('19 · In a narrow window the sidebar and the panel open over the page', covers('shell/TC-015', 'F-WS-05', 'F-WS-01'), async ({ page }) => {
    const sh = data.text.shell
    const app = new AppPage(page, data)
    await page.setViewportSize(data.set.sizes.narrow)
    await app.goto(sh.routes.today)
    await expect(app.expandSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeHidden()
    const bar = layout().bar
    await app.expandSidebar.click()
    await expect(app.foldSidebar).toBeVisible()
    for (const name of sh.activities.slice(0, 5)) await expect(app.activityLabel(name)).toBeVisible()
    // At this width the choice is for this session only: the layout file keeps the bar as it was.
    await page.waitForTimeout(sh.saveWait)
    expect(layout().bar).toEqual(bar)
    await page.keyboard.press(key('escape'))
    await expect(app.expandSidebar).toBeVisible()
    for (const name of sh.activities) await expect(app.activityLabel(name)).toBeHidden()
    await app.inbox.click()
    await expect(app.closePanel(app.rightPanel)).toBeVisible()
    await app.closePanel(app.rightPanel).click()
    await expect(app.closePanel(app.rightPanel)).toHaveCount(0)
  })
})
