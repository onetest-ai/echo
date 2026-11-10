// S-24 The calendar, preps and the Inbox on a busy day — its stages with fixed checks (standard data): a series' rule on
// its meetings' cards (stage 5), the cards' preps (8), Zoom and Google Meet invites shown and never armed (9–10), the
// prep's header (13–14), a series set to Record and kept (17), Import Teams transcript without Microsoft 365 (29), Won't
// do's comment (36), and two tests beside the stages.
// Not here (for now): the other stages, among them Today's (stages 9–10's Your day steps, 37–45: Needs you now's Won't
// do through ⌘K is new (#297), and Today's widgets are being built, #336) and prep/TC-012 step 5's people line (the
// header's answers are new, #286).
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { MeetingPage } from '../pages/meeting.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { CalendarPage } from '../pages/calendar.page.mjs'

const data = useData(test, { set: 'standard' })
const set = () => data.set.s24
const c = () => data.text.cal
/** `echoctl series list`, read as JSON. */
const series = () => data.cli('series', 'list').json()
/** A short wait that checks a click changed nothing (no request lands later), not a wait for a condition. */
const seconds = (n) => new Promise((resolve) => setTimeout(resolve, n * 1000))
/** What the data's `open` was asked to show so far (nothing before its first call). */
const opened = () => (data.exists(data.dir, data.text.recording.openLog) ? data.read(data.dir, data.text.recording.openLog) : '')

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
