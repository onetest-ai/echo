// S-02 Plan the afternoon — every stage (small data, built by the spec just before its first test): Your day's accepted
// meetings and its line now, the calendar's Day and Week, a meeting's card beside it, and a meeting set to record from
// Today and taken back on the calendar.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CalendarPage } from '../pages/calendar.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'

const data = useData(test, { set: 'small' })

/** A meeting of today's agenda, as `echoctl status` gives it (its start and end). */
const meeting = (subject) => {
  const r = data.cli('status')
  expect(r.code).toBe(0)
  return r.json().agenda.find((m) => m.subject === subject)
}
/** The meetings set to record (`.echo/scheduled.json`), by subject; none while the file isn't there. */
const scheduledFile = () => path.join(data.home, data.text.cal.scheduled)
const scheduled = () => (fs.existsSync(scheduledFile()) ? JSON.parse(fs.readFileSync(scheduledFile(), 'utf8')).map((m) => m.subject) : [])

/** A meeting card's line of when at a moment: "<when> · in N min", or "in N h" from an hour on (the window rounds). */
function whenAt(when, start, at) {
  const m = Math.round((new Date(start) - at) / 60000)
  return m < 60 ? fill(data.text.cal.inMin, { when, n: Math.max(1, m) }) : fill(data.text.cal.inHours, { when, n: Math.round(m / 60) })
}

/** Today, once Your day is drawn. */
async function openToday(page) {
  const today = new TodayPage(page, data)
  await today.goto(data.text.shell.routes.today)
  await expect(today.yourDay).toBeVisible()
  return today
}

scenario(test, 'S-02', 'Plan the afternoon: what\'s left today, the calendar, set a meeting to record and change your mind', () => {
  test('1 · Your day lists only the meetings the user accepted, in time order; the greeting counts them', covers('today/TC-001', 'F-TODAY-02', 'F-TODAY-01'), async ({ page }) => {
    const s = data.set.s02
    const today = await openToday(page)
    await expect(today.section(data.text.shell.yourDay)).toBeVisible()
    expect(await today.dayRows()).toEqual(s.accepted)
    await expect(today.dayRow(s.notAccepted)).toHaveCount(0)
    await expect(today.daySays(s.notAccepted)).toHaveCount(0)
    await expect(today.textStarting(await today.longDay())).toContainText(fill(data.text.today.meetings, { n: s.accepted.length }))
  })

  test('2 · The line now sits between the last past meeting and the first coming one, in the recording colour (coral); the coming ones offer Prepare and Record when it starts', covers('today/TC-002', 'F-TODAY-02'), async ({ page }) => {
    const s = data.set.s02
    const t = data.text.today
    const today = await openToday(page)
    const before = fill(t.now, { time: await today.clock(new Date().toISOString()) })
    const lines = await today.dayLines()
    const after = fill(t.now, { time: await today.clock(new Date().toISOString()) })
    const at = lines.findIndex((n) => n === before || n === after)
    expect(at).toBeGreaterThan(0)
    expect(lines[at - 1]).toBe(s.lastPast)
    expect(lines.slice(at + 1)).toEqual(s.coming)
    expect(await today.nowColour()).toBe(await today.themeColour(data.text.cal.colours.now))
    for (const subject of s.coming) {
      await expect(today.dayButton(subject, t.prepare)).toBeVisible()
      await expect(today.dayButton(subject, t.record)).toBeVisible()
    }
  })

  test("3 · The calendar's Day and Week show today's meetings, the recorded ones green, and the line now; › and Today move by a week and back", covers('window/TC-009', 'F-CAL-01', 'F-CAL-02', 'F-CAL-03', 'F-CAL-04', 'F-CAL-06'), async ({ page }) => {
    const s = data.set.s02
    const c = data.text.cal
    const cal = new CalendarPage(page, data)
    await cal.open(c.routes.day)
    await expect(cal.title).toHaveText(await cal.longDay())
    await expect(cal.view(c.day)).toHaveAttribute('aria-pressed', 'true')
    await expect(cal.says(c.notice)).toBeVisible()
    const day = cal.column(await cal.columnName())
    for (const subject of s.agenda) {
      await expect(cal.event(subject, day)).toHaveAccessibleName(new RegExp(`^${literal(subject)} ${literal(await cal.clock(meeting(subject).start))}`))
    }
    const green = await cal.themeColour(c.colours.recorded)
    for (const subject of s.agenda) expect([subject, await cal.edge(subject)]).toEqual([subject, s.recorded.includes(subject) ? green : expect.not.stringMatching(literal(green))])
    const accent = await cal.themeColour(c.colours.now)
    expect(await cal.nowColours(day)).toEqual({ line: accent, dot: accent })
    expect(await cal.nowBetween(day, s.lastPast, s.coming[0])).toBe(true)

    await cal.view(c.week).click()
    await expect(cal.view(c.week)).toHaveAttribute('aria-pressed', 'true')
    const week = async (from) => Promise.all(Array.from({ length: c.days }, (_, i) => cal.columnName(from + i)))
    await expect.poll(() => cal.columnNames()).toEqual(await week(c.from))
    const todays = cal.column(await cal.columnName())
    await expect(todays).toHaveAttribute('aria-current', 'date')
    expect(await cal.shaded(todays)).toBe(true)
    expect(await cal.shaded(cal.column(await cal.columnName(c.from)))).toBe(false)
    for (const subject of s.agenda) await expect(cal.event(subject, todays)).toBeVisible()
    await expect(cal.now(todays)).toBeVisible()
    expect(await cal.nowBetween(todays, s.lastPast, s.coming[0])).toBe(true)
    await cal.later.click()
    await expect.poll(() => cal.columnNames()).toEqual(await week(c.from + c.days))
    await cal.today.click()
    await expect.poll(() => cal.columnNames()).toEqual(await week(c.from))
    await expect(cal.column(await cal.columnName())).toHaveAttribute('aria-current', 'date')
  })

  test("4 · A meeting's card opens beside it inside the window, stays with it on scroll, and closes", covers('window/TC-010', 'F-CAL-07', 'F-CAL-08'), async ({ page }) => {
    const { card: k } = data.set.s02
    const c = data.text.cal
    const t = data.text.today
    const m = meeting(k.subject)
    const cal = new CalendarPage(page, data)
    await cal.open(c.routes.week)
    const todays = cal.column(await cal.columnName())
    await expect(cal.event(k.subject, todays)).toBeVisible()
    await cal.event(k.subject).click()
    const card = cal.card(k.subject)
    await expect(card).toBeVisible()
    await expect(cal.cardHeading(k.subject)).toBeVisible()
    await expect(cal.cardButton(k.subject, c.close)).toBeVisible()
    const when = fill(c.when, { start: await cal.clock(m.start), end: await cal.clock(m.end) })
    const early = whenAt(when, m.start, Date.now())
    const read = await cal.cardWhen(k.subject, when)
    expect([early, whenAt(when, m.start, Date.now())]).toContain(read)
    await expect(cal.cardSelect(k.subject, c.kind)).toBeVisible()
    await expect(cal.cardButton(k.subject, c.addThread)).toBeVisible()
    await expect(cal.cardSays(k.subject, c.people)).toBeVisible()
    await expect(cal.cardSays(k.subject, k.organizer)).toBeVisible()
    await expect(cal.cardSays(k.subject, c.organizer)).toBeVisible()
    expect(await cal.cardPlace(k.subject, t.record, t.prepare)).toEqual({ inside: true, beside: true, level: true, oneLine: true })

    const top = await cal.top(k.subject)
    await cal.scrollGrid(todays, data.set.s02.scroll)
    await expect.poll(() => cal.top(k.subject)).toBeLessThan(top)
    await expect.poll(() => cal.cardPlace(k.subject, t.record, t.prepare)).toEqual({ inside: true, beside: true, level: true, oneLine: true })
    await cal.cardButton(k.subject, c.close).click()
    await expect(card).toHaveCount(0)
  })

  test("5 · Record when it starts sets a coming meeting to record on Today and in the calendar; Don't record on its card takes it back", covers('calendar/TC-001', 'F-CAL-09'), async ({ page }) => {
    const { schedule: x } = data.set.s02
    const c = data.text.cal
    const t = data.text.today
    const today = new TodayPage(page, data)
    try {
      await openToday(page)
      await expect(today.dayButton(x.subject, t.prepare)).toBeVisible()
      await today.dayButton(x.subject, t.record).click()
      await expect(today.dayButton(x.subject, t.dontRecord)).toBeVisible()
      await expect(today.dayButton(x.other, t.record)).toBeVisible()
      await expect.poll(scheduled).toEqual([x.subject])

      const cal = new CalendarPage(page, data)
      await cal.open(c.routes.day)
      await expect(cal.event(x.subject)).toContainText(c.willRecord)
      expect(await cal.edge(x.subject)).toBe(await cal.themeColour(c.colours.scheduled))
      await cal.event(x.subject).click()
      await expect(cal.cardButton(x.subject, t.dontRecord)).toBeVisible()
      // The sentence whole, as the line's first (Prepare's sentence follows it on the same line).
      await expect(cal.cardStarting(x.subject, c.starts)).toBeVisible()
      await cal.cardButton(x.subject, t.dontRecord).click()
      await expect(cal.cardButton(x.subject, t.record)).toBeVisible()
      await expect.poll(scheduled).toEqual([])
    } finally {
      // Teardown: the meeting no longer set to record, even when a check failed: Don't record on Today, as a person does.
      if (scheduled().length) {
        await openToday(page)
        await today.dayButton(x.subject, t.dontRecord).click()
        await expect.poll(scheduled).toEqual([])
      }
    }
    expect(scheduled()).toEqual([])
  })
})
