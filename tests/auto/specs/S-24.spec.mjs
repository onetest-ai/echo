// S-24 The calendar, preps and the Inbox on a busy day — its stages with fixed checks (standard data): a series' rule on
// its meetings' cards (stage 5), the cards' preps (8), Zoom and Google Meet invites shown and never armed (9–10), the
// prep's header (13–14), a series set to Record and kept (17), Import Teams transcript without Microsoft 365 (29), Won't
// do's comment (36), Today's widgets (41–52), and two tests beside the stages.
// Not here (for now): the other stages, among them stages 9–10's Your day steps, 37–40 (Needs you now's Won't do through
// ⌘K is new, #297), prep/TC-012 step 5's people line (the header's answers are new, #286), and of Today's widgets: the
// small set's work line (today/TC-023 step 6, another data set) and "instead of" (step 3: the standard day has none).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
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
      expect(moved.json()[0]).toEqual({ row: true, size: 'M', widget: id.mine })
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
      const big = data.cli('today', 'size', id.mine, 'L')
      expect(big.code).toBe(1)
      expect(big.err.trim()).toBe(fill(w.takesNot, { id: id.mine, sizes: catalogOf(id.mine).sizes.join(', '), size: 'L' }))
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
      const h = await today.hold()
      expect(h.held).toBe(true)
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
      expect(data.cli('today', 'size', id.agenda, 'M').code).toBe(0)
      await today.reloadToday()
      expect((await today.hold()).held).toBe(false)
      expect(await today.rows([n.yourDay, ...rest])).toEqual([[n.yourDay], [n.next, n.needs], [n.choices], [n.work]])
      // (Its width isn't checked: the case says across 2 columns, the app draws a fill line alone across all 3 — asked.)
      expect(data.cli('today', 'size', id.agenda, 'S').code).toBe(0)
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
      expect(data.cli('today', 'size', id.mine, 'M').code).toBe(0)
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
      expect(data.cli('today', 'size', id.next, 'L').code).toBe(0)
      expect(data.cli('today', 'move', id.next, '--above', id.needs).code).toBe(0)
      await today.openToday()
      const card = today.widget(n.next)
      // On top, across what is beside Your day, Needs you now under it.
      expect(await today.rows([n.next, n.needs, n.choices])).toEqual([[n.next], [n.needs], [n.choices]])
      expect(Math.abs((await today.box(n.next)).width - (await today.box(n.choices)).width)).toBeLessThanOrEqual(2)
      const time = await today.clock(start)
      if (new Date(start) <= new Date()) await expect(card.getByText(fill(x.since, { time }), { exact: true })).toBeVisible()
      else await expect(card.getByText(new RegExp(fill(x.comingIn, { time: literal(time) })))).toBeVisible()
      for (const t of threads) await expect(card.getByRole('button', { name: new RegExp(`^${literal(t.name)}\\b`) })).toBeVisible()
      const coming = people.filter((p) => p.answer !== x.declined).length
      const who = [fill(coming === 1 ? x.person : x.people, { n: coming }), ...(scheduled ? [x.records] : [])].join(x.sep)
      await expect(card.getByText(`${subject} ${who}`, { exact: true }).or(card.getByText(who, { exact: true }))).toBeVisible()
      await expect(card.getByRole('heading', { name: x.walkIn, level: 3 })).toBeVisible()
      const owe = (e) => (e.kind === x.reply && e.from ? fill(x.waitsFor, e) : fill(x.youOwe, e))
      const knowing = card.getByRole('list').first().getByRole('listitem')
      await expect(knowing).toHaveText([brief.summary, ...brief.expected.slice(0, x.show).map(owe)])
      await expect(today.widgetButton(n.next, x.openPrep)).toBeVisible()
      await expect(today.widgetButton(n.next, x.ask)).toBeVisible()
      await expect(today.widgetButton(n.next, x.join)).toHaveCount(app === x.teams ? 1 : 0)
      // On the right: who's coming (each named with their answer, a hover says so), a newcomer named, what to ask.
      await expect(card.getByRole('heading', { name: x.coming, level: 3 })).toBeVisible()
      const faces = card.getByRole('list', { name: x.coming, exact: true }).getByRole('listitem')
      await expect(faces).toHaveCount(Math.min(people.length, 8))
      for (const p of people.slice(0, 8)) {
        const said = fill(x.answered, p)
        await expect(faces.filter({ has: page.getByTitle(said, { exact: true }) }).or(card.getByTitle(said, { exact: true }))).toHaveAccessibleName(said)
      }
      for (const p of people.filter((q) => q.new).slice(0, 2)) await expect(card.getByText(fill(x.newcomer, p))).toBeVisible()
      await expect(card.getByRole('heading', { name: x.askThem, level: 3 })).toBeVisible()
      await expect(card.getByRole('list').last().getByRole('listitem')).toHaveText(brief.ask.slice(0, x.askShown))
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
      expect(data.cli('today', 'size', id.next, 'M').code).toBe(0)
      await today.reloadToday()
      const plain = (e) => (e.kind === x.reply && e.from ? fill(x.waitsFor, e) : e.title)
      await expect(today.widgetSays(n.next, fill(x.expected, { list: brief.expected.map(plain).join(x.listSep) }))).toBeVisible()
      await expect(today.widgetButton(n.next, x.showPrep)).toBeVisible()
      for (const gone of [x.coming, x.askThem, x.walkIn]) await expect(card.getByRole('heading', { name: gone })).toHaveCount(0)
      await expect(today.widgetButton(n.next, x.ask)).toHaveCount(0)
      // S: the time line, the subject and its buttons only.
      expect(data.cli('today', 'size', id.next, 'S').code).toBe(0)
      await today.reloadToday()
      await expect(card.getByText(subject, { exact: true })).toBeVisible()
      await expect(today.widgetButton(n.next, x.showPrep)).toBeVisible()
      await expect(card.getByText(new RegExp(`^${literal(fill(x.expected, { list: '' }))}`))).toHaveCount(0)
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
    const at = lines.findIndex((l) => l.startsWith('Now '))
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
    await expect(today.line(s.maybe).getByText(new RegExp(`\\b${x.maybe}\\b`))).toBeVisible()
    // Meetings you accepted at once, to come: the line amber, "N others at once"; the others no lines of their own.
    await expect(today.line(s.choice).getByText(new RegExp(x.atOnce.slice(1, -1)))).toBeVisible()
    expect((await today.lineLook(s.choice)).time).toBe(await colour('choice'))
    for (const f of s.folded) await expect(today.yourDay.getByRole('listitem', { name: f, exact: true })).toHaveCount(0)
    // (Step 3, "instead of": the standard day has no such line — the rule is the node test today-agenda.test.mjs.)
    // Show N skipped: the folded meetings, in time order, dimmed, each "skipped"; a no reply dashed grey; Hide folds them.
    const shownLines = (await today.dayRows()).length
    await today.yourDay.getByRole('button', { name: new RegExp(x.showSkipped) }).click()
    await expect(today.skipped(x.hideSkipped)).toBeVisible()
    const rows = today.yourDay.getByRole('listitem')
    const all = await today.dayRows()
    const folded = all.slice(shownLines)
    expect(folded.length).toBeGreaterThan(0)
    for (const f of s.folded) expect(folded).toContain(f)
    const foldedTimes = (await today.lineTimes()).slice(times.length)
    expect(foldedTimes).toEqual([...foldedTimes].sort((a, b) => a - b))
    for (let i = 0; i < folded.length; i++) {
      const row = rows.nth((await rows.count()) - folded.length + i)
      await expect(row.getByText(new RegExp(`^${x.skipped}\\b`))).toBeVisible()
      expect(await today.opacityOf(row)).toBeLessThan(1)
      await expect(row.getByText(new RegExp(`^(${x.now}|${x.next})$`))).toHaveCount(0)
    }
    const noReply = folded.indexOf(s.noReply)
    expect(noReply).toBeGreaterThanOrEqual(0)
    const look = await today.lineLook(s.noReply, (await today.yourDay.getByRole('listitem', { name: s.noReply, exact: true }).count()) - 1)
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
    // A click on the line: the meeting's card on the right, as in Calendar; Close closes it.
    await today.line(s.choice).click({ position: { x: 200, y: 8 } })
    await expect(today.panelTab(today.rightPanel, x.meetingTab)).toHaveAttribute('aria-selected', 'true')
    const card = today.meetingCard(s.choice)
    await expect(card.getByRole('heading', { name: s.choice })).toBeVisible()
    await expect(card.getByText(new RegExp(`^${literal(x.when)}`))).toBeVisible()
    await expect(card.getByRole('combobox', { name: s.card.kind })).toBeVisible()
    for (const b of s.card.buttons) await expect(card.getByRole('button', { name: b, exact: true })).toBeVisible()
    await expect(card.getByText(new RegExp(`^${x.people}\\b`))).toBeVisible()
    await card.getByRole('button', { name: x.close, exact: true }).click()
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
      await expect(today.widget(n.strip).getByRole('heading', { name: fill(c2.stripHeading, { n: going }), level: 2 })).toBeVisible()
      await expect(today.stripBlocks()).toHaveCount(onStrip.length)
      await expect(today.widget(n.strip).getByRole('separator', { name: /^Now / })).toHaveCount(1)
      const ticks = await today.stripTicks()
      expect(ticks[0]).toBeLessThanOrEqual(c2.from)
      expect(ticks.at(-1)).toBeGreaterThanOrEqual(c2.to)
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
      await expect(today.calendarList.getByRole('listitem')).toHaveCount(api.day.filter((d) => d.start).length)
      await expect(today.block(n.yourDay, s.going)).toContainText(await today.clock(api.day.find((e) => e.subject === s.going).start))
      const [p, q] = [await today.block(n.yourDay, s.folded[0]).first().boundingBox(), await today.block(n.yourDay, s.folded[1]).first().boundingBox()]
      expect(Math.abs(p.y - q.y)).toBeLessThanOrEqual(2)
      expect(p.x === q.x).toBe(false)
      expect(await today.blockLook(n.yourDay, s.noReply)).toMatchObject({ border: 'dashed', colour: await today.themeColour(w.day.colours.noReply) })
      expect(await today.blockLook(n.yourDay, s.maybe)).toMatchObject({ border: 'dashed', colour: await today.themeColour(w.day.colours.maybe) })
      expect(await today.opacityOf(today.block(n.yourDay, s.going).first())).toBeLessThan(1)
      expect(await today.calendarNowInView()).toBe(true)
      // A block clicked: its card on the right; right-clicked (calendar or strip): Calendar's menu for it.
      await today.block(n.yourDay, s.choice).last().click({ position: { x: 6, y: 4 } })
      await expect(today.panelTab(today.rightPanel, w.day.meetingTab)).toHaveAttribute('aria-selected', 'true')
      await expect(today.meetingCard(s.choice)).toBeVisible()
      await today.meetingCard(s.choice).getByRole('button', { name: w.day.close, exact: true }).click()
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
