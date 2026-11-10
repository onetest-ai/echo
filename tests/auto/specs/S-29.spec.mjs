// S-29 Notes, transcripts, settings and Today off the happy path — its stages with fixed checks (standard data): a call
// made from a Teams transcript and its speakers, Transcript tabs without a transcript, a to-do ticked and back, a note
// edited, cancelled, emptied and deleted, notes on what was said and on a line of the minutes, a note typed but not
// added kept when another call opens or Write minutes is pressed, unreadable notes and an outside change kept beside, a
// broken config.json, a card sent to Today until Got it, a failed calendar sync on Today, and what General and Health
// say is this computer's.
// Not here: stage 10 (today/TC-005, ⌘K's actions on a busy Today) — too new to automate: its NEEDS YOU NOW group
// changed with #297 (an item to Claude) and its Close dialog with #281, both merged today.
// Each test has its own browser: a stage that continues an earlier one's view opens its call again and My notes with it.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { SettingsPage } from '../pages/settings.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'

const data = useData(test, { set: 'standard', ai: 'fake' })

const op = () => data.text.offPath
const cs = () => data.text.callStates
const set = () => data.set.s29
/** The call folders whose names end in a slug, oldest first. */
const foldersOf = (slug) => fs.readdirSync(path.join(data.home, cs().callsFolder)).filter((n) => n.endsWith(`-${slug}`)).sort()
const folderOf = (slug) => foldersOf(slug).at(-1)
/** A call's my-notes.md (empty while there is none), as its lines. */
const notesFile = (slug) => (data.exists(data.home, cs().callsFolder, folderOf(slug), data.text.calls.notes.file) ? data.read(data.home, cs().callsFolder, folderOf(slug), data.text.calls.notes.file) : '')
const lines = (slug) => notesFile(slug).split('\n')

/** The Calls list on screen; with a call open on a tab and My notes beside it. */
async function calls(page) {
  const view = new CallsPage(page, data)
  await view.open()
  await expect(view.list).toBeVisible()
  return view
}

async function withNotes(page, title, tab) {
  const view = await calls(page)
  await view.openNewest(title, tab)
  if (!(await view.newNote.isVisible())) await view.headerButton(data.text.calls.myNotes).click()
  await expect(view.newNote).toBeVisible()
  return view
}

scenario(test, 'S-29', 'Notes, transcripts, settings and Today off the happy path: imports, empty transcripts, to-dos, edits, comments, notes not lost, broken files, cards and the day\'s actions', () => {
  test('1 · A call made from a Teams transcript says so and shows its turns with the transcript\'s speaker names and "Me"', covers('window/TC-022', 'F-TR-06'), async ({ page }) => {
    const s = set().steering
    const t = cs().tabs
    const view = await calls(page)
    await view.openNewest(s.title)
    const at = data.json(data.home, cs().callsFolder, folderOf(s.slug), data.text.calls.meta).started_at
    const people = data.set.s23.steering.people
    expect(await view.factLines()).toEqual([`${await view.longDay(at)}, ${await view.clock(at)} ${fill(cs().duration, { n: data.set.s23.steering.minutes })} ${fill(data.text.calls.kindChip, { kind: data.set.s23.steering.kind })}`, cs().kindSelect, `${cs().fromTeams} ${people}`])
    await expect(view.factTitled(cs().fromTeamsHint)).toHaveText(cs().fromTeams)
    await expect(view.threadChip(data.set.s23.memo.chips[1][0])).toBeVisible()
    await view.tab(t.transcript).click()
    await expect(view.turnGroups).toHaveCount(s.turns.length)
    expect(await view.turnNames()).toEqual(s.turns.map((x) => x[0]))
    for (const [name, words] of s.turns) {
      await expect(view.turnWords(name)).toHaveText(words)
      await expect(view.within(view.turn(name), s.lang)).toBeVisible()
    }
    const segs = data.json(data.home, cs().callsFolder, folderOf(s.slug), op().segments)
    expect(segs.map((x) => (x.speaker === 'me' ? { speaker: x.speaker } : { speaker: x.speaker, name: x.name }))).toEqual(s.segments)
    expect(segs.map((x) => x.text)).toEqual(s.turns.map((x) => x[1]))
    await expect(view.callTabs).toHaveText([t.minutes, t.transcript])
  })

  test('2 · A Transcript tab without a transcript says Transcribing on this computer while it is transcribed, else No transcript', covers('window/TC-020', 'F-TR-04'), async ({ page }) => {
    const s = set()
    const view = await calls(page)
    await view.openNewest(s.quick, cs().tabs.transcript)
    await expect(view.tab(cs().tabs.transcript)).toHaveAttribute('aria-selected', 'true')
    await expect(view.says(op().transcribing)).toBeVisible()
    await expect(view.find).toHaveCount(0)
    await expect(view.liveRegion(view.main)).toHaveCount(0)
    for (const title of s.noTranscript) {
      await view.openNewest(title, cs().tabs.transcript)
      await expect(view.says(cs().noTranscriptTab)).toBeVisible()
      await expect(view.find).toHaveCount(0)
    }
    await view.openNewest(s.initech.title, cs().tabs.transcript)
    await expect(view.find).toBeVisible()
    await expect(view.turnGroups.first()).toHaveAccessibleName(s.initech.first)
    await expect(view.says(cs().noTranscriptTab)).toHaveCount(0)
  })

  test("3 · A to-do's box in My notes ticks it done and back, and my-notes.md follows", covers('window/TC-016', 'F-NOTES-03'), async ({ page }) => {
    const { acme, todos } = set()
    const [[omar, omarAt], [agenda, agendaAt]] = todos
    const view = await withNotes(page, acme.title)
    const line = (text, at, done) => `- [${done ? 'x' : ' '}] [${at}] ${text}`
    const box = async (text, done) => {
      await expect(view.noteBox(text)).toHaveAttribute('aria-checked', String(done))
      await expect(view.noteBox(text)).toHaveAccessibleName(done ? op().markToDo : op().markDone)
    }
    await expect(view.within(view.notesList, acme.heading)).toBeVisible()
    await expect(view.noteRows).toHaveCount(acme.rows)
    await box(omar, false)
    await box(agenda, true)
    const step = async (text, omarDone, agendaDone) => {
      await view.noteBox(text).click()
      await box(omar, omarDone)
      await box(agenda, agendaDone)
      await expect(view.panelSays(op().saved)).toBeVisible()
      await expect.poll(() => lines(acme.slug)).toEqual(expect.arrayContaining([line(omar, omarAt, omarDone), line(agenda, agendaAt, agendaDone)]))
    }
    await step(omar, true, true)
    await step(omar, false, true)
    await step(agenda, false, false)
    await step(agenda, false, true)
  })

  test('4 · A note edited in place changes its text and kind on Enter, Esc cancels, emptied it stays as it was, × deletes it', covers('window/TC-017', 'F-NOTES-04'), async ({ page }) => {
    const { acme, edit: e } = set()
    const keys = data.text.shell.keys
    const view = await withNotes(page, acme.title)
    await view.panelSays(e.from).click()
    await expect(view.noteEditor).toHaveValue(e.from)
    await expect(view.panelSays(op().editHint)).toBeVisible()
    await view.noteEditor.fill(e.to)
    await view.editorButton(op().question).click()
    await view.noteEditor.press(keys.enter)
    await expect(view.noteEditor).toHaveCount(0)
    expect(await view.noteTexts()).toContain(e.row)
    await expect(view.panelSays(op().saved)).toBeVisible()
    await expect.poll(() => lines(acme.slug)).toContain(e.line)
    expect(notesFile(acme.slug)).not.toContain(e.gone)
    await view.panelSays(e.cancel).click()
    await view.noteEditor.fill(e.cancelTyped)
    await view.noteEditor.press(keys.escape)
    await expect(view.noteEditor).toHaveCount(0)
    expect(await view.noteTexts()).toContain(e.cancelRow)
    expect(lines(acme.slug)).toContain(e.cancelLine)
    await view.panelSays(e.empty).click()
    await view.noteEditor.fill('')
    await view.editorButton(op().save).click()
    await expect(view.noteEditor).toHaveCount(0)
    expect(await view.noteTexts()).toContain(e.emptyRow)
    expect(lines(acme.slug)).toContain(e.emptyLine)
    const gone = view.noteRow(e.deleted)
    await gone.hover()
    await view.inside(gone, cs().deleteNote).click()
    await expect(view.noteRow(e.deleted)).toHaveCount(0)
    await expect(view.noteRows).toHaveCount(e.left)
    await expect(view.liveRows).toHaveCount(data.set.s23.live.items.length)
    await expect(view.panelSays(op().saved)).toBeVisible()
    await expect.poll(() => notesFile(acme.slug)).not.toContain(e.deleted)
    const after = await view.noteTexts()
    await page.reload()
    await view.openNewest(acme.title)
    if (!(await view.newNote.isVisible())) await view.headerButton(data.text.calls.myNotes).click()
    await expect(view.noteRows).toHaveCount(e.left)
    expect(await view.noteTexts()).toEqual(after)
  })

  test('5 · Selecting words and Comment, or right-clicking a turn or a minutes line and Add note, adds a note at that moment with the words quoted', covers('window/TC-021', 'F-TR-05'), async ({ page }) => {
    const { initech, acme, comment: c, rightClick: r, minutesLine: m } = set()
    const view = await withNotes(page, initech.title, cs().tabs.transcript)
    if (await view.foldSidebar.isVisible()) await view.foldSidebar.click()
    await expect(view.panelSays(data.text.calls.notes.empty)).toBeVisible()
    await view.turnWords(c.turn).click({ clickCount: 3 })
    await expect(view.popButtons).toHaveText([op().comment])
    await view.popButton(op().comment).click()
    await expect(view.popSays(fill(op().quote, { t: c.at, q: c.words }))).toBeVisible()
    await expect(view.popBox).toBeVisible()
    await expect(view.popSays(fill(op().goesTo, { t: c.at }))).toBeVisible()
    await view.popBox.fill(c.typed)
    await view.popButton(op().addNote).click()
    await expect.poll(() => view.noteTexts()).toContain(fill(op().quoted, { row: c.row, q: c.words }))
    await expect(view.turnTitled(c.turn, fill(op().dot, { text: c.text }))).toHaveCount(1)
    await expect.poll(() => lines(initech.slug)).toEqual(expect.arrayContaining([c.line, c.quoteLine]))
    await view.turnWords(r.turn).click({ button: 'right' })
    await expect(view.popItems).toHaveText([fill(op().addNoteAt, { t: r.at })])
    await view.popItems.first().click()
    await view.popBox.fill(r.typed)
    await view.popBox.press(data.text.shell.keys.enter)
    await expect.poll(() => view.noteTexts()).toContain(r.row)
    await expect.poll(() => lines(initech.slug)).toContain(r.line)
    await view.openNewest(acme.title, cs().tabs.minutes)
    await view.minutesLine(m.words).click({ button: 'right', position: { x: 24, y: 8 } })
    await expect(view.popItems).toHaveText([fill(op().addNoteAt, { t: m.at })])
    await view.popItems.first().click()
    await expect(view.popSays(fill(op().quote, { t: m.at, q: m.words }))).toBeVisible()
    await expect(view.popSays(fill(op().goesToOn, { t: m.at, s: m.section }))).toBeVisible()
    await view.popBox.fill(m.typed)
    await view.popButton(op().addNote).click()
    await expect.poll(() => view.noteTexts()).toContain(m.row)
    await expect.poll(() => lines(acme.slug)).toEqual(expect.arrayContaining([m.line, m.quoteLine]))
  })

  test('6 · A note typed in My notes but not added is added to its own call when another call opens or Write minutes is pressed', covers('window/TC-018', 'F-NOTES-07'), async ({ page }) => {
    const { first: [uTitle, uSlug, uText], second: [dTitle], write: [cTitle, cSlug, cText] } = set().typed
    const notes = data.text.calls.notes
    const view = await withNotes(page, uTitle)
    await view.newNote.fill(uText)
    await expect(view.panelSays(op().notSaved)).toBeVisible()
    await expect(view.panelSays(notes.empty)).toBeVisible()
    await view.openNewest(dTitle)
    await expect(view.panelSays(notes.empty)).toBeVisible()
    await expect(view.newNote).toHaveValue('')
    await expect.poll(() => notesFile(uSlug)).toBe(`- ${uText}\n`)
    await view.openNewest(uTitle)
    expect(await view.noteTexts()).toEqual([fill(op().noteRow, { text: uText })])
    await expect(view.panelSays(op().saved)).toBeVisible()
    await view.openNewest(cTitle, cs().tabs.minutes)
    await view.newNote.fill(cText)
    await view.headerButton(cs().writeMinutes).click()
    await expect.poll(() => view.noteTexts()).toEqual([fill(op().noteRow, { text: cText })])
    await expect(view.newNote).toHaveValue('')
    await expect(view.paneHeading(cs().couldnt)).toBeVisible({ timeout: 15_000 })
    await expect(view.says(data.text.fake.claudeFails)).toBeVisible()
    expect(notesFile(cSlug)).toBe(`- ${cText}\n`)
  })

  test('7 · An unreadable my-notes.md says so with Show folder; one changed outside while a note is typed is kept beside', covers('window/TC-019', 'F-NOTES-09'), async ({ page }) => {
    const { unreadable: u, outside: o } = set()
    const view = new CallsPage(page, data)
    await view.goto(fill(data.text.calls.route, { name: foldersOf(u.slug)[0], tab: data.text.calls.tabs.minutes }))
    if (!(await view.rightPanel.isVisible())) await view.headerButton(data.text.calls.myNotes).click()
    await expect(view.panelSays(fill(op().unreadable, { error: u.error }))).toBeVisible()
    await expect(view.panelButton(op().showFolder)).toBeVisible()
    await expect(view.newNote).toHaveCount(0)
    await view.openNewest(o.title)
    expect(await view.noteTexts()).toEqual([o.row])
    await expect(view.panelSays(op().saved)).toBeVisible()
    await view.newNote.fill(o.typed)
    await expect(view.panelSays(op().notSaved)).toBeVisible()
    const dir = path.join(data.home, cs().callsFolder, folderOf(o.slug))
    // The window reads the call again once its file changed; while a note is typed nothing is drawn over.
    const reread = view.callReread(folderOf(o.slug))
    fs.writeFileSync(path.join(dir, data.text.calls.notes.file), o.other)
    await reread
    expect(await view.noteTexts()).toEqual([o.row])
    await expect(view.newNote).toHaveValue(o.typed)
    await view.newNote.press(data.text.shell.keys.enter)
    const kept = view.panelSaysLike(new RegExp(op().kept))
    await expect(kept).toBeVisible()
    const copy = new RegExp(op().kept).exec(await kept.textContent())[1]
    expect(await view.noteTexts()).toEqual([o.row, fill(op().noteRow, { text: o.typed })])
    await expect(view.panelSays(op().saved)).toBeVisible()
    expect(notesFile(o.slug).split('\n')).toEqual(expect.arrayContaining([o.line, o.typedLine]))
    expect(data.read(dir, copy)).toBe(o.other)
    await view.panelButton(op().noticeOk).click()
    await expect(kept).toHaveCount(0)
    expect(await view.noteTexts()).toEqual([o.row, fill(op().noteRow, { text: o.typed })])
  })

  test('8 · An unreadable config.json says so in every Settings section instead of its settings; the rest of the window works', covers('settings/TC-025', 'F-SET-18'), async ({ page }) => {
    const c = op().config
    const file = path.join(data.home, c.file)
    const good = fs.readFileSync(file)
    const settings = new SettingsPage(page, data)
    const says = fill(c.page, { error: c.error })
    try {
      fs.writeFileSync(file, c.broken)
      const r = data.cli('config')
      expect(r.code).toBe(1)
      expect(r.err).toContain(fill(c.cli, { error: c.error }))
      await settings.open(data.text.settings.general.route)
      await expect(settings.sections).toBeVisible()
      await expect(settings.says(says)).toBeVisible()
      await expect(settings.group(c.appearance)).toHaveCount(0)
      await expect(settings.switches).toHaveCount(0)
      for (const name of set().sections) {
        await settings.section(name).click()
        await expect(settings.says(says)).toBeVisible()
        await expect(settings.switches).toHaveCount(0)
      }
      const view = new CallsPage(page, data)
      await view.activity(data.text.shell.activities[1]).click()
      await expect(view.list).toBeVisible()
      await expect(view.headingNamed(data.text.calls.pick)).toBeVisible()
      await expect(view.says(says)).toHaveCount(0)
      await view.callRows.first().click()
      await expect(view.heading(data.set.s23.standup.title)).toBeVisible()
      await expect(view.tab(cs().tabs.minutes)).toHaveAttribute('aria-selected', 'true')
      // Its automatic minutes need the settings: the box says why they can't start, and offers Write minutes.
      await expect(view.paneHeading(c.autoCant)).toBeVisible()
      await expect(view.says(fill(c.autoWhy, { error: c.error }))).toBeVisible()
      await expect(view.headerButton(cs().writeMinutes)).toBeVisible()
      await expect(view.says(says)).toHaveCount(0)
    } finally {
      fs.writeFileSync(file, good)
    }
    await settings.open(data.text.settings.general.route)
    await expect(settings.group(c.appearance)).toBeVisible()
    expect(data.cli('config').code).toBe(0)
  })

  test('9 · A card sent to Today shows until Got it, and comes back when its source sends again', covers('today/TC-004', 'F-TODAY-05'), async ({ page }) => {
    const card = set().card
    const k = op().card
    const status = data.cli('deliver', 'status')
    expect(status.code).toBe(0)
    expect(status.json().today).toBe(true)
    const send = (text, ...ai) => data.cliIn(text, 'deliver', 'send', 'today', '--title', card.title, '--about', card.about, ...ai, '--file', '/dev/stdin')
    const sent = send(card.first, '--ai', card.ai)
    expect(sent.code).toBe(0)
    expect(sent.out.trim()).toBe(k.sent)
    const sentAt = new Date().toISOString()
    const today = new TodayPage(page, data)
    await today.goto(data.text.shell.routes.today)
    await expect(today.yourDay).toBeVisible()
    await expect(today.card(card.title)).toBeVisible()
    await expect(today.cardSays(card.title, fill(k.about, { about: card.about, time: await today.clock(sentAt) }))).toBeVisible()
    await expect(today.cardHeading(card.title, card.heading)).toBeVisible()
    await expect(today.cardItems(card.title)).toHaveText(card.items)
    await expect(today.cardBold(card.title, card.bold)).toHaveCount(1)
    await expect(today.cardSays(card.title, k.ai)).toBeVisible()
    // Beside Your day: in the column right of it (Today as it comes, F-TODAY-21).
    const [c, y] = [await today.card(card.title).boundingBox(), await today.yourDay.boundingBox()]
    expect(c.x).toBeGreaterThanOrEqual(y.x + y.width)
    await today.cardButton(card.title, k.gotIt).click()
    await expect(today.card(card.title)).toHaveCount(0)
    await page.reload()
    await expect(today.yourDay).toBeVisible()
    await expect(today.card(card.title)).toHaveCount(0)
    const cards = Object.values(data.json(data.home, k.file)).flat().filter((c) => c && c.title === card.title)
    expect(cards.map((c) => c.seen)).toEqual([true])
    expect(send(card.second).code).toBe(0)
    await page.reload()
    await expect(today.card(card.title)).toHaveCount(1)
    await expect(today.cardItems(card.title)).toHaveText([card.secondItem])
    await expect(today.cardSays(card.title, k.ai)).toHaveCount(0)
    await today.cardButton(card.title, k.gotIt).click()
    await expect(today.card(card.title)).toHaveCount(0)
  })

  test("11 · After last night's empty agenda, Your day says the calendar couldn't be read only once today's sync has failed", covers('today/TC-012', 'F-TODAY-09'), async ({ page }) => {
    const c = op().calendar
    const agenda = path.join(data.home, c.agenda)
    const config = path.join(data.home, c.config)
    const [goodAgenda, goodConfig] = [fs.readFileSync(agenda), fs.readFileSync(config)]
    const today = new TodayPage(page, data)
    await today.goto(data.text.shell.routes.today)
    await expect(today.yourDay).toBeVisible()
    const built = await today.dayRows()
    try {
      const cleared = data.cli('agenda', 'clear')
      expect(cleared.code).toBe(0)
      expect(cleared.out + cleared.err).toContain(c.cleared)
      const yesterday = new Date(Date.now() - 36 * 3600_000)
      fs.utimesSync(agenda, yesterday, yesterday)
      fs.writeFileSync(config, JSON.stringify({ ...JSON.parse(goodConfig), calendar_command: c.command }, null, 2))
      await page.reload()
      await expect(today.yourDay).toBeVisible()
      await expect(today.says(c.notice)).toHaveCount(0)
      await expect(today.dayButtons(c.openCall).first()).toBeVisible()
      await expect(today.dayButtons(c.record)).toHaveCount(0)
      const rows = await today.dayRows()
      const sync = data.cli('agenda', 'sync')
      const out = JSON.parse(sync.out.trim().split('\n').at(-1))
      expect(out.synced).toBe(false)
      expect(out.reason.startsWith(c.reason)).toBe(true)
      await page.reload()
      await expect(today.says(c.notice)).toBeVisible()
      expect(await today.dayRows()).toEqual(rows)
    } finally {
      fs.writeFileSync(config, goodConfig)
      fs.writeFileSync(agenda, goodAgenda)
      fs.rmSync(path.join(data.home, c.failed), { force: true })
    }
    await page.reload()
    await expect(today.yourDay).toBeVisible()
    await expect(today.says(c.notice)).toHaveCount(0)
    expect(await today.dayRows()).toEqual(built)
  })

  test("12 · General and Health say what is this computer's; Claude Code is the run's, with no home path on the page", covers('settings/TC-028', 'F-SET-19'), async ({ page }) => {
    const settings = new SettingsPage(page, data)
    const fake = path.join(path.dirname(data.home), 'bin', 'claude')
    const home = process.env.HOME
    const shown = fake.startsWith(home + path.sep) ? '~' + fake.slice(home.length) : fake
    await settings.open(data.text.settings.general.route)
    await expect(settings.says(op().general)).toBeVisible()
    await expect(settings.rowHas(settings.settingRow(op().dataFolder), data.home)).toBeVisible()
    await settings.open(op().healthRoute)
    await expect(settings.says(op().health)).toBeVisible()
    const row = settings.settingRow(op().claudeCode)
    await expect(row).toBeVisible({ timeout: 20_000 })
    await expect(settings.saysPart(op().checking)).toHaveCount(0, { timeout: 20_000 })
    await expect(row).toContainText(`${op().mark} ${op().claudeCode}`)
    await expect(row).toContainText(fill(op().claudeRow, { version: set().claude.version, path: shown }))
    await expect(settings.settingRow(op().plugin)).toContainText(`${op().mark} ${op().plugin}`)
    await expect(settings.settingRow(op().plugin)).toContainText(op().pluginLine)
    await expect(settings.settingRow(op().older)).toContainText(`${op().badMark} ${op().older}`)
    await expect(settings.settingRow(op().older)).toContainText(op().olderVersion)
    expect(await settings.pageText()).not.toContain(home + '/')
  })
})
