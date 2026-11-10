// S-01 Review a client call — every stage (small data): the Calls list and its search, Acme weekly sync's minutes, a
// time chip into the transcript, Find in transcript, a note of mine kept with the call, and a task's card on Today.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { WorkPage } from '../pages/work.page.mjs'

const data = useData(test, { set: 'small' })

/** Acme weekly sync's start, as its meta.json has it. */
const started = () => data.json(data.callDir(data.set.s01.acme.folder), data.text.calls.meta).started_at

/** Calls, with Acme weekly sync open on its Minutes. */
async function openAcme(page) {
  const calls = new CallsPage(page, data)
  await calls.open()
  await calls.openCall(data.set.s01.acme.title)
  return calls
}

/** My notes in the right panel: its button pressed unless the panel already shows them. */
async function showNotes(calls) {
  const tab = calls.panelTab(calls.rightPanel, data.text.calls.myNotes)
  if (!(await tab.count()) || (await tab.getAttribute('aria-selected')) !== 'true') await calls.headerButton(data.text.calls.myNotes).click()
}

scenario(test, 'S-01', 'Review a client call: find it, read the minutes, check a point, note a follow-up', () => {
  test('1 · The Calls list groups the calls by day, newest first, under Coming up today', covers('window/TC-001', 'F-CALLS-01'), async ({ page }) => {
    const s = data.set.s01
    const calls = new CallsPage(page, data)
    await calls.open()
    await expect(calls.search).toHaveAttribute('placeholder', data.text.calls.searchPlaceholder)
    await expect(calls.call(s.acme.title)).toBeVisible()
    // The list from the top: Coming up today, then each day's heading and its calls, newest first.
    const expected = s.groups.flatMap(([day, titles]) => [day, ...titles])
    const head = new RegExp(`^${literal(data.text.upcoming.head)} `)
    // Polled: the list is drawn again as the agenda and the calls come in.
    await expect.poll(async () => {
      const [first, ...rest] = await calls.lines()
      return [head.test(first), ...rest.map((l) => expected.find((x) => l === x || new RegExp(` ${literal(x)} `).test(l)))]
    }).toEqual([true, ...expected])
    const time = await calls.clock(started())
    await expect(calls.call(s.acme.title)).toHaveAccessibleName(new RegExp(`${literal(s.acme.title)} ${literal(time)} ${literal(s.acme.length)} ${literal(s.acme.firstPeople)}`))
    await expect(calls.comingUp).toHaveAccessibleName(new RegExp(` ${literal(s.next)}$`))
  })

  test('2 · Search narrows the list by a word of the title and by a person, and clearing it brings every call back', covers('window/TC-002', 'F-CALLS-03'), async ({ page }) => {
    const s = data.set.s01
    const all = s.groups.flatMap(([, titles]) => titles)
    const calls = new CallsPage(page, data)
    await calls.open()
    await expect(calls.callRows).toHaveCount(all.length)
    for (const { words, listed } of s.search) {
      await calls.search.fill(words)
      await expect(calls.callRows).toHaveCount(listed.length)
      for (const title of listed) await expect(calls.call(title)).toBeVisible()
    }
    await calls.search.fill('')
    await expect(calls.callRows).toHaveCount(all.length)
    for (const title of all) await expect(calls.call(title)).toBeVisible()
  })

  test('3 · A call with minutes opens on Minutes: its header, the AI notice, the four sections, the action items with their times; What moved in the right panel', covers('window/TC-003', 'F-CALLS-05', 'F-CALLS-06', 'F-CALLS-07', 'F-MIN-01', 'F-MIN-02', 'F-MIN-04'), async ({ page }) => {
    const s = data.set.s01
    const c = data.text.calls
    const calls = await openAcme(page)
    await expect.poll(() => calls.hash()).toMatch(new RegExp(fill(c.openOn, { tab: c.tabs.minutes })))
    await expect(calls.callTabs).toHaveText([c.minutesTab, c.transcriptTab])
    await expect(calls.heading(s.acme.title)).toBeVisible()
    await expect(calls.says(fill(c.header, { day: await calls.longDay(started()), time: await calls.clock(started()) }))).toBeVisible()
    await expect(calls.says(s.acme.length)).toBeVisible()
    await expect(calls.says(s.acme.people)).toBeVisible()
    await expect(calls.headerButton(s.acme.thread)).toBeVisible()
    await expect(calls.minutesSay(c.aiNotice)).toBeVisible()
    await expect(calls.sections).toHaveText(s.sections)
    for (const [line, time] of s.actions) await expect(calls.timeChip(line, time)).toBeVisible()
    const moved = calls.headerButton(c.whatMoved)
    if ((await moved.getAttribute('aria-pressed')) !== 'true') await moved.click()
    await expect(calls.panelTab(calls.rightPanel, c.whatMoved)).toHaveAttribute('aria-selected', 'true')
    await expect(calls.panelSays(s.acme.thread)).toBeVisible()
    expect(await calls.minutesFill()).toBe(true)
  })

  test('4 · A time chip in the minutes opens the transcript on its turn, highlighted and in view', covers('window/TC-004', 'F-MIN-03'), async ({ page }) => {
    const s = data.set.s01
    const c = data.text.calls
    const calls = await openAcme(page)
    await calls.timeChip(s.chip.line, s.chip.time).click()
    await expect(calls.tab(c.transcriptTab)).toHaveAttribute('aria-selected', 'true')
    await expect.poll(() => calls.hash()).toMatch(new RegExp(fill(c.openOn, { tab: c.tabs.transcript })))
    await expect(calls.turn(s.jump.turn)).toContainText(s.jump.says)
    await expect(calls.turn(s.jump.turn)).toBeInViewport()
    await expect.poll(() => calls.highlighted()).toEqual([s.jump.turn])
  })

  test('5 · Find in transcript highlights exactly the turns with the word, and nothing once cleared', covers('window/TC-005', 'F-TR-01', 'F-TR-02'), async ({ page }) => {
    const s = data.set.s01
    const calls = await openAcme(page)
    await calls.tab(data.text.calls.transcriptTab).click()
    await expect(calls.turns).toHaveCount(s.turns.length)
    expect(await calls.turnNames()).toEqual(s.turns)
    await calls.find.fill(s.find.word)
    await expect.poll(() => calls.highlighted()).toEqual(s.find.hits)
    await calls.find.fill('')
    await expect.poll(() => calls.highlighted()).toEqual([])
  })

  test('6 · A note added in My notes is saved, kept after a reload and in the call\'s my-notes.md', covers('window/TC-007', 'F-NOTES-01', 'F-NOTES-02', 'F-NOTES-06'), async ({ page }) => {
    const s = data.set.s01
    const c = data.text.calls
    const n = c.notes
    const file = path.join(data.callDir(s.acme.folder), n.file)
    try {
      const calls = await openAcme(page)
      await showNotes(calls)
      await expect(calls.panelTab(calls.rightPanel, c.myNotes)).toHaveAttribute('aria-selected', 'true')
      await expect(calls.panelSays(n.empty)).toBeVisible()
      await expect(calls.note(s.note)).toHaveCount(0)
      // The save held while the window says so, then let go.
      const release = await calls.holdRequests()
      await calls.newNote.click()
      await calls.newNote.fill(s.note)
      await calls.newNote.press(data.text.shell.keys.enter)
      await expect(calls.note(s.note)).toBeVisible()
      await expect(calls.panelSays(n.saving)).toBeVisible()
      release()
      await expect(calls.panelSays(n.saved)).toBeVisible()
      await expect.poll(() => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '')).toContain(s.note)

      await page.reload()
      await calls.openCall(s.acme.title)
      await showNotes(calls)
      await expect(calls.note(s.note)).toBeVisible()
      expect(fs.readFileSync(file, 'utf8')).toContain(s.note)
    } finally {
      // Teardown: the call's notes file removed.
      fs.rmSync(file, { force: true })
    }
    expect(fs.existsSync(file)).toBe(false)
  })

  test("7 · A task card in My work opens the task's details on the right, over Today", covers('today/TC-003', 'F-TODAY-03', 'F-TASK-07'), async ({ page }) => {
    const t = data.set.tasks.acme
    const w = data.text.work
    const today = new TodayPage(page, data)
    const details = new WorkPage(page, data)
    await today.goto(data.text.shell.routes.today)
    await expect(today.taskCard(t.title)).toBeVisible()
    await today.openTaskCard(t.title)
    await expect(today.panelTab(today.rightPanel, w.details)).toHaveAttribute('aria-selected', 'true')
    expect(today.hash()).toBe(data.text.shell.routes.today)
    await expect(details.itemTitle).toHaveText(t.title)
    await expect(details.detail(t.path)).toBeVisible()
    await expect(details.value(w.owner)).toHaveText(t.owner)
    await expect(details.status).toHaveText(data.set.s01.status)
    await expect(details.sendToClaude).toHaveText(w.sendTo)
    await expect(details.edit).toHaveText(w.edit)
    expect(today.hash()).toBe(data.text.shell.routes.today)
  })
})
