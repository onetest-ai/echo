// S-10 Inbox and prep on a quiet day — every stage (small data): the empty Inbox in the window and the CLI, the
// meetings still to come today (the fold, a meeting's page, its prep line with and without a thread, the CLI's list),
// then the day emptied: Today says it is quiet.
import { test, expect } from '@playwright/test'
import { useData, fill } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { InboxPage } from '../pages/inbox.page.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { PalettePage } from '../pages/palette.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'

const data = useData(test, { set: 'small' })

/** `echoctl prep --upcoming`: today's meetings still to come, as the CLI lists them. */
const upcoming = () => {
  const r = data.cli('prep', '--upcoming')
  expect(r.code).toBe(0)
  return r.json().meetings
}

/** Calls, Coming up today unfolded. */
async function unfold(calls) {
  await calls.open()
  await expect(calls.call(data.set.s10.listed)).toBeVisible()
  if ((await calls.comingUp.getAttribute('aria-expanded')) !== 'true') await calls.comingUp.click()
  await expect(calls.comingUp).toHaveAttribute('aria-expanded', 'true')
}

/** A meeting still to come, its row's name as the list reads it: "Coming up <subject> <time> 30 min <names>". */
async function rowName(app, m, start) {
  return fill(data.text.upcoming.rowName, { subject: m.subject, time: await app.clock(start), minutes: m.minutes, names: m.names.join(', ') })
}

scenario(test, 'S-10', 'Inbox and prep on a quiet day', () => {
  test('1 · echoctl attention prints the same empty lists as the Inbox', covers('inbox/TC-003', 'F-INBOX-08'), async ({ page }) => {
    const { s10 } = data.set
    const t = data.text.inbox
    const r = data.cli('attention')
    expect(r.code).toBe(0)
    const a = r.json()
    expect(Object.keys(a).sort()).toEqual(t.keys)
    expect(a.open).toEqual([])
    expect(a.done).toEqual([])
    expect(a.urgent).toBe(0)
    expect(a.clients).toEqual(s10.clients)
    const inbox = new InboxPage(page, data)
    await inbox.goto(data.text.shell.routes.today)
    await expect(inbox.text(data.text.shell.yourDay)).toBeVisible()
    await inbox.inbox.click()
    await expect(inbox.says(t.nothing)).toBeVisible()
  })

  test('2 · An empty Inbox says Nothing needs you; Done says nothing was decided this week', covers('inbox/TC-002', 'F-INBOX-02'), async ({ page }) => {
    const t = data.text.inbox
    const inbox = new InboxPage(page, data)
    await inbox.goto(data.text.shell.routes.today)
    await expect(inbox.text(data.text.shell.yourDay)).toBeVisible()
    await inbox.inbox.click()
    await expect(inbox.tab).toHaveAttribute('aria-selected', 'true')
    await expect(inbox.chip(t.open)).toBeVisible()
    await expect(inbox.chip(t.done)).toBeVisible()
    await expect(inbox.says(t.thisWeek)).toBeVisible()
    await expect(inbox.chip(t.blocking)).toHaveCount(0)
    await expect(inbox.chip(t.open)).toHaveAttribute('aria-pressed', 'true')
    await expect(inbox.chip(t.done)).toHaveAttribute('aria-pressed', 'false')
    await expect(inbox.says(t.nothing)).toBeVisible()
    await expect(inbox.says(t.nothingLine)).toBeVisible()
    await inbox.chip(t.done).click()
    await expect(inbox.chip(t.done)).toHaveAttribute('aria-pressed', 'true')
    await expect(inbox.says(t.nothingDecided)).toBeVisible()
    await inbox.chip(t.open).click()
    await expect(inbox.chip(t.open)).toHaveAttribute('aria-pressed', 'true')
    await expect(inbox.says(t.nothing)).toBeVisible()
    await inbox.inbox.click()
    await expect(inbox.closePanel(inbox.panel)).toHaveCount(0)
  })

  test('3 · The Inbox button opens and closes the right panel on its tab, with no badge when nothing waits', covers('inbox/TC-001', 'F-INBOX-01'), async ({ page }) => {
    const sh = data.text.shell
    const inbox = new InboxPage(page, data)
    await inbox.goto(sh.routes.today)
    await expect(inbox.text(sh.yourDay)).toBeVisible()
    await expect(inbox.inbox).toBeVisible()
    // At the top bar's right end, on the line of the Record pill.
    const line = await inbox.topBarLine()
    expect(line.missing).toEqual([])
    expect(line.sameLine).toBe(true)
    expect(line.inOrder).toBe(true)
    await expect(inbox.badge).toHaveCount(0)
    await expect(inbox.activity(sh.inbox)).toHaveCount(0)
    await inbox.inbox.click()
    await expect(inbox.tab).toHaveAttribute('aria-selected', 'true')
    await expect(inbox.inbox).toHaveAttribute('aria-pressed', 'true')
    expect(inbox.hash()).toBe(sh.routes.today)
    // To Calls from the window itself: the panel follows from view to view.
    await inbox.activity(sh.activities[1]).click()
    await expect(inbox.activity(sh.activities[1])).toHaveAttribute('aria-current', 'page')
    await expect(inbox.tab).toHaveAttribute('aria-selected', 'true')
    await inbox.inbox.click()
    await expect(inbox.closePanel(inbox.panel)).toHaveCount(0)
    await expect(inbox.inbox).toHaveAttribute('aria-pressed', 'false')
    await expect(inbox.badge).toHaveCount(0)
  })

  test('4 · Coming up today counts the meetings still to come and unfolds into rows', covers('prep/TC-001', 'F-PREP-01'), async ({ page }) => {
    const { s10 } = data.set
    const u = data.text.upcoming
    const meetings = upcoming()
    const calls = new CallsPage(page, data)
    await calls.open()
    await expect(calls.call(s10.listed)).toBeVisible()
    // The list starts with the fold's head: how many, and the next one.
    await expect(calls.rows.first()).toHaveAccessibleName(fill(u.headName, { n: s10.meetings.length, time: await calls.clock(meetings[0].start), next: s10.meetings[0].subject }))
    await expect(calls.comingUp).toHaveAttribute('aria-expanded', 'false')
    await expect(calls.upcomingRows).toHaveCount(0)
    await calls.comingUp.click()
    await expect(calls.comingUp).toHaveAttribute('aria-expanded', 'true')
    await expect(calls.upcomingRows).toHaveCount(s10.meetings.length)
    for (const [i, m] of s10.meetings.entries()) {
      const row = calls.upcomingRows.nth(i)
      await expect(row).toHaveAccessibleName(await rowName(calls, m, meetings[i].start))
      await expect(row).toHaveAccessibleDescription(u.noPrep)
    }
    await expect(calls.upcoming(s10.notListed)).toHaveCount(0)
    await calls.search.fill(s10.search)
    await expect(calls.comingUp).toHaveCount(0)
    await calls.search.fill('')
    await expect(calls.comingUp).toBeVisible()
    await expect(calls.search).toHaveValue('')
  })

  test('5 · A coming meeting opens like a call with a Prep tab and the rest disabled', covers('prep/TC-002', 'F-PREP-02'), async ({ page }) => {
    const m = data.set.s10.meetings[0]
    const u = data.text.upcoming
    const meeting = upcoming()[0]
    const calls = new CallsPage(page, data)
    const view = new MeetingPage(page, data)
    await unfold(calls)
    await calls.upcoming(m.subject).click()
    await expect.poll(() => calls.hash()).toBe(fill(u.route, { id: meeting.id }))
    await expect(calls.upcoming(m.subject)).toHaveAttribute('aria-selected', 'true')
    await expect(view.heading(m.subject)).toBeVisible()
    await expect(view.says(fill(u.when, { day: await view.longDay(meeting.start), time: await view.clock(meeting.start) }))).toBeVisible()
    await expect(view.says(fill(u.minutes, { n: m.minutes }))).toBeVisible()
    for (const name of m.names) await expect(view.person(name)).toBeVisible()
    await expect(view.addThread).toBeVisible()
    await expect(view.tab(u.prepTab)).toHaveAttribute('aria-selected', 'true')
    for (const tab of u.laterTabs) await expect(view.tab(tab)).toBeDisabled()
    await expect(view.button(u.copyLink)).toHaveCount(0)
  })

  test('6 · Putting a meeting in a thread changes the No prep yet line and shows the thread', covers('prep/TC-004', 'F-PREP-02', 'F-PREP-03'), async ({ page }) => {
    const { s10 } = data.set
    const m = s10.meetings[0]
    const u = data.text.upcoming
    const start = upcoming()[0].start
    const calls = new CallsPage(page, data)
    const view = new MeetingPage(page, data)
    await unfold(calls)
    await calls.upcoming(m.subject).click()
    await expect(view.saysStarting(u.noThreadStart)).toBeVisible()
    try {
      await view.addThread.click()
      await expect(view.threadItems).toHaveCount(s10.threads.length + 1)
      for (const [i, name] of [...s10.threads, u.newThread].entries()) await expect(view.threadItems.nth(i)).toHaveAccessibleName(name)
      await view.threadItem(s10.pin).click()
      await expect(view.chip(s10.pin)).toBeVisible()
      await expect(view.takeOut(s10.pin)).toBeVisible()
      await expect(view.says(u.inThread)).toBeVisible()
      // The row shows the thread's icon before its time.
      const row = calls.upcoming(m.subject)
      const icon = calls.rowIcon(row, s10.pin)
      await expect(icon).toBeVisible()
      const time = calls.rowText(row, await calls.clock(start))
      expect((await icon.boundingBox()).x).toBeLessThan((await time.boundingBox()).x)
      await view.takeOut(s10.pin).click()
      await expect(view.chip(s10.pin)).toHaveCount(0)
      await expect(view.saysStarting(u.noThreadStart)).toBeVisible()
    } finally {
      // The pin comes off even when a check failed: the next stages read the meeting in no thread.
      if (await view.takeOut(s10.pin).count()) await view.takeOut(s10.pin).click()
    }
  })

  test('7 · A meeting in no thread says No prep yet and offers Prepare now', covers('prep/TC-003', 'F-PREP-03'), async ({ page }) => {
    const m = data.set.s10.meetings[1]
    const u = data.text.upcoming
    const calls = new CallsPage(page, data)
    const view = new MeetingPage(page, data)
    await unfold(calls)
    await calls.upcoming(m.subject).click()
    await expect(view.heading(m.subject)).toBeVisible()
    await expect(view.tab(u.prepTab)).toHaveAttribute('aria-selected', 'true')
    await expect(view.paneHeading(u.noPrep)).toBeVisible()
    await expect(view.says(u.noThread)).toBeVisible()
    // There, not pressed: it starts Claude Code.
    await expect(view.button(u.prepareNow)).toBeVisible()
  })

  test('8 · echoctl prep --upcoming lists today\'s meetings and --show refuses a missing prep', covers('prep/TC-005', 'F-PREP-08'), async ({ page }) => {
    const { s10 } = data.set
    const u = data.text.upcoming
    const meetings = upcoming()
    expect(meetings.map((x) => x.subject)).toEqual(s10.meetings.map((m) => m.subject))
    for (const [i, m] of s10.meetings.entries()) {
      expect(meetings[i]).toMatchObject({ names: m.names, prep: null, writing: false, threads: [] })
    }
    expect(meetings.map((x) => x.subject)).not.toContain(s10.notListed)
    const id = meetings[0].id
    expect(id).not.toBe(s10.globexEvent)
    expect(id).toMatch(new RegExp(u.hash))
    const show = data.cli('prep', '--show', id)
    expect(show.code).toBe(1)
    expect(show.err.trim()).toBe(fill(u.noPrepShow, { id }))
    const calls = new CallsPage(page, data)
    await unfold(calls)
    await expect(calls.upcomingRows).toHaveCount(meetings.length)
    for (const [i, m] of s10.meetings.entries()) await expect(calls.upcomingRows.nth(i)).toHaveAccessibleName(await rowName(calls, m, meetings[i].start))
    await calls.comingUp.click()
    await expect(calls.comingUp).toHaveAttribute('aria-expanded', 'false')
  })

  test('9 · A day with no meetings, no calls and nothing on you says "A quiet day", and ⌘K on it offers no Today or My work items', covers('today/TC-006', 'F-TODAY-06'), async ({ page }) => {
    const { s10 } = data.set
    const q = data.text.quietDay
    const p = data.text.palette
    // Empty the day: the agenda cleared, today's calls to the trash, the task done.
    const cleared = data.cli('agenda', 'clear')
    expect(cleared.code).toBe(0)
    expect(cleared.out.trim()).toBe(q.agendaCleared)
    const deleted = []
    let mid
    try {
      for (const suffix of s10.todayCalls) {
        const r = data.cli('delete', data.callName(suffix))
        expect(r.code).toBe(0)
        deleted.push(r.json().deleted)
      }
      const done = data.cli('project', 'ledger', s10.task.thread, 'edit', s10.task.done)
      expect(done.code).toBe(0)
      mid = done.json().mid

      const today = new TodayPage(page, data)
      const palette = new PalettePage(page, data)
      await today.goto(data.text.shell.routes.today)
      await expect(today.greeting(fill(q.greeting, { name: s10.me }))).toBeVisible()
      await expect(today.text(await today.longDay())).toBeVisible()
      await expect(today.section(q.heading)).toBeVisible()
      await expect(today.text(q.line)).toBeVisible()
      await expect(today.section(q.yourDay)).toHaveCount(0)
      await expect(today.section(q.myWork)).toHaveCount(0)
      await palette.open(data.text.shell.keys.palette)
      await expect(palette.target(fill(p.target, { label: p.today }))).toBeVisible()
      for (const g of q.notGroups) await expect(palette.group(g)).toHaveCount(0)
      await expect(palette.groups().first()).toHaveAccessibleName(p.groups.echo)
      for (const [i, label] of q.echoFirst.entries()) await expect(palette.items(p.groups.echo).nth(i)).toHaveAccessibleName(label)
      await page.keyboard.press(data.text.shell.keys.escape)
      await expect(palette.dialog).toBeHidden()
    } finally {
      // The calls and the task back (the agenda stays empty: this stage runs last).
      for (const name of deleted) expect(data.cli('restore', name).code).toBe(0)
      if (mid) expect(data.cli('project', 'ledger', s10.task.thread, 'edit', fill(s10.task.undo, { mid })).code).toBe(0)
    }
  })
})
