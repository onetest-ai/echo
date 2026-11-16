// S-09 A call's housekeeping — its stages with fixed checks (small data; its fake Claude Code).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'

const data = useData(test, { set: 'small' })

/** A key of the window's shortcuts, by its name in texts.json. */
const key = (name) => data.text.shell.keys[name]
/** A thread as `echoctl project show` gives it. */
const thread = (id) => data.cli('project', 'show', id).json()

/** The window loaded on Today and done starting (My work drawn): a hash change after it is the user's. */
async function loaded(page) {
  const today = new TodayPage(page, data)
  await today.goto(data.text.shell.routes.today)
  await expect(today.closeFor(data.set.myTask)).toBeVisible()
  return new CallsPage(page, data)
}

scenario(test, 'S-09', "A call's housekeeping: list, minutes, kind, thread, tabs and the actions menu", () => {
  test('1 · Calls with no call open says Pick a call', covers('calls/TC-007', 'F-CALLS-04'), async ({ page }) => {
    const c = data.text.calls
    const calls = await loaded(page)
    // A hash change: the activity bar's Calls, never visited yet. (page.goto to #calls loads the page afresh here, and
    // a fresh load opens the newest call.)
    await calls.activity(data.text.shell.activities[1]).click()
    await expect.poll(() => calls.hash()).toBe(data.text.shell.routes.calls)
    await expect(calls.list).toBeVisible()
    await expect(calls.main.getByRole('heading', { name: c.pick })).toBeVisible()
    await expect(calls.main.getByText(c.pickLine, { exact: true })).toBeVisible()
    await expect(calls.main.getByRole('heading', { level: 2 })).toHaveCount(0)
  })

  test("2 · The list's day groups and each row's icon: Minutes or Transcript", covers('calls/TC-009', 'F-CALLS-02'), async ({ page }) => {
    const h = data.set.housekeeping
    const calls = new CallsPage(page, data)
    await calls.open()
    await expect(calls.call(h.acme.title)).toBeVisible()
    for (const g of h.groups) await expect(calls.listGroup(g)).toBeVisible()
    for (const [title, icon] of h.icons) await expect(calls.call(title)).toHaveAccessibleName(new RegExp(`^${literal(icon)} ${literal(title)} `))
  })

  test("3 · A short call's minutes: Summary and Action items with their times, nothing else", covers('calls/TC-010', 'F-MIN-01'), async ({ page }) => {
    const k = data.set.housekeeping.kickoff
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(k.title)
    await expect(calls.tab(data.text.calls.minutesTab)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.headerButton(k.thread)).toBeVisible()
    for (const [section, line, time] of k.sections) {
      await expect(calls.minutes.getByRole('heading', { name: section })).toBeVisible()
      await expect(calls.minutes.getByText(line)).toBeVisible()
      await expect(calls.minutes.getByText(time, { exact: true })).toBeVisible()
    }
    for (const section of k.absent) await expect(calls.minutes.getByRole('heading', { name: section })).toHaveCount(0)
  })

  test('4 · Call actions on a call with minutes: its items, Escape closes it', covers('calls/TC-001', 'F-CALLS-11'), async ({ page }) => {
    const a = data.set.housekeeping.acme
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(a.title)
    await expect(calls.tab(data.text.calls.minutesTab)).toHaveAttribute('aria-selected', 'true')
    await calls.callActions.click()
    await expect(page.getByRole('menu')).toBeVisible()
    await expect(calls.menuItems).toHaveText(a.menu)
    await expect(page.getByRole('menu').getByText(data.text.calls.writeAgainWith, { exact: true })).toBeVisible()
    await page.keyboard.press(key('escape'))
    await expect(page.getByRole('menu')).toHaveCount(0)
    await expect(calls.heading(a.title)).toBeVisible()
  })

  test("5 · What moved names the call's thread and offers Sync now; a call in no thread has none", covers('calls/TC-011', 'F-MIN-04', 'F-MIN-05'), async ({ page }) => {
    const { acme: a, initech: i } = data.set.housekeeping
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(a.title)
    const moved = calls.headerButton(c.whatMoved)
    if ((await moved.getAttribute('aria-pressed')) !== 'true') await moved.click()
    await expect(calls.panelTab(calls.rightPanel, c.whatMoved)).toHaveAttribute('aria-selected', 'true')
    // No count on the tab or the button (the thread hasn't read the call): their names are the bare words.
    await expect(moved).toHaveAccessibleName(c.whatMoved)
    for (const text of [a.thread.name, c.readsSoon]) await expect(calls.rightPanel.getByText(text, { exact: true })).toBeVisible()
    await expect(calls.rightPanel.getByRole('button', { name: c.syncNow, exact: true })).toBeVisible()
    await calls.openCall(i.title)
    await expect(calls.headerButton(c.whatMoved)).toHaveCount(0)
    await expect(calls.panelTab(calls.rightPanel, c.whatMoved)).toHaveCount(0)
  })

  test('6 · Call actions on a call without minutes: only Show folder, Ask Claude and Delete', covers('calls/TC-002', 'F-CALLS-11'), async ({ page }) => {
    const i = data.set.housekeeping.initech
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(i.title)
    await calls.callActions.click()
    await expect(calls.menuItems).toHaveText(i.menu)
    await expect(page.getByRole('menu').getByText(data.text.calls.writeAgainWith)).toHaveCount(0)
    await page.keyboard.press(key('escape'))
    await expect(page.getByRole('menu')).toHaveCount(0)
  })

  test('7 · + Thread puts a call in a thread and takes it out', covers('calls/TC-004', 'F-CALLS-09'), async ({ page }) => {
    const { initech: i, pin, threads } = data.set.housekeeping
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(i.title)
    for (const name of threads) await expect(calls.headerButton(name)).toHaveCount(0)
    await calls.headerButton(c.addThread).click()
    for (const name of threads) {
      await expect(calls.threadItem(name)).toHaveAccessibleName(name)
      await expect(calls.threadItem(name)).toHaveAttribute('aria-checked', 'false')
    }
    await expect(calls.menuItems).toHaveText([c.newThreadItem])
    await calls.threadItem(pin.name).click()
    await expect(calls.headerButton(pin.name)).toBeVisible()
    await expect.poll(() => thread(pin.thread).calls.find((x) => x.title === i.title)?.why).toBe(pin.why)
    await calls.headerButton(c.addThread).click()
    await expect(calls.threadItem(pin.name)).toHaveAccessibleName(fill(c.tick, { name: pin.name }))
    await expect(calls.threadItem(pin.name)).toHaveAttribute('aria-checked', 'true')
    await expect(calls.threadItem(threads[0])).toHaveAccessibleName(threads[0])
    await calls.threadItem(pin.name).click()
    await expect(calls.headerButton(pin.name)).toHaveCount(0)
    await expect.poll(() => thread(pin.thread).calls.some((x) => x.title === i.title)).toBe(false)
    expect(thread(pin.thread).exclusions).toContain(data.callName(i.folder).slice(0, 17))

    // Teardown: the exclusion cleared, the rules decide again.
    expect(data.cli('project', 'reset', pin.thread, data.callName(i.folder)).code).toBe(0)
    expect(thread(pin.thread).exclusions).toEqual([])
  })

  test("8 · The kind of meeting: a guess until you set it; yours is kept", covers('calls/TC-003', 'F-CALLS-05', 'F-CALLS-08'), async ({ page }) => {
    const { acme: a, initech: i, kinds } = data.set.housekeeping
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(a.title)
    await expect(calls.kindChip(c.guess)).toHaveText(new RegExp(`^${literal(fill(c.kindChip, { kind: a.kind }))}`))
    await expect(calls.kind.getByRole('option')).toHaveText(kinds)
    await calls.kind.selectOption({ label: a.kindSet })
    await expect(calls.kindChip(c.yours)).toHaveText(new RegExp(`^${literal(fill(c.kindChip, { kind: a.kindSet }))}`))
    await expect(calls.kindChip(c.guess)).toHaveCount(0)
    await calls.goto(data.text.shell.routes.calls)
    await calls.openCall(a.title)
    await expect(calls.kindChip(c.yours)).toHaveText(new RegExp(`^${literal(fill(c.kindChip, { kind: a.kindSet }))}`))
    await calls.openCall(i.title)
    await expect(calls.kindChip(c.guess)).toHaveText(new RegExp(`^${literal(fill(c.kindChip, { kind: i.kind }))}`))

    // Teardown: Acme weekly sync back to Client (still yours).
    await calls.openCall(a.title)
    await calls.kind.selectOption({ label: a.kind })
    await expect(calls.kindChip(c.yours)).toHaveText(new RegExp(`^${literal(fill(c.kindChip, { kind: a.kind }))}`))
  })

  test("9 · A call's thread chip, added by hand, opens the thread, whose Calls lists it", covers('calls/TC-005', 'F-CALLS-09'), async ({ page }) => {
    const a = data.set.housekeeping.acme
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(a.title)
    await expect(calls.headerButton(c.addThread)).toBeVisible()
    await expect(calls.headerButton(a.thread.name)).toHaveAccessibleDescription(c.pinnedBy)
    await calls.headerButton(a.thread.name).click()
    await expect.poll(() => calls.hash()).toMatch(new RegExp(`^${literal(fill(data.text.clients.routes.thread, { id: a.thread.id }))}`))
    await expect(calls.heading(a.thread.name)).toBeVisible()
    await calls.tab(data.text.clients.tabs.calls).click()
    await expect(calls.main.getByRole('button', { name: a.title, exact: true })).toBeVisible()
  })

  test('10 · The tab stays from call to call', covers('calls/TC-006', 'F-CALLS-06', 'F-CALLS-07'), async ({ page }) => {
    const { acme: a, initech: i } = data.set.housekeeping
    const c = data.text.calls
    const calls = new CallsPage(page, data)
    await calls.open()
    await calls.openCall(a.title)
    await expect(calls.tab(c.minutesTab)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.main.getByRole('tab')).toHaveText([c.minutesTab, c.transcriptTab])
    await calls.tab(c.transcriptTab).click()
    await expect.poll(() => calls.hash()).toMatch(new RegExp(fill(c.openOn, { tab: c.tabs.transcript })))
    await expect(calls.turns).toHaveCount(a.turns)
    await calls.openCall(i.title)
    await expect(calls.tab(c.transcriptTab)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.turns).toHaveCount(i.turns)
    await calls.tab(c.minutesTab).click()
    await expect(calls.main.getByRole('heading', { name: c.noMinutes })).toBeVisible()
    await calls.openCall(a.title)
    await expect(calls.tab(c.minutesTab)).toHaveAttribute('aria-selected', 'true')
  })

  test("11 · A call that can't be opened: said once, not polled; Calls leaves it for the newest call", covers('calls/TC-008', 'F-CALLS-16'), async ({ page }) => {
    const h = data.set.housekeeping
    const c = data.text.calls
    const calls = await loaded(page)
    await expect(calls.text(data.text.shell.yourDay)).toBeVisible()
    await page.goto(`${data.base}/${fill(c.route, { name: h.missing, tab: c.tabs.minutes })}`)
    await expect(calls.main.getByRole('heading', { name: c.cantOpen })).toBeVisible()
    await expect(calls.main.getByText(new RegExp(`^${literal(c.error)}`))).toBeVisible()
    // What the window asks for the missing call while it refreshes (every 2 s): one retry at most, not one a tick.
    const missing = []
    let refreshes = 0
    const seen = (r) => {
      if (r.status() === 404 && r.url().includes(h.missing)) missing.push(r.url())
      if (r.url().includes(c.refresh)) refreshes++
    }
    page.on('response', seen)
    await expect.poll(() => refreshes, { timeout: 20_000 }).toBeGreaterThanOrEqual(c.refreshes)
    page.off('response', seen)
    expect(missing.length).toBeLessThanOrEqual(c.most404)
    await calls.activity(data.text.shell.activities[1]).click()
    await expect(calls.main.getByRole('heading', { name: c.cantOpen })).toHaveCount(0)
    await expect.poll(() => calls.hash()).toMatch(new RegExp(fill(c.openOn, { tab: c.tabs.minutes })))
    expect(calls.hash()).not.toContain(h.missing)
    await calls.openCall(h.acme.title)
    await expect(calls.heading(h.acme.title)).toBeVisible()
    await expect(calls.tab(c.minutesTab)).toHaveAttribute('aria-selected', 'true')
  })

  test('12 · A run again that fails keeps the previous minutes; what it wrote goes to the trash', covers('claude/TC-018', 'F-MIN-13'), async () => {
    const { acme: a, failingRun: f } = data.set.housekeeping
    const r = data.text.calls.run
    const bin = path.join(data.dir, f.bin)
    // A fake that writes over the minutes, then fails, in place of the data's fake.
    fs.renameSync(path.join(bin, f.claude), path.join(bin, f.dataFake))
    try {
      fs.writeFileSync(path.join(bin, f.claude), f.script, { mode: 0o755 })
      const dir = data.callDir(a.folder)
      const before = data.read(dir, r.notes)
      const cli = data.cli('notes', dir, '--wait')
      expect(cli.code).toBe(1)
      expect(cli.err).toContain(`${f.says}. ${r.kept}`)
      expect(cli.err).toContain(r.log)
      expect(data.read(dir, r.notes)).toBe(before)
      expect(before.split('\n')[0]).toBe(a.firstLine)
      expect(data.json(dir, r.runFile)).toMatchObject({ state: r.failed, had_minutes: true, message: `${f.says}. ${r.kept}` })
      const trash = path.join(data.home, r.trash)
      const kept = fs.readdirSync(trash).filter((n) => n.includes(`-${a.folder}.notes-`))
      expect(kept).toHaveLength(1)
      expect(data.read(trash, kept[0], r.notes).trim()).toBe(f.wrote)
      expect(fs.readdirSync(dir).filter((n) => n.includes(r.beforeRun))).toEqual([])
      const log = data.read(dir, r.log).trim().split('\n').slice(-2).join('\n')
      expect(log).toContain(r.back)
      expect(log).toContain(path.basename(r.trash))
    } finally {
      // Teardown: the data's fake back.
      fs.renameSync(path.join(bin, f.dataFake), path.join(bin, f.claude))
    }
  })
})
