// S-24 The calendar, preps and the Inbox on a busy day — its stages with fixed checks (standard data): a series' rule on
// its meetings' cards (stage 5), the cards' preps (8), Zoom and Google Meet invites shown and never armed (9–10), the
// prep's header (13–14), a series set to Record and kept (17), Import Teams transcript without Microsoft 365 (29), Won't
// do's comment (36), Today's widgets (41–55), and two tests beside the stages.
// Not here (for now): the other stages, among them stages 9–10's Your day steps, 37–40 (Needs you now's Won't do through
// ⌘K is new, #297), prep/TC-012 step 5's people line (the header's answers are new, #286), and of Today's widgets: the
// small set's work line (today/TC-023 step 6, another data set) and "instead of" (step 3: the standard day has none).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal, ROOT } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { CalendarPage } from '../pages/calendar.page.mjs'
import { TodayWidgetsPage } from '../pages/today-widgets.page.mjs'

const data = useData(test, { set: 'standard' })
const set = () => data.set.s24
const c = () => data.text.cal
/** `echoctl series list`, read as JSON. */
const series = () => data.cli('series', 'list').json()
/** A short wait that checks a click changed nothing (no request lands later), not a wait for a condition. */
const seconds = (n) => new Promise((resolve) => setTimeout(resolve, n * 1000))
/** What the data's `open` was asked to show so far (nothing before its first call). */
const opened = () => (data.exists(data.dir, data.text.recording.openLog) ? data.read(data.dir, data.text.recording.openLog) : '')
/** Today's widgets in order (`echoctl today`), a widget's data (`echoctl widget show`), its catalog entry. */
const layout = () => data.cli('today').json()
const shown = (id) => data.cli('widget', 'show', id).json()
const catalogOf = (id) => data.text.widgets.catalog.find((w) => w.id === id)
/** The test plugin with a widget of each kind (tests/fixtures/plugins), installed in this data's own config; and taken
 *  off again with its marketplace. */
const installDemo = () => {
  const p = data.text.widgets.plugin
  const add = data.cli('plugin', 'marketplace', 'add', path.join(ROOT, p.folder), '--name', p.market)
  if (add.code !== 0) throw new Error(add.err)
  const r = data.cli('plugin', 'install', p.id, '--yes')
  if (r.code !== 0) throw new Error(r.err)
  return r
}
const removeDemo = () => {
  const p = data.text.widgets.plugin
  data.cli('plugin', 'remove', p.id)
  data.cli('plugin', 'marketplace', 'remove', p.market)
}

scenario(test, 'S-24', 'The calendar, preps and the Inbox on a busy day', () => {
  test("5 · A recurring meeting's card shows its series' rule — Record, Ask or Don't record, and its language; pressing the chosen one changes nothing", covers('calendar/TC-018', 'F-CAL-13'), async ({ page }) => {
    const s = set().series
    const cal = new CalendarPage(page, data)
    await cal.open(c().routes.day)
    const shows = async (m) => {
      await cal.openSeries(m.subject)
      await expect(cal.cardSays(m.subject, c().everyMeeting)).toBeVisible()
      for (const choice of s.choices) await expect(cal.seriesButton(m.subject, choice)).toHaveAttribute('aria-pressed', String(choice === m.pressed))
      await expect(cal.cardChosen(m.subject, c().language, m.language)).toHaveCount(1)
    }
    await shows(s.acme)
    await shows(s.globex)
    await cal.seriesButton(s.globex.subject, s.globex.pressed).click()
    await seconds(1)
    expect(series()[s.globex.key].record).toBe(s.list[s.globex.key].record)
    await expect(cal.seriesButton(s.globex.subject, s.globex.pressed)).toHaveAttribute('aria-pressed', 'true')
    await cal.openWith(s.single.subject, c().importButton)
    await expect(cal.cardSays(s.single.subject, c().everyMeeting)).toHaveCount(0)
    expect(series()).toMatchObject(s.list)
    await cal.cardButton(s.single.subject, c().close).click()
    await expect(cal.card(s.single.subject)).toHaveCount(0)
  })

  test('8 · A meeting\'s card offers Show prep (and Prepare again), Prepare, or nothing, by its prep and its time', covers('prep/TC-012', 'F-PREP-06', 'F-PREP-03'), async ({ page }) => {
    const p = set().preps
    const cal = new CalendarPage(page, data)
    await cal.open(c().routes.day)
    await cal.openWith(p.readyToCome, c().showPrep)
    await expect(cal.cardButton(p.readyToCome, c().showPrep)).toBeVisible()
    await expect(cal.cardButton(p.readyToCome, c().prepareAgain)).toBeVisible()
    await cal.openWith(p.readyPast, c().showPrep)
    await expect(cal.cardButton(p.readyPast, c().prepareAgain)).toHaveCount(0)
    await cal.openWith(p.noneToCome, c().prepare)
    await expect(cal.cardButton(p.noneToCome, c().prepare)).toBeVisible()
    await expect(cal.card(p.noneToCome).getByText(new RegExp(`${literal(c().prepareLine)}$`))).toBeVisible()
    await expect(cal.cardButton(p.noneToCome, c().showPrep)).toHaveCount(0)
    await cal.openWith(p.nonePast, c().importButton)
    for (const b of [c().showPrep, c().prepare, c().prepareAgain]) await expect(cal.cardButton(p.nonePast, b)).toHaveCount(0)
    await expect(cal.card(p.nonePast).getByText(c().prepareLine)).toHaveCount(0)
    await cal.openWith(p.readyPast, c().showPrep)
    await cal.cardButton(p.readyPast, c().showPrep).click()
    await expect.poll(() => cal.hash()).toMatch(new RegExp(p.prepId))
    const meeting = new MeetingPage(page, data)
    await expect(meeting.header).toHaveText(p.readyPast)
    await expect(meeting.section(data.text.prep.inShort)).toBeVisible()
    // The meeting is over: no Write it again.
    await expect(meeting.writeAgain).toHaveCount(0)
  })

  test('9 · A Zoom or a Google Meet invite, its link only in its body, shows in the calendar with its app named', covers('calendar/TC-025', 'F-CAL-17'), async ({ page }) => {
    const o = set().otherApps
    const agenda = data.status().agenda
    for (const m of [o.zoom, o.meet]) expect(agenda.find((e) => e.id === m.id).join_url).toBe(m.url)
    const cal = new CalendarPage(page, data)
    await cal.open(c().routes.day)
    for (const m of [o.zoom, o.meet]) await expect(cal.event(m.subject)).toHaveAccessibleName(new RegExp(` ${literal(m.app)}$`))
    // Only those two name an app: the Teams meetings (Acme help desk sync among them) none.
    await expect(cal.appTagged(c().apps)).toHaveCount(2)
    for (const m of [o.zoom, o.meet]) {
      await cal.event(m.subject).click()
      await expect(cal.cardHeading(m.subject)).toBeVisible()
      await expect(cal.cardSays(m.subject, m.app)).toBeVisible()
      await expect(cal.cardSays(m.subject, m.organizer)).toBeVisible()
      await expect(cal.cardSays(m.subject, c().organizer)).toBeVisible()
      await cal.cardButton(m.subject, c().close).click()
    }
  })

  test('10 · A Zoom or Google Meet meeting is never armed: no Record on its card or menu, `arm` and `join` refuse it', covers('calendar/TC-026', 'F-CAL-18', 'F-CAL-08', 'F-CAL-16'), async ({ page }) => {
    const o = set().otherApps
    const cal = new CalendarPage(page, data)
    await cal.open(c().routes.day)
    for (const m of [o.zoom, o.meet]) {
      await cal.event(m.subject).click()
      for (const b of c().recordButtons) await expect(cal.cardButton(m.subject, b)).toHaveCount(0)
      await expect(cal.cardStarting(m.subject, fill(c().notYet, { app: m.app }))).toBeVisible()
      await cal.cardButton(m.subject, c().close).click()
    }
    await cal.openWith(o.teams.subject, c().recordWhen)
    await cal.cardButton(o.teams.subject, c().close).click()
    expect(await cal.menuOf(o.zoom.subject)).toEqual(o.menu)
    await page.keyboard.press(data.text.shell.keys.escape)
    const arm = data.cliIn(JSON.stringify({ id: o.zoom.id, subject: o.zoom.subject, join_url: o.zoom.url }), 'arm', '--meeting', '-')
    expect(arm.code).toBe(1)
    expect(arm.err.trim()).toBe(fill(c().armRefused, { subject: o.zoom.subject, app: o.zoom.app }))
    const before = opened()
    const join = data.cliIn(JSON.stringify({ subject: o.meet.subject, join_url: o.meet.url }), 'join', '-')
    expect(join.code).toBe(1)
    expect(join.err.trim()).toBe(fill(c().armRefused, { subject: o.meet.subject, app: o.meet.app }))
    expect(opened()).toBe(before)
    expect(data.status().armed).toBeNull()
    expect(data.exists(data.home, data.text.app.stateDir, data.text.app.armedFile)).toBe(false)
  })

  test("13 · A meeting's header shows each person's answer, the most important first, who declined last; a hover and +N more tell the rest", covers('prep/TC-008', 'F-PREP-02'), async ({ page }) => {
    const m = data.set.meetings.globexWeekly
    const meeting = new MeetingPage(page, data)
    await meeting.open(m.prep)
    await expect(meeting.answers(m.answers)).toBeVisible()
    expect(await meeting.people()).toEqual(m.people)
    expect(await meeting.isBold(m.key)).toBe(true)
    expect(await meeting.isBold(m.notKey)).toBe(false)
    await expect(meeting.person(m.newcomer)).toContainText('new')
    await expect(meeting.person(m.key)).not.toContainText('new')
    const card = await meeting.hoverCard(m.key)
    for (const line of m.card) await expect(card).toContainText(line)
    expect(await meeting.morePeople()).toEqual(m.more)
  })

  test("14 · A prep reads in one column, with no Who's coming", covers('prep/TC-009', 'F-PREP-04'), async ({ page }) => {
    const meeting = new MeetingPage(page, data)
    await meeting.open(data.set.meetings.sponsorUpdate.prep)
    await expect(meeting.section(data.text.prep.inShort)).toBeVisible()
    await expect(meeting.section(data.text.prep.whosComing)).toHaveCount(0)
    await expect(meeting.prepFooter).toBeVisible()
  })

  test('17 · Record chosen for a series is kept after a reload and is what `echoctl series list` says', covers('calendar/TC-019', 'F-CAL-13'), async ({ page }) => {
    const s = set().series
    const m = s.globex
    const cal = new CalendarPage(page, data)
    let listed, back
    try {
      await cal.open(c().routes.day)
      await cal.openSeries(m.subject)
      await expect(cal.seriesButton(m.subject, m.pressed)).toHaveAttribute('aria-pressed', 'true')
      await cal.seriesButton(m.subject, s.choices[0]).click()
      for (const choice of s.choices) await expect(cal.seriesButton(m.subject, choice)).toHaveAttribute('aria-pressed', String(choice === s.choices[0]))
      await expect.poll(() => series()[m.key].record).toBe(s.set)
      await page.reload()
      await cal.openSeries(m.subject)
      await expect(cal.seriesButton(m.subject, s.choices[0])).toHaveAttribute('aria-pressed', 'true')
      listed = true
    } finally {
      back = data.cli('series', 'set', m.key, s.back)
      await page.reload()
    }
    expect(listed).toBe(true)
    expect(back.code, back.err).toBe(0)
    expect(series()[m.key].record).toBe(s.list[m.key].record)
  })

  test('29 · Import Teams transcript without Microsoft 365 says why on the card; the meeting stays unrecorded, no call made', covers('calendar/TC-014', 'F-CAL-15', 'F-CAL-08'), async ({ page }) => {
    const m = set().import
    const cal = new CalendarPage(page, data)
    await cal.open(c().routes.day)
    await cal.openWith(m.subject, c().importButton)
    await cal.cardButton(m.subject, c().importButton).click()
    await expect(cal.cardSays(m.subject, c().notSignedIn)).toBeVisible({ timeout: 10_000 })
    expect(await cal.openEdge(m.subject)).not.toBe(await cal.themeColour(c().colours.recorded))
    expect(fs.readdirSync(path.join(data.home, 'calls')).filter((n) => n.endsWith(`-${m.slug}`))).toHaveLength(m.calls)
    await cal.cardButton(m.subject, c().close).click()
  })

  test("36 · Won't do needs a comment; Cancel and Escape keep the item and decide nothing", covers('today/TC-007', 'F-TODAY-04'), async ({ page }) => {
    const { chat, chatLine, chatFrom } = data.set.needs
    const today = new TodayPage(page, data)
    await today.open()
    await today.wontDo(chatFrom)
    await expect(today.wontConfirm).toBeDisabled()
    await today.wontComment.fill('   ')
    await expect(today.wontConfirm).toBeDisabled()
    await today.wontComment.fill('x')
    await expect(today.wontConfirm).toBeEnabled()
    await today.wontCancel.click()
    await expect(today.wontDialog).toBeHidden()
    await today.wontDo(chatFrom)
    await page.keyboard.press('Escape')
    await expect(today.wontDialog).toBeHidden()
    await expect(today.item(chat, chatLine)).toBeVisible()
    await expect(today.asks).toHaveCount(0)
    expect(data.json(data.home, data.text.needs.file).decided || {}).toEqual({})
  })

  // ── Today's widgets (stages 41–52): each puts Today back as it comes (`echoctl today reset`) whatever happens ──

  test('41 · `echoctl widget list` gives the widgets Today can show and `widget show` the data Today gets; an unknown one names widget list', covers('today/TC-019', 'F-TODAY-10'), async ({ page }) => {
    const w = data.text.widgets
    const list = data.cli('widget', 'list')
    expect(list.code, list.err).toBe(0)
    expect(list.json()).toEqual(w.catalog)
    const today = new TodayWidgetsPage(page, data)
    await today.goto()
    expect((await today.api('widgets')).map((x) => x.id)).toEqual(w.catalog.map((x) => x.id))
    const api = await today.api('today')
    const mine = shown(w.id.mine).mine.items
    expect(mine.length).toBeGreaterThan(0)
    expect(mine.map((t) => t.title)).toEqual(api.work.mine.items.map((t) => t.title))
    const needs = shown(w.id.needs)
    expect(needs.items.length).toBeGreaterThan(0)
    expect(needs.items.map((i) => i.id)).toEqual(api.needs.items.map((i) => i.id))
    expect(needs.empty).toBe(false)
    const nope = data.cli('widget', 'show', w.id.nope)
    expect(nope.code).toBe(1)
    expect(nope.err.trim()).toBe(fill(w.unknown, { id: w.id.nope }))
    expect(layout()).toEqual(w.default)
  })

  test('42 · `echoctl today` moves, takes off, puts back and resizes widgets, and refuses what can\'t be; the page follows; reset brings Today back', covers('today/TC-020', 'F-TODAY-11'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const all = [n.yourDay, n.next, n.needs, n.choices, n.work]
    // The first stage that needs the day's choices on Today: they hold until the meetings under way end, 20 minutes after
    // the build (the case reads them within 15).
    expect(Date.now() - data.built, "the day's choices are read within 15 minutes of the data build").toBeLessThan(w.choices.within * 60_000)
    const today = new TodayWidgetsPage(page, data)
    // Today as it comes: Your day holds the first column, beside it Next and Needs you now, then the choices, My work.
    const asItComes = async () => {
      expect(layout()).toEqual(w.default)
      await expect(today.widget(n.yourDay)).toBeVisible()
      expect((await today.hold()).held).toBe(true)
      expect(await today.rows(all)).toEqual([[n.yourDay, n.next, n.needs], [n.choices], [n.work]])
      const day = await today.box(n.yourDay)
      for (const other of all.slice(1)) expect((await today.box(other)).x).toBeGreaterThanOrEqual(day.x + day.width)
    }
    try {
      await today.openToday()
      await asItComes()
      const moved = data.cli('today', 'move', id.mine, '--above', id.agenda)
      expect(moved.code, moved.err).toBe(0)
      expect(moved.json()[0]).toEqual({ row: true, size: w.size.M, widget: id.mine })
      await today.reloadToday()
      expect(await today.rows(all)).toEqual([[n.work], [n.yourDay], [n.next, n.needs], [n.choices]])
      expect((await today.hold()).held).toBe(false)
      expect(data.cli('today', 'remove', id.needs).code).toBe(0)
      await today.reloadToday()
      await expect(today.widget(n.needs)).toHaveCount(0)
      expect(shown(id.needs).items.length).toBeGreaterThan(0)
      expect(data.cli('today', 'add', id.needs, '--after', id.mine).code).toBe(0)
      await today.reloadToday()
      expect((await today.rows(all))[0]).toEqual([n.work, n.needs])
      const before = layout()
      const big = data.cli('today', 'size', id.mine, w.size.L)
      expect(big.code).toBe(1)
      expect(big.err.trim()).toBe(fill(w.takesNot, { id: id.mine, sizes: catalogOf(id.mine).sizes.join(', '), size: w.size.L }))
      expect(layout()).toEqual(before)
      const nope = data.cli('today', 'add', id.nope)
      expect(nope.code).toBe(1)
      expect(nope.err.trim()).toBe(fill(w.unknown, { id: id.nope }))
      expect(layout()).toEqual(before)
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.reloadToday()
      await asItComes()
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test('43 · Today lays itself out by its width — 3 columns, 2 with a task open on the right, 1 narrow — each row by its widgets\' sizes; Your day held, sticky, on now', covers('today/TC-021', 'F-TODAY-21', 'F-TODAY-12'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const rest = [n.next, n.needs, n.choices, n.work]
    const today = new TodayWidgetsPage(page, data)
    const near = (a, b) => expect(Math.abs(a - b)).toBeLessThanOrEqual(2)
    // Three columns: Your day holds the first (1fr 2fr), as tall as what is beside it, sticky and at most the view's
    // height, opened on now and scrolling inside; the choices across Next and Needs you now.
    const threeColumns = async () => {
      // The measure the wait saw: one taken after it may land on a frame Today's refresh is replacing (all 0).
      let h
      await expect.poll(async () => { h = await today.hold(); return h.held && h.width > 0 }).toBe(true)
      near(h.besideWidth, 2 * h.width)
      near(h.column, h.beside)
      expect(h.height).toBeLessThanOrEqual(h.view)
      expect(await today.nowInView()).toBe(true)
      expect((await today.inner(n.yourDay)).scrolls).toBe(true)
      expect(await today.rows(rest)).toEqual([[n.next, n.needs], [n.choices], [n.work]])
      const [next, needs, choices] = [await today.box(n.next), await today.box(n.needs), await today.box(n.choices)]
      near(choices.x, next.x)
      near(choices.width, needs.x + needs.width - next.x)
      return h
    }
    try {
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      const wide = await threeColumns()
      // Today scrolled: Your day stays in view.
      const top = (await today.box(n.next)).y
      await today.scrollToday(n.needs, 600)
      await expect.poll(async () => (await today.box(n.next)).y).toBeLessThan(top)
      await expect(today.widget(n.yourDay)).toBeInViewport({ ratio: 0.9 })
      await today.scrollToday(n.needs, -2000)
      // A task's Details on the right: two columns, Your day keeps the first (1fr 1fr), the rest one under another.
      const first = shown(id.mine).mine.items[0].title
      await today.openTaskCard(first)
      await expect(today.panelTab(today.rightPanel, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
      await expect.poll(async () => (await today.rows(rest)).length).toBe(4)
      const two = await today.hold()
      expect(two.held).toBe(true)
      near(two.besideWidth, two.width)
      expect(await today.nowInView()).toBe(true)
      expect(await today.rows(rest)).toEqual(rest.map((x) => [x]))
      for (const x of rest.slice(1)) near((await today.box(x)).x, (await today.box(n.next)).x)
      // The panel closed: the widths of three columns again, nothing moved.
      await today.closePanel(today.rightPanel).click()
      await expect.poll(async () => (await today.hold()).width).toBeCloseTo(wide.width, 0)
      await threeColumns()
      // Your day at M holds no column: a row of its own across two.
      expect(data.cli('today', 'size', id.agenda, w.size.M).code).toBe(0)
      await today.reloadToday()
      expect((await today.hold()).held).toBe(false)
      expect(await today.rows([n.yourDay, ...rest])).toEqual([[n.yourDay], [n.next, n.needs], [n.choices], [n.work]])
      // (Its width isn't checked: the case says across 2 columns, the app draws a fill line alone across all 3 — asked.)
      expect(data.cli('today', 'size', id.agenda, w.size.S).code).toBe(0)
      expect(layout()).toEqual(w.default)
      // Narrow: one column, Your day as the strip on top, then the rest, each across, in order.
      await today.reloadToday()
      await page.setViewportSize(w.sizes.narrow)
      await expect(today.widget(n.strip)).toBeVisible()
      await expect(today.widget(n.yourDay)).toHaveCount(0)
      const narrow = [n.strip, ...rest]
      await expect.poll(() => today.rows(narrow)).toEqual(narrow.map((x) => [x]))
      const across = async () => { const s = (await today.box(n.strip)).width; for (const x of rest) if (Math.abs((await today.box(x)).width - s) > 2) return false; return true }
      await expect.poll(across).toBe(true)
      // My work beside Next at M (Needs you now wraps). The fade shows while its list holds more than it shows (on this
      // data the row grows with My work since the held column's fix, #367: no cut — asked).
      await page.setViewportSize(w.sizes.short)
      expect(data.cli('today', 'move', id.mine, '--before', id.needs).code).toBe(0)
      expect(data.cli('today', 'size', id.mine, w.size.M).code).toBe(0)
      await today.reloadToday()
      expect(await today.rows(rest)).toEqual([[n.next, n.work], [n.needs], [n.choices]])
      const cut = await today.inner(n.work)
      expect(cut.fade).toBe(cut.scrolls && !cut.atEnd)
      expect(data.cli('today', 'reset').code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.reloadToday()
      await threeColumns()
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test("44 · Next meeting on Today: at L its prep brought to the front, at M what is expected from you, at S the meeting and its buttons", covers('today/TC-022', 'F-TODAY-13'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const x = w.next
    const next = shown(id.next).next
    const today = new TodayWidgetsPage(page, data)
    // The case's precondition: the next meeting gets a prep — another meeting's (the case checks where each part shows).
    const prep = path.join(data.home, 'prep', next.prep.id, 'prep.json')
    const had = data.exists(prep)
    try {
      if (!had) {
        fs.mkdirSync(path.dirname(prep), { recursive: true })
        fs.copyFileSync(path.join(data.home, 'prep', data.set.meetings.sponsorUpdate.prep, 'prep.json'), prep)
      }
      const { brief, people, scheduled, app, start, subject, threads } = shown(id.next).next
      expect(brief.summary).toBeTruthy()
      expect(brief.expected.length).toBeGreaterThan(0)
      expect(data.cli('today', 'size', id.next, w.size.L).code).toBe(0)
      expect(data.cli('today', 'move', id.next, '--above', id.needs).code).toBe(0)
      await today.openToday()
      // On top, across what is beside Your day, Needs you now under it.
      // (Read as soon as Today shows: wait for the row, don't read it once.)
      await expect.poll(() => today.rows([n.next, n.needs, n.choices])).toEqual([[n.next], [n.needs], [n.choices]])
      expect(Math.abs((await today.box(n.next)).width - (await today.box(n.choices)).width)).toBeLessThanOrEqual(2)
      const time = await today.clock(start)
      if (new Date(start) <= new Date()) await expect(today.widgetSays(n.next, fill(x.since, { time }))).toBeVisible()
      else await expect(today.widgetMatch(n.next, new RegExp(fill(x.comingIn, { time: literal(time) })))).toBeVisible()
      for (const t of threads) await expect(today.widgetButtonStarting(n.next, t.name)).toBeVisible()
      const coming = people.filter((p) => p.answer !== x.declined).length
      const who = [fill(coming === 1 ? x.person : x.people, { n: coming }), ...(scheduled ? [x.records] : [])].join(x.sep)
      await expect(today.widgetSays(n.next, `${subject} ${who}`).or(today.widgetSays(n.next, who))).toBeVisible()
      await expect(today.widgetHeading(n.next, x.walkIn, 3)).toBeVisible()
      const owe = (e) => (e.kind === x.reply && e.from ? fill(x.waitsFor, e) : fill(x.youOwe, e))
      const knowing = today.widgetItems(n.next, 0)
      await expect(knowing).toHaveText([brief.summary, ...brief.expected.slice(0, x.show).map(owe)])
      await expect(today.widgetButton(n.next, x.openPrep)).toBeVisible()
      await expect(today.widgetButton(n.next, x.ask)).toBeVisible()
      await expect(today.widgetButton(n.next, x.join)).toHaveCount(app === x.teams ? 1 : 0)
      // On the right: who's coming (each named with their answer, a hover says so), a newcomer named, what to ask.
      await expect(today.widgetHeading(n.next, x.coming, 3)).toBeVisible()
      const faces = today.widgetItems(n.next, x.coming)
      await expect(faces).toHaveCount(Math.min(people.length, 8))
      for (const p of people.slice(0, 8)) {
        const said = fill(x.answered, p)
        await expect(today.widgetTitled(n.next, said)).toHaveAccessibleName(said)
      }
      for (const p of people.filter((q) => q.new).slice(0, 2)) await expect(today.widgetMatch(n.next, fill(x.newcomer, p))).toBeVisible()
      await expect(today.widgetHeading(n.next, x.askThem, 3)).toBeVisible()
      await expect(today.widgetItems(n.next, -1)).toHaveText(brief.ask.slice(0, x.askShown))
      await expect(today.widgetSays(n.next, x.recording)).toHaveCount(scheduled ? 1 : 0)
      // Open prep: the prep, as Show prep in Your day opens it.
      await today.widgetButton(n.next, x.openPrep).click()
      await expect.poll(() => today.hash()).toBe(fill(x.prepRoute, { id: next.prep.id }))
      // Ask Claude about it: the Claude panel, which the data's fake Claude Code never starts in.
      await today.openToday()
      await today.widgetButton(n.next, x.ask).click()
      await expect(today.panelTab(today.bottomPanel, x.claudeTab)).toHaveAttribute('aria-selected', 'true')
      await expect(today.noticeSays(x.notStarted)).toBeVisible({ timeout: 40_000 })
      await today.noticeOk().click()
      // M: the time line, the subject, what is expected from you on one line, Show prep; no people, nothing to ask.
      expect(data.cli('today', 'size', id.next, w.size.M).code).toBe(0)
      await today.reloadToday()
      const plain = (e) => (e.kind === x.reply && e.from ? fill(x.waitsFor, e) : e.title)
      await expect(today.widgetSays(n.next, fill(x.expected, { list: brief.expected.map(plain).join(x.listSep) }))).toBeVisible()
      await expect(today.widgetButton(n.next, x.showPrep)).toBeVisible()
      for (const gone of [x.coming, x.askThem, x.walkIn]) await expect(today.widgetHeadings(n.next, gone)).toHaveCount(0)
      await expect(today.widgetButton(n.next, x.ask)).toHaveCount(0)
      // S: the time line, the subject and its buttons only.
      expect(data.cli('today', 'size', id.next, w.size.S).code).toBe(0)
      await today.reloadToday()
      await expect(today.widgetSays(n.next, subject)).toBeVisible()
      await expect(today.widgetButton(n.next, x.showPrep)).toBeVisible()
      await expect(today.widgetMatch(n.next, new RegExp(`^${literal(fill(x.expected, { list: '' }))}`))).toHaveCount(0)
    } finally {
      data.cli('today', 'reset')
      if (!had) fs.rmSync(path.dirname(prep), { recursive: true, force: true })
      await today.goto('#today')
      if (await today.closePanel(today.bottomPanel).isVisible()) await today.closePanel(today.bottomPanel).click()
    }
    expect(layout()).toEqual(w.default)
    expect(data.exists(prep)).toBe(had)
    await expect(today.bottomPanel).toBeHidden()
  })

  test("45 · Your day says where you'll be, a reason on each line: the next, choices in amber, maybes and no replies dashed, the skipped folded; ⋯ or right-click its menu, a click its card", covers('today/TC-023', 'F-TODAY-02'), async ({ page }) => {
    const w = data.text.widgets
    const x = w.day
    const s = set().widgets.day
    const today = new TodayWidgetsPage(page, data)
    const colour = (v) => today.themeColour(x.colours[v])
    await page.setViewportSize(w.sizes.wide)
    await today.openToday()
    // In time order, the line now between past and to come, past lines dimmed; the next says "now" (or "next"), violet.
    const times = await today.lineTimes()
    expect(times).toEqual([...times].sort((a, b) => a - b))
    const lines = await today.dayLines()
    const at = lines.findIndex((l) => l.startsWith(x.nowPrefix))
    expect(at).toBeGreaterThan(0)
    const next = shown(w.id.next).next
    const word = new Date(next.start) <= new Date() ? x.now : x.next
    await expect(today.lineSays(next.subject, word)).toBeVisible()
    expect((await today.lineLook(next.subject)).time).toBe(await colour('next'))
    for (const past of lines.slice(0, at).filter((l) => l !== next.subject)) expect(await today.opacityOf(today.line(past))).toBeLessThan(1)
    for (const coming of lines.slice(at + 1)) expect(await today.opacityOf(today.line(coming))).toBe(1)
    // Going: a solid green bar; maybe: dashed violet, "maybe".
    expect(await today.lineLook(s.going)).toMatchObject({ bar: 'solid', barColour: await colour('going') })
    expect(await today.lineLook(s.maybe)).toMatchObject({ bar: 'dashed', barColour: await colour('maybe') })
    await expect(today.lineMatch(s.maybe, new RegExp(`\\b${x.maybe}\\b`))).toBeVisible()
    // Meetings you accepted at once, to come: the line amber, "N others at once"; the others no lines of their own.
    await expect(today.lineMatch(s.choice, new RegExp(x.atOnce.slice(1, -1)))).toBeVisible()
    expect((await today.lineLook(s.choice)).time).toBe(await colour('choice'))
    for (const f of s.folded) await expect(today.linesOf(f)).toHaveCount(0)
    // (Step 3, "instead of": the standard day has no such line — the rule is the node test today-agenda.test.mjs.)
    // Show N skipped: the folded meetings, in time order, dimmed, each "skipped"; a no reply dashed grey; Hide folds them.
    const shownLines = (await today.dayRows()).length
    await today.showSkipped.click()
    await expect(today.skipped(x.hideSkipped)).toBeVisible()
    const all = await today.dayRows()
    const folded = all.slice(shownLines)
    expect(folded.length).toBeGreaterThan(0)
    for (const f of s.folded) expect(folded).toContain(f)
    const foldedTimes = (await today.lineTimes()).slice(times.length)
    expect(foldedTimes).toEqual([...foldedTimes].sort((a, b) => a - b))
    for (let i = 0; i < folded.length; i++) {
      const at = (await today.lineCount()) - folded.length + i
      const row = today.lineAt(at)
      await expect(today.lineAtMatch(at, new RegExp(`^${x.skipped}\\b`))).toBeVisible()
      expect(await today.opacityOf(row)).toBeLessThan(1)
      await expect(today.lineAtMatch(at, new RegExp(`^(${x.now}|${x.next})$`))).toHaveCount(0)
    }
    const noReply = folded.indexOf(s.noReply)
    expect(noReply).toBeGreaterThanOrEqual(0)
    const look = await today.lineLook(s.noReply, (await today.linesOf(s.noReply).count()) - 1)
    expect(look).toMatchObject({ bar: 'dashed', barColour: await colour('noReply') })
    await today.skipped(x.hideSkipped).click()
    await expect.poll(async () => (await today.dayRows()).length).toBe(shownLines)
    // ⋯ over a coming line: Calendar's menu for it; Escape closes it; a right-click opens the same.
    const menu = await today.dayMenu(s.choice)
    await expect(menu).toHaveAccessibleName(s.choice)
    expect(await today.menuItems()).toEqual(s.menu)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(today.menu).toHaveCount(0)
    await today.line(s.choice).click({ button: 'right', position: { x: 200, y: 8 } })
    expect(await today.menuItems()).toEqual(s.menu)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(today.menu).toHaveCount(0)
    // A click on the line: Calendar's card as a popover beside it, as in Calendar (no right panel); the line clicked
    // again closes it.
    await today.line(s.choice).click({ position: { x: 200, y: 8 } })
    const card = today.meetingCard(s.choice)
    await expect(today.meetingCardHeading(s.choice)).toBeVisible()
    await expect(today.meetingTab).toHaveCount(0)
    await expect.poll(() => today.meetingCardPlace(s.choice, today.line(s.choice))).toEqual({ inside: true, beside: true, level: true })
    await today.line(s.choice).click({ position: { x: 200, y: 8 } })
    await expect(card).toHaveCount(0)
    // Opened again, all of it shows; Close closes it.
    await today.line(s.choice).click({ position: { x: 200, y: 8 } })
    await expect(today.meetingCardHeading(s.choice)).toBeVisible()
    await expect(today.meetingCardMatch(s.choice, new RegExp(`^${literal(x.when)}`))).toBeVisible()
    await expect(today.meetingCardSelect(s.choice, s.card.kind)).toBeVisible()
    for (const b of s.card.buttons) await expect(today.meetingCardButton(s.choice, b)).toBeVisible()
    await expect(today.meetingCardMatch(s.choice, new RegExp(`^${x.people}\\b`))).toBeVisible()
    await today.meetingCardButton(s.choice, x.close).click()
    await expect(card).toHaveCount(0)
    // The menu's Show details: the same card beside the line; Close closes it.
    await today.dayMenuItem(s.choice, x.showDetails)
    await expect(today.meetingCardHeading(s.choice)).toBeVisible()
    await expect.poll(() => today.meetingCardPlace(s.choice, today.line(s.choice))).toEqual({ inside: true, beside: true, level: true })
    await today.meetingCardButton(s.choice, x.close).click()
    await expect(card).toHaveCount(0)
    // A past meeting's new minutes: violet in its reason, a click opens them.
    const m = s.minutes
    expect(await today.colourOf(today.lineLink(m.subject, m.link))).toBe(await colour('newMinutes'))
    await today.lineLink(m.subject, m.link).click()
    await expect.poll(() => today.hash()).toMatch(new RegExp(x.minutesRoute))
  })

  test('46 · Your day as a calendar (the hours down, lanes side by side, opened on now) and as a strip (the day in one bar); a block opens its card, a right-click its menu', covers('today/TC-024', 'F-TODAY-14'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const s = set().widgets.day
    const c2 = w.calendar
    const today = new TodayWidgetsPage(page, data)
    try {
      await page.setViewportSize(w.sizes.wide)
      for (const args of [['remove', id.agenda], ['add', id.calendar, '--size', c2.calendarSize, '--before', id.mine], ['add', id.strip, '--size', c2.stripSize, '--above', id.needs]]) {
        const r = data.cli('today', ...args)
        expect(r.code, r.err).toBe(0)
      }
      await today.openToday()
      const api = await today.api('today')
      // Next meeting on top, then the strip, Needs you now, the choices, and the calendar beside My work.
      expect(await today.rows([n.next, n.strip, n.needs, n.choices, n.yourDay, n.work])).toEqual([[n.next], [n.strip], [n.needs], [n.choices], [n.yourDay, n.work]])
      // The strip: a block a meeting you go to or maybe (dashed), the line now, a tick every 2 hours from 9 (or earlier) to 18 at least.
      const r = w.api
      const onStrip = api.day.filter((e) => e.start && e.kind === r.meeting && e.response !== r.noReply)
      const going = onStrip.filter((e) => e.response !== r.maybe).length
      await expect(today.widgetHeading(n.strip, fill(c2.stripHeading, { n: going }), 2)).toBeVisible()
      await expect(today.stripBlocks()).toHaveCount(onStrip.length)
      await expect(today.stripNow).toHaveCount(1)
      const ticks = await today.stripTicks()
      expect(ticks[0]).toBeLessThanOrEqual(c2.from)
      // (Every 2 hours from the first: an odd first hour ends a tick before 18.)
      expect(ticks.at(-1)).toBeGreaterThan(c2.to - c2.tick)
      ticks.slice(1).forEach((t, i) => expect(t - ticks[i]).toBe(c2.tick))
      expect(await today.blockLook(n.strip, s.maybe)).toMatchObject({ border: 'dashed' })
      // Two at once: each a part of the bar's height, one under the other.
      const [a, b] = [await today.block(n.strip, s.choice).first().boundingBox(), await today.block(n.strip, s.folded[0]).first().boundingBox()]
      const bar = await today.stripBar().boundingBox()
      expect(a.height).toBeLessThan(bar.height)
      expect(a.y === b.y).toBe(false)
      // The calendar: the hours down, a block a meeting at its time with its subject and time, overlapping ones side by
      // side; no reply dashed grey, maybe dashed violet; past dimmed; the line now in view (opened on now).
      const hours = await today.calendarHours()
      expect(hours[0]).toBeLessThanOrEqual(c2.from)
      expect(hours.at(-1)).toBeGreaterThanOrEqual(c2.to - 1)
      hours.slice(1).forEach((h, i) => expect(h - hours[i]).toBe(1))
      await expect(today.calendarBlocks).toHaveCount(api.day.filter((d) => d.start).length)
      await expect(today.block(n.yourDay, s.going)).toContainText(await today.clock(api.day.find((e) => e.subject === s.going).start))
      const [p, q] = [await today.block(n.yourDay, s.folded[0]).first().boundingBox(), await today.block(n.yourDay, s.folded[1]).first().boundingBox()]
      expect(Math.abs(p.y - q.y)).toBeLessThanOrEqual(2)
      expect(p.x === q.x).toBe(false)
      expect(await today.blockLook(n.yourDay, s.noReply)).toMatchObject({ border: 'dashed', colour: await today.themeColour(w.day.colours.noReply) })
      expect(await today.blockLook(n.yourDay, s.maybe)).toMatchObject({ border: 'dashed', colour: await today.themeColour(w.day.colours.maybe) })
      expect(await today.opacityOf(today.block(n.yourDay, s.going).first())).toBeLessThan(1)
      expect(await today.calendarNowInView()).toBe(true)
      // A block clicked (calendar, then strip): its card as a popover beside it; right-clicked (calendar or strip):
      // Calendar's menu for it.
      for (const [block, at] of [[today.block(n.yourDay, s.choice).last(), { position: { x: 6, y: 4 } }], [today.block(n.strip, s.choice).last(), {}]]) {
        await block.click(at)
        await expect(today.meetingCard(s.choice)).toBeVisible()
        await expect(today.meetingTab).toHaveCount(0)
        await expect.poll(() => today.meetingCardPlace(s.choice, block)).toEqual({ inside: true, beside: true, level: true })
        await today.meetingCardButton(s.choice, w.day.close).click()
        await expect(today.meetingCard(s.choice)).toHaveCount(0)
      }
      await today.block(n.yourDay, s.choice).last().click({ button: 'right', position: { x: 6, y: 4 } })
      expect(await today.menuItems()).toEqual(s.menu)
      await page.keyboard.press(data.text.shell.keys.escape)
      await expect(today.menu).toHaveCount(0)
      await today.block(n.strip, s.choice).last().click({ button: 'right' })
      expect(await today.menuItems()).toEqual(s.menu)
      await page.keyboard.press(data.text.shell.keys.escape)
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.reloadToday()
      expect((await today.hold()).held).toBe(true)
      await expect(today.widget(n.strip)).toHaveCount(0)
      await expect(today.calendarList).toHaveCount(0)
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test('47 · Edit Today frames each widget: sizes, moves, ×, a drag onto an edge or a half; Add a widget searches, previews and adds; Reset and Done', covers('today/TC-025', 'F-TODAY-15'), async ({ page }) => {
    const w = data.text.widgets
    const { id } = w
    const e = w.edit
    const label = (i) => catalogOf(i).label
    const [day, next, needs, choices, cards, mine, glance, strip] = [id.agenda, id.next, id.needs, id.choices, id.cards, id.mine, id.glance, id.strip].map(label)
    const at = (i) => layout().findIndex((x) => x.widget === i)
    const today = new TodayWidgetsPage(page, data)
    try {
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      await today.editToday.click()
      // The head: Add a widget, Reset to the default, Done; each widget framed with its moves, sizes and ×.
      for (const b of [today.addWidget, today.resetLayout, today.done]) await expect(b).toBeVisible()
      for (const item of w.default) {
        const l = label(item.widget)
        await expect(today.frame(l)).toBeVisible()
        for (const z of catalogOf(item.widget).sizes) await expect(today.frameSize(l, z)).toHaveAttribute('aria-pressed', String(z === item.size))
        await expect(today.frameRemove(l)).toBeVisible()
      }
      await expect(today.frameMove(day, e.moves.down)).toBeVisible()
      for (const m of [e.moves.up, e.moves.left, e.moves.right]) await expect(today.frameMove(day, m)).toHaveCount(0)
      await expect(today.frameMove(mine, e.moves.up)).toBeVisible()
      await expect(today.frameMove(mine, e.moves.down)).toHaveCount(0)
      await expect(today.frameSays(cards, catalogOf(id.cards).about + e.onlyWhile)).toBeVisible()
      // Its content shown but inert: a click on Next meeting's thread chip (which opens the thread) does nothing.
      const prep = await today.frameInnerButton(next).boundingBox()
      await page.mouse.click(prep.x + prep.width / 2, prep.y + prep.height / 2)
      // Nothing happening gives no event to wait for: a second lets a late navigation show before the hash is read.
      await seconds(1)
      expect(today.hash()).toBe(data.text.shell.routes.today)
      // Add a widget on the right: search, parts, a row a widget ("on Today" or Add), the picked one's preview.
      await expect(today.panelTab(today.rightPanel, e.tab)).toHaveAttribute('aria-selected', 'true')
      await expect(today.widgetSearch).toBeVisible()
      await expect(today.partButtons).toHaveText(e.parts)
      expect(await today.catalogLabels()).toEqual(w.catalog.map((x) => x.label))
      for (const x of w.catalog) {
        const on = w.default.some((d) => d.widget === x.id)
        await expect(today.catalogOn(x.label)).toHaveCount(on ? 1 : 0)
        await expect(today.catalogAdd(x.label)).toHaveCount(on ? 0 : 1)
      }
      await expect(today.anyPreview).toBeVisible()
      // Sizes: S, then M again (its default).
      await today.frameSize(mine, w.size.S).click()
      await expect.poll(() => layout()[at(id.mine)].size).toBe(w.size.S)
      await today.frameSize(mine, w.size.M).click()
      await expect.poll(() => layout()[at(id.mine)].size).toBe(w.size.M)
      await expect(today.frameSize(mine, w.size.M)).toHaveAttribute('aria-pressed', 'true')
      // My work dragged onto Needs you now's top edge: the line above Needs while over it; dropped, a row of its own above.
      // (The window made tall enough for both frames to be in view: a drag can't scroll.)
      await page.setViewportSize(w.sizes.tall)
      await today.hoverDrag(today.frame(mine), today.frame(needs), w.side.above)
      await expect.poll(() => today.dropMark(needs)).toBe(w.side.above)
      await page.mouse.up()
      await expect.poll(() => at(id.mine)).toBe(at(id.needs) - 1)
      expect(layout()[at(id.mine)].row).toBe(true)
      expect(layout()[at(id.needs)].row).toBe(true)
      await expect.poll(() => today.frameRows([mine, needs])).toEqual([[mine], [needs]])
      // Threads at a glance (not on Today) dragged from the panel onto My work's right half: beside it, after it.
      await today.catalogRow(glance).click()
      await expect(today.preview(glance, w.size.M)).toBeVisible()
      await today.hoverDrag(today.catalogRow(glance), today.frame(mine), w.side.after)
      await expect.poll(() => today.dropMark(mine)).toBe(w.side.after)
      await page.mouse.up()
      await expect.poll(() => at(id.glance)).toBe(at(id.mine) + 1)
      expect(layout()[at(id.glance)].row).toBeFalsy()
      // × takes it off; the panel offers Add again.
      await today.frameRemove(glance).click()
      await expect.poll(() => at(id.glance)).toBe(-1)
      await expect(today.catalogAdd(glance)).toBeVisible()
      await page.setViewportSize(w.sizes.wide)
      // The strip takes M and L: S greyed; Add at M puts it on a new last row.
      await today.catalogRow(strip).click()
      await expect(today.pickSize(w.size.S)).toBeDisabled()
      await today.pickSize(w.size.M).click()
      await today.addAt(w.size.M).click()
      await expect.poll(() => layout().at(-1)).toEqual({ row: true, size: w.size.M, widget: id.strip })
      // Today read again after the add (the panel then says "on Today"), before typing: a refresh while the search has
      // the keyboard takes it away (reported, not this case's step).
      await expect(today.catalogOn(strip)).toBeVisible()
      // The search keeps the keyboard, a letter at a time: Calendar's widgets listed, the part back on All.
      await today.part(catalogOf(id.mine).part).click()
      await today.widgetSearch.click()
      await today.widgetSearch.pressSequentially(e.search, { delay: 60 })
      await expect(today.widgetSearch).toBeFocused()
      await expect(today.widgetSearch).toHaveValue(e.search)
      await expect(today.part(e.parts[0])).toHaveAttribute('aria-pressed', 'true')
      const hits = w.catalog.filter((x) => `${x.label} ${x.about} ${x.part}`.toLowerCase().includes(e.search)).map((x) => x.label)
      expect(hits).toContain(next)
      expect(await today.catalogLabels()).toEqual(hits)
      // Done: no frames, the panel closed; Today keeps the changes after a reload.
      const kept = layout()
      await today.done.click()
      await expect(today.frame(mine)).toHaveCount(0)
      await expect(today.panelTab(today.rightPanel, e.tab)).toHaveCount(0)
      await today.reloadToday()
      expect(layout()).toEqual(kept)
      await expect(today.widget(w.name.strip)).toBeVisible()
      // Reset to the default: Today as it comes, still in edit mode with Add a widget open (narrower: one under another).
      await today.editToday.click()
      await today.resetLayout.click()
      await expect.poll(() => layout()).toEqual(w.default)
      await expect(today.done).toBeVisible()
      await expect(today.panelTab(today.rightPanel, e.tab)).toHaveAttribute('aria-selected', 'true')
      await expect.poll(() => today.frameRows([day, next, needs, choices, cards, mine])).toEqual([[day, next], [needs], [choices], [cards], [mine]])
      expect((await today.hold(day)).held).toBe(true)
      // By keyboard: ↑ on My work moves it to a row above the one before it; the keyboard stays on its move button.
      await today.frameMove(mine, e.moves.up).focus()
      await page.keyboard.press(data.text.shell.keys.enter)
      await expect.poll(() => at(id.mine)).toBe(at(id.cards) - 1)
      await expect(today.frameMove(mine, e.moves.up)).toBeFocused()
      expect(await today.framesInRows([day, next, needs, choices, cards, mine])).toBe(true)
      // × by keyboard: off Today, the keyboard on the next widget's ×.
      await today.frameRemove(next).focus()
      await page.keyboard.press(data.text.shell.keys.enter)
      await expect.poll(() => at(id.next)).toBe(-1)
      await expect(today.frameRemove(needs)).toBeFocused()
      // Narrow: Add a widget in the head opens the panel again (over the page).
      await page.setViewportSize(w.sizes.narrow)
      // (The panel may close by itself as the window narrows: then there is nothing to close.)
      await today.closePanel(today.rightPanel).click({ timeout: 3_000 }).catch(() => {})
      await expect(today.widgetSearch).toBeHidden()
      await today.addWidget.click()
      await expect(today.panelTab(today.rightPanel, e.tab)).toBeVisible()
      await expect(today.widgetSearch).toBeVisible()
      await today.panelDone.click()
      await expect(today.frame(mine)).toHaveCount(0)
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test("48 · The day's choices: meetings at once with the one Echo suggests and why, invites with its answer; Keep asks before declining; an answer without Microsoft 365 says why and sends nothing", covers('today/TC-026', 'F-TODAY-16'), async ({ page }) => {
    const w = data.text.widgets
    const x = w.choices
    const s = set().widgets.choices
    // Its lines hold until the meetings under way end, 20 minutes after the build (the case reads them within 15).
    expect(Date.now() - data.built, 'stage 48 reads the choices within 15 minutes of the data build').toBeLessThan(x.within * 60_000)
    const today = new TodayWidgetsPage(page, data)
    const sameButtons = async (name, suggested) => {
      const b = await today.choiceButtons(name)
      const pick = b.find((y) => y.name === suggested)
      expect(pick).toBeTruthy()
      for (const y of b.filter((z) => z !== pick)) {
        expect(y.filled).not.toBe(pick.filled)
        expect(y.filled).toBe(b.find((z) => z !== pick).filled)
      }
      return b
    }
    try {
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      await expect(today.widgetHeading(w.name.choices, fill(x.heading, { n: s.count }), 2)).toBeVisible()
      expect(await today.choices()).toEqual(s.order)
      const times = []
      for (const l of s.order) times.push((await today.choiceTime(l)).text)
      // A line of meetings at once: its time amber, "A or B or …", Echo's suggestion and why, a Keep each, the pick filled.
      const first = s.order[0]
      expect((await today.choiceTime(first)).colour).toBe(await today.themeColour(w.day.colours.choice))
      await expect(today.choiceSays(first, fill(x.accepted, { n: s.first.members.length, pick: s.first.pick, why: s.first.why }))).toBeVisible()
      for (const m of s.first.members) await expect(today.choiceMeeting(first, m)).toBeVisible()
      const keeps = await sameButtons(first, fill(x.keep, { name: s.first.pick }))
      expect(keeps.map((b) => b.name)).toEqual(s.first.members.map((m) => fill(x.keep, { name: m })))
      // The invites: one where you go (Decline), one at a free time between the two lines of meetings at once (Accept).
      for (const inv of [s.busy, s.free]) {
        const name = fill(x.invite, inv)
        await expect(today.choiceSays(name, fill(x.notAnswered, inv))).toBeVisible()
        await expect(today.choiceSays(name, fill(x.suggests, inv))).toBeVisible()
        expect((await sameButtons(name, inv.how)).map((b) => b.name)).toEqual(x.answers)
      }
      const free = s.order.indexOf(fill(x.invite, s.free))
      expect(s.order[free - 2].startsWith(fill(x.atOnce, { names: '' }))).toBe(true)
      expect(s.order[free + 1].startsWith(fill(x.atOnce, { names: '' }))).toBe(true)
      expect(times).toEqual([...times].sort((a, b) => new Date(`2000-01-01 ${a}`) - new Date(`2000-01-01 ${b}`)))
      // Keep one that isn't suggested: nothing sent, the others named, yours stays; Cancel brings the Keeps back.
      const before = { layout: layout(), agenda: data.status().agenda }
      await today.choiceButton(first, fill(x.keep, { name: s.first.keep })).click()
      await expect(today.choiceContains(first, fill(x.confirm, { names: s.first.declined.join(x.namesSep) }))).toBeVisible()
      await expect(today.choiceContains(first, fill(x.yours, { names: s.first.yours.join(x.namesSep) }))).toBeVisible()
      await expect(today.choiceButton(first, x.declineThem)).toBeFocused()
      await today.choiceButton(first, x.cancel).click()
      await expect(today.choiceButton(first, fill(x.keep, { name: s.first.keep }))).toBeFocused()
      await expect(today.choiceAllButtons(first)).toHaveCount(s.first.members.length)
      // Accept without Microsoft 365: Sending…, the buttons off, then the notice says why; nothing answered.
      expect(data.exists(data.env.ECHOCTL_CONFIG_DIR, data.text.m365.token)).toBe(false)
      const freeLine = fill(x.invite, s.free)
      await today.choiceButton(freeLine, s.free.how).click()
      await expect(today.choiceButton(freeLine, x.sending)).toBeDisabled()
      // Read at one moment while it sends: the clicked one "Sending…", all three off.
      await expect.poll(() => today.choiceStates(freeLine)).toEqual([{ text: x.sending, disabled: true }, ...x.answers.slice(1).map((text) => ({ text, disabled: true }))])
      await expect(today.noticeSays(x.refused)).toBeVisible({ timeout: 15_000 })
      await expect(today.noticeCode()).toHaveText(x.command)
      await expect(today.choiceAllButtons(freeLine)).toHaveText(x.answers)
      for (const b of x.answers) await expect(today.choiceButton(freeLine, b)).toBeEnabled()
      await today.noticeOk().click()
      expect(layout()).toEqual(before.layout)
      expect(data.status().agenda).toEqual(before.agenda)
      expect(data.exists(data.env.ECHOCTL_CONFIG_DIR, data.text.m365.token)).toBe(false)
      // A meeting's name on a line, then an invite's line: its card as a popover beside what was clicked.
      await today.choiceMeeting(first, s.first.meeting).click()
      await expect(today.meetingCard(s.first.meeting)).toBeVisible()
      await expect(today.meetingTab).toHaveCount(0)
      await expect.poll(() => today.meetingCardPlace(s.first.meeting, today.choiceMeeting(first, s.first.meeting))).toEqual({ inside: true, beside: true, level: true })
      await today.meetingCardButton(s.first.meeting, w.day.close).click()
      await expect(today.meetingCard(s.first.meeting)).toHaveCount(0)
      await today.choice(freeLine).click({ position: { x: 100, y: 10 } })
      await expect(today.meetingCard(s.free.subject)).toBeVisible()
      await expect.poll(() => today.meetingCardPlace(s.free.subject, today.choice(freeLine))).toEqual({ inside: true, beside: true, level: true })
      await today.meetingCardButton(s.free.subject, w.day.close).click()
      await expect(today.meetingCard(s.free.subject)).toHaveCount(0)
      // Taken off, gone; reset, back (it is in the default).
      expect(data.cli('today', 'remove', w.id.choices).code).toBe(0)
      await today.reloadToday()
      await expect(today.widget(w.name.choices)).toHaveCount(0)
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.reloadToday()
      await expect(today.widget(w.name.choices)).toBeVisible()
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test('49 · Threads at a glance (a bar a thread: done this week, to do, waiting, and its total) and Done this week, at M and at L / S; a name opens its Work, a task its details', covers('today/TC-027', 'F-TODAY-17'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const x = w.glance
    const s = set().widgets.glance
    const today = new TodayWidgetsPage(page, data)
    try {
      const g = shown(id.glance)
      expect(g.since).toMatch(/^\d{4}-\d\d-\d\d$/)
      expect(new Date(`${g.since}T12:00:00`).getDay()).toBe(1)
      expect(g.threads).toHaveLength(s.threads)
      expect(g.threads[0].name).toBe(s.first)
      expect(g.threads.slice(-s.last.length).map((t) => t.name).sort()).toEqual(s.last)
      for (const t of g.threads.slice(-s.last.length)) expect(t).toMatchObject({ todo: 1, total: 1 })
      for (let i = 1; i < g.threads.length; i++) expect(g.threads[i].total).toBeLessThanOrEqual(g.threads[i - 1].total)
      for (const d of g.done) expect(d.thread).toBeTruthy()
      const ons = g.done.map((d) => d.on)
      expect(ons).toEqual([...ons].sort().reverse())
      expect(data.cli('today', 'add', id.glance, '--size', x.glanceSize, '--above', id.needs).code).toBe(0)
      expect(data.cli('today', 'add', id.done, '--size', x.doneSize, '--after', id.glance).code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      expect((await today.rows([n.glance, n.done, n.needs]))[0]).toEqual([n.glance, n.done])
      // At M: five threads, each its name, its bar (green, amber, red, each as wide as its count) and its total; the key.
      const top = g.threads.slice(0, x.atM)
      expect(await today.glanceLines()).toEqual(top.map((t) => fill(x.line, t)))
      for (const t of top) {
        await expect(today.glanceTotal(t.name)).toHaveText(String(t.total))
        const looks = []
        for (const [k, title, colour] of x.segments) {
          if (!t[k]) { await expect(today.segment(t.name, fill(title, { n: 0 }))).toHaveCount(0); continue }
          const look = await today.segmentLook(t.name, fill(title, { n: t[k] }))
          expect(look.colour).toBe(await today.themeColour(colour))
          looks.push([look.width, t[k]])
        }
        for (const [width, count] of looks) expect(Math.abs(width / count - looks[0][0] / looks[0][1])).toBeLessThan(2)
      }
      for (const k of x.key) await expect(today.widgetSays(n.glance, k)).toBeVisible()
      // Done this week: the number since Monday, up to four with their thread.
      await expect(today.widgetSays(n.done, String(g.done.length))).toBeVisible()
      await expect(today.widgetSays(n.done, fill(x.doneCount, { s: g.done.length === 1 ? '' : 's' }))).toBeVisible()
      const doneRows = today.widgetItems(n.done)
      await expect(doneRows).toHaveCount(Math.min(g.done.length, x.doneShown))
      for (const [i, d] of g.done.slice(0, x.doneShown).entries()) await expect(doneRows.nth(i)).toHaveText(`${d.title}${d.thread}`)
      // A thread's name: its Work; a done task: its details on the right.
      await today.glanceLink(top[0].name).click()
      await expect.poll(() => today.hash()).toBe(fill(x.workRoute, top[0]))
      await today.openToday()
      await today.widgetButton(n.done, g.done[0].title).click()
      await expect(today.panelTab(today.rightPanel, data.text.work.details)).toHaveAttribute('aria-selected', 'true')
      await expect(today.detailsHeading(g.done[0].title)).toBeVisible()
      await today.closePanel(today.rightPanel).click()
      // At L ten threads and "2 more in Threads" (it opens Threads); Done this week at S only the number.
      expect(data.cli('today', 'size', id.glance, x.large).code).toBe(0)
      expect(data.cli('today', 'size', id.done, x.small).code).toBe(0)
      await today.reloadToday()
      expect((await today.glanceLines()).map((l) => l.split(':')[0])).toEqual(s.atL)
      for (const t of s.atL.slice(5)) await expect(today.glanceTotal(t)).toHaveText('1')
      await expect(today.widgetSays(n.done, String(g.done.length))).toBeVisible()
      await expect(today.widgetItems(n.done)).toHaveCount(0)
      await today.widgetButton(n.glance, fill(x.more, { n: s.more })).click()
      await expect.poll(() => today.hash()).toBe(x.threadsRoute)
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.openToday()
      await expect(today.widget(n.glance)).toHaveCount(0)
      await expect(today.widget(n.done)).toHaveCount(0)
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test('50 · Recorded this week: the calls since Monday (memos counted), the time and how many have minutes, at M a bar a day; a click opens Calls; `widget show` gives the same', covers('today/TC-028', 'F-TODAY-18'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const x = w.week
    const today = new TodayWidgetsPage(page, data)
    try {
      const v = shown(id.week)
      expect(Object.keys(v).sort()).toEqual([...x.fields].sort())
      expect(v.days.map((d) => d.date)[0]).toBe(v.since)
      expect(v.days).toHaveLength(7)
      // The same counted from the calls' folders, since Monday up to today (by the date their name starts with).
      const local = (iso) => { const t = new Date(iso); return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(t.getDate()).padStart(2, '0')}` }
      const calls = fs.readdirSync(path.join(data.home, 'calls')).map((f) => {
        const meta = data.json(data.home, 'calls', f, x.meta)
        const date = (/^(\d{4}-\d\d-\d\d)/.exec(f) || [])[1] || local(meta.started_at)
        return { date, meta, notes: data.exists(data.home, 'calls', f, x.notes) }
      }).filter((c) => c.date >= v.since && c.date <= v.today)
      const minutes = calls.reduce((a, c) => a + (c.meta.ended_at ? (new Date(c.meta.ended_at) - new Date(c.meta.started_at)) / 60000 : 0), 0)
      expect(v).toMatchObject({ calls: calls.length, memos: calls.filter((c) => c.meta.kind === x.memo).length, with_minutes: calls.filter((c) => c.notes).length, recorded_min: Math.round(minutes) })
      for (const d of v.days) expect(d.calls).toBe(calls.filter((c) => c.date === d.date).length)
      expect(v.memos).toBeGreaterThan(1)
      expect(v.calls).toBeGreaterThan(v.memos)
      expect(v.recorded_min).toBeGreaterThanOrEqual(60)
      // At S: the three numbers, no bars.
      expect(data.cli('today', 'add', id.week, '--size', x.S, '--above', id.needs).code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      const numbers = async () => {
        await expect(today.widgetButton(n.week, x.callsButton)).toBeVisible()
        for (const [num, label] of [[v.calls, fill(x.calls, { n: v.memos })], [Math.round(v.recorded_min / 6) / 10, x.hours], [v.with_minutes, x.withMinutes]]) {
          await expect(today.widgetSays(n.week, String(num))).toBeVisible()
          await expect(today.widgetSays(n.week, label)).toBeVisible()
        }
      }
      await numbers()
      await expect(today.weekDays()).toHaveCount(0)
      // At M: a bar a day, Monday to Sunday, as tall as its calls (the tallest full), today's day bold; nothing out of its frame.
      expect(data.cli('today', 'size', id.week, x.M).code).toBe(0)
      await today.reloadToday()
      await numbers()
      const bars = await today.weekBars()
      const most = Math.max(...v.days.map((d) => d.calls))
      // The tallest day's bar fills its track (its border aside), an empty day's is flat.
      const tallest = Math.max(...bars.map((b) => b.height))
      expect(tallest).toBeGreaterThan(bars[0].full - 4)
      for (const [i, d] of v.days.entries()) {
        const isToday = d.date === v.today
        expect(bars[i].name).toMatch(new RegExp(fill(x.day, { weekday: await today.weekday(d.date), n: d.calls, today: isToday ? x.todayMark : '' })))
        expect(Math.abs(bars[i].height - (d.calls / most) * tallest)).toBeLessThanOrEqual(1)
        expect(bars[i].bold).toBe(isToday)
      }
      expect(await today.overflows(n.week)).toEqual([])
      // A click on its numbers, and on the heading's Calls: Calls.
      await today.widgetSays(n.week, x.withMinutes).click()
      await expect.poll(() => today.hash()).toMatch(new RegExp(x.callsRoute))
      await today.openToday()
      await today.widgetButton(n.week, x.callsButton).click()
      await expect.poll(() => today.hash()).toMatch(new RegExp(x.callsRoute))
      // Narrow and dark: the numbers and the seven bars inside the frame, nothing cut.
      await page.emulateMedia({ colorScheme: 'dark' })
      await page.setViewportSize(w.sizes.narrow)
      await today.openToday()
      expect(await today.windowColour()).toBe(data.text.settings.appearance.window.dark)
      await numbers()
      await expect(today.weekDays()).toHaveCount(7)
      expect(await today.overflows(n.week)).toEqual([])
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.reloadToday()
      await expect(today.widget(n.week)).toHaveCount(0)
    } finally {
      data.cli('today', 'reset')
    }
    expect(layout()).toEqual(w.default)
  })

  test("51 · A plugin's widgets on Today: counted on its install sheet, in the catalog under its name, drawn by kind with its text escaped and links only to echo://; data Echo can't draw says why", covers('today/TC-029', 'F-TODAY-19'), async ({ page }) => {
    const w = data.text.widgets
    const x = w.plugin
    const ids = x.ids
    const label = x.widgets
    const today = new TodayWidgetsPage(page, data)
    const dialogs = []
    page.on('dialog', (d) => { dialogs.push(d.message()); d.dismiss() })
    try {
      // Installed from the test marketplace, into this data's own config only.
      const install = installDemo()
      for (const line of x.sheet) expect(install.out).toContain(line)
      expect(install.out).toContain(x.installed)
      const ours = data.cli('widget', 'list').json().filter((e) => e.id.startsWith(x.prefix))
      expect(ours.map((e) => e.id)).toEqual(x.order.map((k) => ids[k]))
      for (const e of ours) expect(e).toMatchObject({ part: x.name, label: label[e.id.slice(x.prefix.length)] })
      for (const e of ours) expect(e.sizes.length).toBeGreaterThan(0)
      const builds = shown(ids.builds)
      expect(builds).toMatchObject({ kind: x.listKind, empty: false })
      expect(builds.data.items).toHaveLength(x.builds.length)
      // Put on Today: open, ci, broken at S in a row above Needs you now, sprint and builds at M, note on its own.
      for (const [k, size, ...where] of x.place) {
        const r = data.cli('today', 'add', ids[k], '--size', size, ...where)
        expect(r.code, r.err).toBe(0)
      }
      expect(data.cli('plugin', 'call', x.id, x.ciCmd, x.warn).code).toBe(0)
      expect(data.cli('plugin', 'call', x.id, x.noteCmd).code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      await expect(async () => {
        await today.reloadToday()
        for (const k of x.drawn) await expect(today.widget(label[k])).toBeVisible({ timeout: 1_000 })
      }).toPass({ timeout: 20_000 })
      const warn = await today.themeColour(w.day.colours.choice)
      await expect(today.widgetSays(label.open, x.open.value)).toBeVisible()
      expect(await today.textColour(label.open, x.open.value)).toBe(warn)
      await expect(today.widgetSays(label.open, x.open.label)).toBeVisible()
      expect(await today.dotColour(label.ci)).toBe(warn)
      await expect(today.widgetSays(label.ci, x.ci.title)).toHaveCount(2) // its heading and its status' title
      await expect(today.widgetSays(label.ci, x.ci.failed)).toBeVisible()
      await expect(today.widgetSays(label.broken, x.broken)).toBeVisible()
      for (const [value, what, colour] of x.sprint) {
        await expect(today.widgetSays(label.sprint, value)).toBeVisible()
        await expect(today.widgetSays(label.sprint, what)).toBeVisible()
        if (colour) expect(await today.textColour(label.sprint, value)).toBe(await today.themeColour(colour))
      }
      const lines = today.widgetItems(label.builds)
      await expect(lines).toHaveText(x.builds.map(([t, sub]) => (sub ? `${t}${sub}` : t)))
      await expect(today.widgetLink(label.builds, x.builds[2][0])).toHaveAttribute('href', x.taskLink)
      await expect(today.itemLinks(label.builds, 3)).toHaveCount(0)
      await expect(today.widgetImages(label.builds)).toHaveCount(0)
      await expect(today.widgetSays(label.note, x.note.title)).toBeVisible()
      await expect(today.widgetParagraph(label.note)).toHaveText(x.note.text.replace('\n', ''))
      expect(await today.widgetParagraph(label.note).innerText()).toBe(x.note.text)
      expect(dialogs).toEqual([])
      // Open the task: Echo's own place for echo://task/…, its details on the right.
      await today.widgetLink(label.builds, x.builds[2][0]).click()
      await expect.poll(() => today.hash()).toBe(x.taskRoute)
      await expect(today.detailsHeading(x.taskTitle)).toBeVisible()
      // The note cleared: gone from Today, `widget show` says it's empty.
      expect(data.cli('plugin', 'call', x.id, x.clearNote).code).toBe(0)
      await today.openToday()
      await today.reloadToday()
      await expect(today.widget(label.open)).toBeVisible()
      await expect(today.widget(label.note)).toHaveCount(0)
      expect(shown(ids.note).empty).toBe(true)
      // Add a widget, part "Widget demo": its eight with their about lines, the ones on Today "on Today".
      await today.editToday.click()
      await today.part(x.name).click()
      expect(await today.catalogLabels()).toEqual(ours.map((e) => e.label))
      for (const e of ours) {
        await expect(today.catalogRow(e.label)).toContainText(e.about)
        await expect(today.catalogOn(e.label)).toHaveCount(x.notPlaced.map((k) => ids[k]).includes(e.id) ? 0 : 1)
      }
      await today.done.click()
      expect(dialogs).toEqual([])
    } finally {
      data.cli('today', 'reset')
      removeDemo()
    }
    expect(layout()).toEqual(w.default)
    expect(data.cli('widget', 'list').json().filter((e) => e.id.startsWith(x.prefix))).toEqual([])
    expect(data.cli('plugin', 'list').out).not.toContain(x.market)
  })

  test("52 · Each routine that sends Today a card is a widget of its own: put where you want it, its card drawn there, Routine cards shows the rest; Got it hides it until the next", covers('today/TC-030', 'F-TODAY-20'), async ({ page }) => {
    const w = data.text.widgets
    const x = w.routines
    const [brief, review] = x.cards
    const today = new TodayWidgetsPage(page, data)
    const send = (card) => data.cliIn(card.text, 'deliver', 'send', 'today', '--title', card.title, '--file', '/dev/stdin', '--about', card.about)
    try {
      for (const card of x.cards) {
        const r = send(card)
        expect(r.code, r.err).toBe(0)
        expect(r.out.trim()).toBe(x.sent)
      }
      const list = data.cli('widget', 'list').json()
      const of = (card) => list.find((e) => new RegExp(card.id).test(e.id))
      for (const card of x.cards) expect(of(card)).toMatchObject({ part: x.part, label: card.about, sizes: x.sizes })
      expect(data.cli('today', 'add', of(brief).id, '--size', x.size, '--before', w.id.needs).code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      // The brief in Next meeting's row (Needs you now wraps under it), with Got it; the review in Routine cards below.
      expect(await today.rows([w.name.next, brief.title, w.name.needs])).toEqual([[w.name.next, brief.title], [w.name.needs]])
      await expect(today.widgetButton(brief.title, x.gotIt)).toBeVisible()
      await expect(today.widget(review.title)).toBeVisible()
      await expect(today.widgetsNamed(brief.title)).toHaveCount(1)
      expect((await today.box(review.title)).y).toBeGreaterThan((await today.box(w.name.needs)).y)
      // Got it: the brief gone, the review stays; sent again, the brief is back in its place.
      await today.widgetButton(brief.title, x.gotIt).click()
      await expect(today.widget(brief.title)).toHaveCount(0)
      await expect(today.widget(review.title)).toBeVisible()
      expect(send(brief).code).toBe(0)
      await today.reloadToday()
      expect(await today.rows([w.name.next, brief.title, w.name.needs])).toEqual([[w.name.next, brief.title], [w.name.needs]])
      // Reset: both in Routine cards.
      expect(data.cli('today', 'reset').code).toBe(0)
      await today.reloadToday()
      for (const card of x.cards) await expect(today.widget(card.title)).toBeVisible()
      expect((await today.box(brief.title)).y).toBeGreaterThan((await today.box(w.name.needs)).y)
    } finally {
      data.cli('today', 'reset')
      // The cards read (Got it), as the data had none; a teardown that fails can't hide the stage's own failure.
      try {
        await today.openToday()
        for (const card of x.cards) if (await today.widget(card.title).count()) await today.widgetButton(card.title, x.gotIt).click()
      } catch { /* checked below */ }
    }
    expect(layout()).toEqual(w.default)
    for (const card of x.cards) await expect(today.widget(card.title)).toHaveCount(0)
  })

  test("53 · A plugin widget's chart (each point titled) and a stat's progress bar; its outside link asked before it leaves Echo, its actions run the plugin's command", covers('today/TC-031', 'F-TODAY-22'), async ({ page }) => {
    const w = data.text.widgets
    const x = w.plugin
    const t = x.trend
    const ids = x.ids
    const today = new TodayWidgetsPage(page, data)
    const before = opened()
    try {
      installDemo()
      for (const [k, size, ...where] of x.chartPlace) {
        const r = data.cli('today', 'add', ids[k], '--size', size, ...where)
        expect(r.code, r.err).toBe(0)
      }
      expect(data.cli('plugin', 'call', x.id, x.ciCmd, x.bad).code).toBe(0)
      await page.setViewportSize(w.sizes.wide)
      await today.openToday()
      await expect(async () => {
        await today.reloadToday()
        await expect(today.chart(x.widgets.trend)).toBeVisible({ timeout: 1_000 })
      }).toPass({ timeout: 20_000 })
      // Open tickets: "12 open tickets" and a bar, 12 of 20.
      await expect(today.widgetSays(x.widgets.open, x.open.value)).toBeVisible()
      await expect(today.widgetSays(x.widgets.open, x.open.label)).toBeVisible()
      await expect(today.widgetImages(x.widgets.open)).toHaveAccessibleName(x.progress)
      // Calls a day: five bars, the last one stronger; "Mon" and "Fri · 4 calls" under them, the note; a hover on each
      // says its point.
      const marks = await today.chartMarks(x.widgets.trend)
      expect(marks).toHaveLength(t.bars)
      expect(marks.map((m) => m.says)).toContain(t.hover)
      const rest = marks.slice(0, -1)
      for (const m of rest) expect(m.fill === marks.at(-1).fill && m.opacity === marks.at(-1).opacity).toBe(false)
      for (const m of rest.slice(1)) expect([m.fill, m.opacity]).toEqual([rest[0].fill, rest[0].opacity])
      for (const text of [t.first, t.last, t.note]) await expect(today.widgetSays(x.widgets.trend, text)).toBeVisible()
      // CI failed: a red dot, its text, Open and Re-run.
      expect(await today.dotColour(x.widgets.ci)).toBe(await today.themeColour(w.plugin.badColour))
      await expect(today.widgetSays(x.widgets.ci, x.ci.failed)).toBeVisible()
      await expect(today.widgetButton(x.widgets.ci, x.ci.rerun)).toBeVisible()
      // Open: nothing leaves Echo, the question under it; Cancel takes it away.
      await today.widgetButton(x.widgets.ci, x.ci.openLink).click()
      await expect(today.widgetMatch(x.widgets.ci, x.ci.askStart)).toBeVisible()
      await expect(today.widget(x.widgets.ci)).toContainText(x.ci.ask)
      expect(opened()).toBe(before)
      await today.widgetButton(x.widgets.ci, x.ci.cancel).click()
      await expect(today.widgetMatch(x.widgets.ci, x.ci.askStart)).toHaveCount(0)
      expect(opened()).toBe(before)
      // Re-run: the plugin's command runs, CI is green, no Re-run.
      await today.widgetButton(x.widgets.ci, x.ci.rerun).click()
      await expect(today.widgetSays(x.widgets.ci, x.ci.green)).toBeVisible()
      await expect(today.widgetButton(x.widgets.ci, x.ci.rerun)).toHaveCount(0)
      expect(await today.dotColour(x.widgets.ci)).toBe(await today.themeColour(w.day.colours.going))
      const trend = shown(ids.trend)
      expect(trend).toMatchObject({ kind: t.kind, data: { type: t.type } })
      expect(trend.data.points).toHaveLength(t.bars)
    } finally {
      data.cli('today', 'reset')
      removeDemo()
    }
    expect(layout()).toEqual(w.default)
    expect(data.cli('widget', 'list').json().filter((e) => e.id.startsWith(x.prefix))).toEqual([])
    expect(data.cli('plugin', 'list').out).not.toContain(x.market)
  })

  test("54 · A widget's settings: Done this week counts this week or this month, a plugin's Last builds gets its branch; set with ⚙ or `echoctl today set`, refused when not one it takes, kept on Today", covers('today/TC-032', 'F-TODAY-23'), async ({ page }) => {
    const w = data.text.widgets
    const { name: n, id } = w
    const x = w.settings
    const builds = w.plugin.widgets.builds
    const buildsId = w.plugin.ids.builds
    const done = catalogOf(id.done).label
    const { period: per, branch: br } = x
    const today = new TodayWidgetsPage(page, data)
    const paramsOf = (i) => layout().find((e) => e.widget === i).params
    const s = (k) => (k === 1 ? '' : 's')
    const firstBuild = () => today.widgetItems(builds).first()
    try {
      installDemo()
      for (const [i, size, ...where] of x.place) {
        const r = data.cli('today', 'add', i, '--size', size, ...where)
        expect(r.code, r.err).toBe(0)
      }
      const week = shown(id.done)
      expect(week.period).toBe(per.week)
      await page.setViewportSize(w.sizes.short)
      await today.openToday()
      // Done this week since Monday; Last builds' first line its default branch (reloaded until the plugin answers).
      await expect(async () => {
        await today.reloadToday()
        await expect(firstBuild()).toHaveText(fill(br.line, { branch: br.main }), { timeout: 1_000 })
      }).toPass({ timeout: 20_000 })
      await expect(today.widgetHeading(n.done, x.heading.week, 2)).toBeVisible()
      await expect(today.widgetSays(n.done, String(week.done.length))).toBeVisible()
      await expect(today.widgetSays(n.done, fill(x.count.week, { s: s(week.done.length) }))).toBeVisible()
      // Refused: a value it doesn't take, a setting it doesn't have (Echo's widget and the plugin's); nothing changed.
      const before = layout()
      for (const [i, key, value, says] of x.refusals) {
        const r = data.cli('today', 'set', i, key, value)
        expect(r.code).toBe(1)
        expect(r.err.trim()).toBe(says)
      }
      expect(layout()).toEqual(before)
      // Edit Today, ⚙ on Done this week: Period under its header, week chosen; month: Done this month.
      await today.editToday.click()
      await today.settingsButton(done).click()
      await expect(today.settingsOf(done)).toBeVisible()
      await expect(today.setting(done, per.label)).toHaveValue(per.week)
      await today.setting(done, per.label).selectOption(per.month)
      await expect.poll(() => paramsOf(id.done)).toEqual({ [per.key]: per.month })
      const month = shown(id.done)
      expect(month.period).toBe(per.month)
      expect(month.done.length).toBeGreaterThanOrEqual(week.done.length)
      await expect(today.frameSays(done, x.heading.month)).toBeVisible()
      await expect(today.frameSays(done, String(month.done.length))).toBeVisible()
      await expect(today.frameSays(done, fill(x.count.month, { s: s(month.done.length) }))).toBeVisible()
      // ⚙ on Last builds, Branch release; Done: the plugin asked again with it. `echoctl today` keeps both.
      await today.settingsButton(builds).click()
      await expect(today.setting(builds, br.label)).toHaveValue(br.main)
      await today.setting(builds, br.label).selectOption(br.release)
      await expect.poll(() => paramsOf(buildsId)).toEqual({ [br.key]: br.release })
      await today.done.click()
      await expect(firstBuild()).toHaveText(fill(br.line, { branch: br.release }), { timeout: 15_000 })
      expect(paramsOf(id.done)).toEqual({ [per.key]: per.month })
      // Kept after a reload.
      await today.reloadToday()
      await expect(today.widgetHeading(n.done, x.heading.month, 2)).toBeVisible()
      await expect(firstBuild()).toHaveText(fill(br.line, { branch: br.release }), { timeout: 15_000 })
      // `echoctl today set` back to week: Done this week again.
      const back = data.cli('today', 'set', id.done, per.key, per.week)
      expect(back.code, back.err).toBe(0)
      await today.reloadToday()
      await expect(today.widgetHeading(n.done, x.heading.week, 2)).toBeVisible()
      await expect(today.widgetSays(n.done, fill(x.count.week, { s: s(week.done.length) }))).toBeVisible()
    } finally {
      data.cli('today', 'reset')
      removeDemo()
    }
    expect(layout()).toEqual(w.default)
    expect(data.cli('widget', 'list').json().filter((e) => e.id.startsWith(w.plugin.prefix))).toEqual([])
    expect(data.cli('plugin', 'list').out).not.toContain(w.plugin.market)
  })

  test("55 · A plugin's own page as a widget: drawn by its window half in Echo's frame from its host half's data, with the widget's size and theme, its text escaped; off with the plugin, its place kept", covers('today/TC-033', 'F-TODAY-24'), async ({ page }) => {
    const w = data.text.widgets
    const p = w.plugin
    const x = p.board
    const board = p.widgets.board
    const today = new TodayWidgetsPage(page, data)
    const dialogs = []
    page.on('dialog', (d) => { dialogs.push(d.message()); d.dismiss() })
    const drawn = async () => {
      await expect(async () => {
        await today.reloadToday()
        await expect(today.widget(board)).toBeVisible({ timeout: 1_000 })
      }).toPass({ timeout: 20_000 })
    }
    try {
      installDemo()
      const add = data.cli('today', 'add', p.ids.board, '--size', ...x.place)
      expect(add.code, add.err).toBe(0)
      await page.emulateMedia({ colorScheme: x.theme })
      await page.setViewportSize(w.sizes.short)
      await today.openToday()
      await drawn()
      // In Echo's frame, its label on top: three boxes, the <i> shown as text, nothing italic; its box the size and theme.
      await expect(today.widgetHeading(board, x.heading, 2)).toBeVisible()
      expect(await today.pageWidget(board)).toEqual({ size: w.size.M, theme: x.theme, boxes: x.boxes, italic: false })
      // At L: drawn again at L.
      const big = data.cli('today', 'size', p.ids.board, x.large)
      expect(big.code, big.err).toBe(0)
      await today.reloadToday()
      await expect.poll(async () => (await today.pageWidget(board))?.size).toBe(x.large)
      // Its plugin off: not drawn (Today itself is; the window gets no data for it), still on Today's layout.
      const fed = async () => !!((await today.api('today')).plugins || {})[p.ids.board]
      expect(await fed()).toBe(true)
      const off = data.cli('plugin', 'disable', p.id)
      expect(off.code, off.err).toBe(0)
      await today.reloadToday()
      await expect(today.widget(w.name.yourDay)).toBeVisible()
      expect(await fed()).toBe(false)
      await expect(today.widget(board)).toHaveCount(0)
      expect(layout().map((e) => e.widget)).toContain(p.ids.board)
      // On again: back, at L.
      const on = data.cli('plugin', 'enable', p.id)
      expect(on.code, on.err).toBe(0)
      await drawn()
      expect(await today.pageWidget(board)).toEqual({ size: x.large, theme: x.theme, boxes: x.boxes, italic: false })
      expect(dialogs).toEqual([])
    } finally {
      data.cli('today', 'reset')
      removeDemo()
    }
    expect(layout()).toEqual(w.default)
    expect(data.cli('widget', 'list').json().filter((e) => e.id.startsWith(p.prefix))).toEqual([])
    expect(data.cli('plugin', 'list').out).not.toContain(p.market)
  })

  // Not a stage of the manual S-24 (it takes an item its Won't do stages need); here it runs last, after them.
  test("after · I'll do it adds the task in the chosen thread; the dialog keeps what was typed while Today is drawn again", covers('today/TC-013', 'F-TODAY-04'), async ({ page }) => {
    const { needs: n, task: t } = data.set
    const today = new TodayPage(page, data)
    await today.installClock()
    await today.open()
    await today.illDoIt(n.mailFrom)
    await expect(today.taskTitle).toHaveValue(n.mailTitle)
    await today.taskTitle.fill(t.title)
    await today.taskThread.selectOption({ label: t.thread.name })
    // A minute on, Today is drawn again (its line "now" moves): its content is replaced, the dialog isn't.
    const content = await today.main.elementHandle()
    const button = await today.addToTasks.elementHandle()
    await today.minutePasses()
    await expect.poll(() => content.evaluate((el) => el.isConnected)).toBe(false)
    expect(await button.evaluate((el) => el.isConnected)).toBe(true)
    await expect(today.taskTitle).toHaveValue(t.title)
    await expect(today.taskThread).toHaveValue(t.thread.id)
    await today.addToTasks.click()
    await expect(today.taskDialog).toBeHidden()
    await page.reload()
    await expect(today.item(n.chat, n.chatLine)).toBeVisible()
    await expect(today.item(n.mail, n.mailTitle)).toHaveCount(0)
    const description = data.text.needs.taskDescription.replace('{from}', n.mailFrom).replace('{where}', n.mail).replace('{title}', n.mailTitle)
    expect(data.lastEvent(t.thread.id)).toMatchObject({ type: 'create', kind: 'task', fields: { title: t.title, description } })
  })

  // Not a stage of the manual S-24: runs after Won't do (cancelled) and I'll do it (on the mail), on the chat's item.
  test("after · ✦ opens My work's Claude dialog on an item; Yolo works on it by its link, no task made; a failed run gives it back", covers('today/TC-014', 'F-TODAY-04'), async ({ page }) => {
    const { needs: n } = data.set
    const t = data.text.needs
    const today = new TodayPage(page, data)
    await today.open()
    expect(await today.itemButtons(n.chat, n.chatLine)).toEqual(['Reply in Teams', t.claude])
    const before = data.read(data.home, 'projects', n.chatThread.id, 'ledger/events.jsonl')
    await today.claude(n.chat, n.chatLine)
    await expect(today.claudeDialog).toContainText(`${n.chatThread.name} · ${n.chat} — ${n.chatLine}`)
    await expect(today.claudeSkill).toHaveAttribute('placeholder', new RegExp(`^${t.skillPlaceholder}`))
    await today.claudeYolo.click()
    await expect(today.item(n.chat, n.chatLine)).toHaveCount(0) // Claude is on it
    await expect(today.askBar).toContainText(data.text.fake.claudeFails, { timeout: 15_000 })
    await expect(today.askBar).toContainText(`${n.chat}: `)
    await expect(today.askBar).toContainText(t.stillThere)
    await expect(today.item(n.chat, n.chatLine)).toBeVisible() // the run failed: the item is back
    expect((data.json(data.home, data.text.needs.file).decided || {})[n.chatId]).toBeUndefined() // not handed over
    expect(data.read(data.home, 'projects', n.chatThread.id, 'ledger/events.jsonl')).toBe(before)
    // Claude got the item by its link only, and the link reads it.
    const prompt = data.yoloPrompts().at(-1)
    for (const words of t.promptHas) expect(prompt).toContain(words)
    expect(prompt).not.toContain(n.chatLine)
    const read = data.cli('read', `echo://needs/${n.chatId}`)
    expect(read.code).toBe(0)
    for (const words of t.readHas) expect(read.out).toContain(words)
    expect(read.out).toContain(n.chatReadCommand)
  })
})
