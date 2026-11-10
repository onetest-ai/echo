// S-27 Others' words everywhere: nothing runs, nothing breaks — its fixed checks (hostile data): every place where
// other people's words reach the window, each piece shown as text, and on every view the cases' check A (no inline
// handler, no inline script, no javascript: address, no broken image made from the data, the page no wider than the
// window); each stage ends with check D (no dialog, no page error) and with no console error but the favicon's 404 and
// no request off Echo's own server. Judgement (how it looks) stays with agent runs.
// Not here: the prelude (it arms the agent's guard and checks it survives a reload: Playwright's own events are armed
// for every test's page), and stage 17 (today/TC-011, Needs you now's icon actions) — too new to automate: its ✦ and
// Claude's dialog on an item came with #297, merged today.
// Each test has its own browser and guard: the scenario's groups of console reads are each stage's own here.
import fs from 'node:fs'
import path from 'node:path'
import { test, expect } from '@playwright/test'
import { useData, fill, literal } from '../support/data.mjs'
import { covers, scenario } from '../support/covers.mjs'
import { HostilePage } from '../pages/hostile.page.mjs'
import { CallsPage } from '../pages/calls.page.mjs'
import { TodayPage } from '../pages/today.page.mjs'
import { TasksPage } from '../pages/tasks.page.mjs'
import { VocabularyPage } from '../pages/vocabulary.page.mjs'

const data = useData(test, { set: 'hostile', ai: 'fake' })

const H = () => data.set
const v = () => data.text.hostile
const route = (name, vars = {}) => fill(v().routes[name], vars)
const thread = (tab) => route('thread', { id: H().thread.id, tab })
const client = (id, tab) => route('client', { id, tab })

/** Echo's window on a route, the guard armed before it loads. */
async function open(page, hash) {
  const h = new HostilePage(page, data)
  await h.goto(hash)
  return h
}

/** Check A on the view shown. */
async function clean(h) {
  expect(await h.markup()).toEqual({ handlers: 0, inlineScripts: 0, badUrls: 0, brokenImgs: 0, fits: true })
}

/** Check D and the rest of the guard: nothing ran, nothing was logged, nothing left Echo's server. */
function still(h) {
  expect(h.quiet()).toEqual({ dialogs: [], pageErrors: [], console: [], outside: [] })
}

/** The hostile call's folder (its name holds the markup). */
const callFolder = () => fs.readdirSync(path.join(data.home, 'calls')).find((n) => n.includes(`${H().marker}${H().call.marker}`))

/** The hostile call open on a tab (yesterday's, its folder named with the markup). */
async function hostileCall(page, tab) {
  const h = await open(page, route('calls'))
  await h.row(h.piece(H().call.marker)).click()
  await expect.poll(() => decodeURIComponent(h.hash())).toContain(h.mark(H().call.marker))
  if (tab) await h.tabStarting(tab).click()
  return h
}

scenario(test, 'S-27', "Others' words everywhere: nothing runs, nothing breaks", () => {
  test('1 · Meeting subjects, organizers, people and the prep show as text in the calendar, Coming up today and Today', covers('hostile/TC-002', 'F-CAL-04', 'F-CAL-07', 'F-PREP-01', 'F-PREP-02', 'F-PREP-04', 'F-TODAY-02'), async ({ page }) => {
    const c = H().calendar
    const h = await open(page, route('calendarDay'))
    await expect(h.shows(c.meeting)).toBeVisible()
    await expect(h.shows(c.past)).toBeVisible()
    await clean(h)
    await h.buttonHolding(h.piece(c.meeting)).click()
    await expect(h.heading(`${H().calendar.base} ${h.piece(c.meeting, 0)}`)).toBeVisible()
    await expect(h.shows(c.organizer)).toBeVisible()
    await clean(h)
    await h.button(v().week).click()
    await expect(h.shows(c.meeting)).toBeVisible()
    await expect(h.shows(c.past)).toBeVisible()
    await expect(h.heading(`${H().calendar.base} ${h.piece(c.meeting, 0)}`)).toBeVisible()
    await clean(h)
    const calls = new CallsPage(page, data)
    await calls.goto(route('calls'))
    await calls.comingUp.click()
    await expect(h.row(h.piece(c.meeting))).toBeVisible()
    await clean(h)
    // Steps 11–13 (the meeting's own header: its people, the 10 000-character name in its column) are left out: that
    // header's invitee chips came with #286, today — too new to automate.
    await h.goto(route('calendarDay'))
    await h.buttonHolding(c.prepMeeting).click()
    await h.button(v().showPrep).click()
    await expect(h.heading(v().inShort)).toBeVisible()
    await expect(h.shows(c.prep.inShort)).toBeVisible()
    await expect(h.heading(v().newSince)).toBeVisible()
    for (const id of [c.prep.new, c.prep.from, c.prep.snippet]) await expect(h.shows(id)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    await clean(h)
    const today = new TodayPage(page, data)
    await today.goto(route('today'))
    await expect(today.dayRow(h.piece(c.past, 0))).toBeVisible()
    await expect(today.dayRow(`${H().calendar.base} ${h.piece(c.meeting, 0)}`)).toBeVisible()
    expect(await h.linksTo(H().evil, today.yourDay)).toBe(0)
    await clean(h)
    still(h)
  })

  test('2 · Inbox items, Found for you and Needs you now show others\' words as text', covers('hostile/TC-011', 'F-INBOX-01', 'F-INBOX-03', 'F-INBOX-04', 'F-INBOX-05', 'F-THR-18', 'F-TODAY-04'), async ({ page }) => {
    const h = await open(page, route('today'))
    const today = new TodayPage(page, data)
    await expect(today.yourDay).toBeVisible()
    for (const id of H().needs) await expect(h.shows(id, today.needs)).toBeVisible()
    await expect(h.showsText(H().markup, today.needs)).toBeVisible()
    expect(await h.inside(h.piece(H().needs[3], 20), h.main)).toBe(true)
    await clean(h)
    await h.inbox.click()
    await expect(h.rightPanel).toBeVisible()
    // Items past the first five of a kind wait behind "N more like these".
    for (const more of await h.buttonHolding(v().moreLike, h.rightPanel).all()) await more.click()
    for (const id of H().inbox) await expect(h.shows(id, h.rightPanel)).toBeVisible()
    await expect(h.showsText(H().markup, h.rightPanel)).toBeVisible()
    expect(await h.fitsBox(h.rightPanel)).toBe(true)
    await clean(h)
    await h.closePanel(h.rightPanel).click()
    await h.goto(route('projects'))
    await h.buttonHolding(v().found).click()
    await expect(h.shows(H().found)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    await clean(h)
    still(h)
  })

  test('3 · Thread and client names, briefs, About, notes, rules, sources and aliases show as text', covers('hostile/TC-010', 'F-THR-01', 'F-THR-08', 'F-THR-10', 'F-THR-14', 'F-THR-17', 'F-CLIENT-02', 'F-CLIENT-05'), async ({ page }) => {
    const t = H().thread
    const h = await open(page, route('projects'))
    const row = h.buttonHolding(h.piece(t.name))
    await expect(row).toBeVisible()
    await clean(h)
    await row.click()
    await h.tab(v().tabNames.brief).click()
    await expect(h.headingStarting(`${t.base} ${h.piece(t.name)}`)).toBeVisible()
    const brief = h.region(v().brief)
    await expect(h.shows(t.brief[0], brief)).toBeVisible()
    await expect(h.showsText(H().markup, brief)).toBeVisible()
    await expect(h.showsText(h.mark(t.brief[2]), brief)).toBeVisible()
    await expect(h.showsText(H().mdWords, brief)).toBeVisible()
    expect(await h.made(brief, v().madeTags)).toBe(0)
    expect((await h.links(brief)).filter((l) => !/^https:\/\//.test(l))).toEqual([])
    expect(await h.fitsBox(brief)).toBe(true)
    expect(await h.inside(h.piece(t.brief[1], 20), brief)).toBe(true)
    await clean(h)
    await h.tab(v().tabNames.profile).click()
    for (const id of [t.rule, t.source, t.notes]) await expect(h.shows(id)).toBeVisible()
    await expect(h.showsText(H().script)).toBeVisible()
    await clean(h)
    await h.button(v().threadActions).click()
    await expect(h.menuItemEnding(t.client)).toBeVisible()
    await page.keyboard.press(data.text.shell.keys.escape)
    await h.goto(client(H().clients.globex, v().tabs.overview))
    const about = h.region(fill(v().about, { name: H().clients.globexName }))
    for (const id of H().clients.about) await expect(h.shows(id, about)).toBeVisible()
    await expect(h.showsText(H().markup, about)).toBeVisible()
    expect(await h.made(about, v().imgTags)).toBe(0)
    expect(await h.headingNames(about)).toEqual([fill(v().about, { name: H().clients.globexName })])
    await clean(h)
    await h.goto(client(H().clients.acme, v().tabs.profile))
    // `client` takes other names as a comma-separated list: the chip reads the alias up to its comma.
    const alias = h.piece(H().clients.alias)
    await expect(h.showsText(alias.slice(0, alias.indexOf(',')))).toBeVisible()
    await clean(h)
    still(h)
  })

  test('4 · People and organisations show as text on the map, org chart, list and person panel', covers('hostile/TC-009', 'F-PPL-01', 'F-PPL-04', 'F-PPL-05', 'F-CLIENT-08'), async ({ page }) => {
    const p = H().people
    const h = await open(page, route('thread', { id: p.thread, tab: v().tabs.people }))
    await h.viewButton(v().viewGroup, v().list).click()
    const row = h.rowHolding(h.piece(p.person))
    await expect(row).toBeVisible()
    for (const id of [p.person, p.title, p.group]) await expect(h.shows(id, row)).toBeVisible()
    await expect(h.showsText(H().markup, row)).toBeVisible()
    expect(await h.inside(h.piece(p.office, 20))).toBe(true)
    await clean(h)
    await row.click()
    for (const id of [p.person, p.about, p.group, p.office, p.fact, p.quote]) await expect(h.shows(id, h.rightPanel)).toBeVisible()
    await expect(h.showsText(H().script, h.rightPanel)).toBeVisible()
    expect((await h.links(h.body)).filter((l) => /^\s*javascript:/i.test(l))).toEqual([])
    expect(await h.made(h.main, v().loadedImgs)).toBe(0)
    await clean(h)
    await h.viewButton(v().viewGroup, v().map).click()
    await clean(h)
    await h.viewButton(v().viewGroup, v().org).click()
    await clean(h)
    await h.goto(thread(v().tabs.people))
    await h.viewButton(v().viewGroup, v().list).click()
    const member = h.rowHolding(`${H().thread.peopleBase} ${h.person(H().thread.people)}`)
    await expect(member).toBeVisible()
    await expect(h.shows(H().thread.role, member)).toBeVisible()
    expect(await h.direction(h.shows(H().thread.role, member))).toBe(H().ltr)
    await clean(h)
    await h.goto(client(H().clients.umbrella, v().tabs.people))
    for (const id of H().clients.umbrellaPeople) await expect(h.showsPerson(id)).toBeVisible()
    for (const id of H().clients.umbrellaLong) {
      await expect(h.showsPerson(id)).toBeVisible()
      expect(await h.inside(h.piece(id, 20))).toBe(true)
    }
    await clean(h)
    await h.goto(route('thread', { id: p.thread, tab: v().tabs.people }))
    await h.viewButton(v().viewGroup, v().list).click()
    await h.rowHolding(h.piece(p.person)).click()
    await expect.poll(async () => (await h.optionTexts(v().organisation, h.rightPanel)).some((o) => o.startsWith(`${p.orgBase} ${h.piece(p.org)}`.slice(0, 30)))).toBe(true)
    await clean(h)
    still(h)
  })

  test('5 · Work items show as text on the tree, the board, Tasks and in Details', covers('hostile/TC-008', 'F-WORK-04', 'F-WORK-07', 'F-WORK-09', 'F-WORK-14', 'F-TASK-06', 'F-TASK-07'), async ({ page }) => {
    const w = H().work
    const h = await open(page, route('thread', { id: w.thread, tab: v().tabs.work }))
    await h.select(v().layoutGroup).selectOption({ label: v().tree })
    const task = h.buttonWithText(`${w.task[0]}`)
    await expect(h.shows(w.task[1], task)).toBeVisible()
    await expect(h.showsText(H().markup, task)).toBeVisible()
    const long = h.buttonWithText(`${w.long[0]}`)
    // The case's checks: the row holds the title's words and stays inside the tree (its title may be cut to nothing).
    await expect(long).toContainText(h.mark(w.long[1]))
    expect(await h.within(long)).toBe(true)
    await expect(h.shows(w.question[1], h.buttonWithText(`${w.question[0]}`))).toBeVisible()
    await clean(h)
    await h.select(v().layoutGroup).selectOption({ label: v().board })
    const card = h.itemHolding(h.piece(w.task[1]))
    await expect(card).toBeVisible()
    await expect(h.itemHolding(h.piece(w.long[1], 20))).toBeVisible()
    await clean(h)
    await card.click()
    const d = h.rightPanel
    await expect(h.headingStarting(`${w.taskBase} ${h.piece(w.task[1])}`, d)).toBeVisible()
    for (const id of [...w.desc, w.waitWho]) await expect(h.shows(id, d)).toBeVisible()
    const links = await h.links(d)
    expect(links).toContain(w.linkUrl)
    expect(links.filter((l) => !/^(https|echo):\/\//i.test(l))).toEqual([])
    // The description's Markdown heading stays text; nothing of it is a script or a frame.
    expect((await h.headingNames(d)).filter((n) => w.desc.some((id) => n.includes(h.mark(id))))).toEqual([])
    expect(await h.made(d, v().scriptTags)).toBe(0)
    await expect(h.showsText(H().script, d)).toBeVisible()
    expect(await h.fitsBox(d)).toBe(true)
    await clean(h)
    await h.itemHolding(h.piece(w.long[1], 20)).click()
    await expect(h.shows(w.long[1], d)).toBeVisible()
    await clean(h)
    await h.closePanel(d).click()
    await h.goto(route('tasks'))
    const tasks = new TasksPage(page, data)
    await expect(h.buttonsLike(new RegExp(v().more)).first()).toBeVisible()
    // Each fold opened in turn (an opened one reads Show fewer).
    while (await h.buttonsLike(new RegExp(v().more)).count()) await h.buttonsLike(new RegExp(v().more)).first().click()
    await expect(h.shows(w.task[1])).toBeVisible()
    if (await tasks.others().isVisible()) await tasks.others().click()
    await expect(h.shows(w.owner)).toBeVisible()
    await clean(h)
    still(h)
  })

  test('6 · Plugin names, summaries, categories and settings show as text in Settings → Plugins', covers('hostile/TC-012', 'F-PLUG-01', 'F-PLUG-02', 'F-PLUG-05', 'F-PLUG-07'), async ({ page }) => {
    const g = H().plugins
    const h = await open(page, route('plugins'))
    await h.tabStarting(v().installed).click()
    await expect(h.shows(g.name)).toBeVisible()
    await expect(h.shows(g.summary)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    await clean(h)
    await h.goto(route('plugin', { id: g.id }))
    await expect(h.shows(g.label)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    expect(await h.values()).toContain(h.piece(g.value, 0))
    await clean(h)
    await h.goto(route('plugins'))
    await h.tabStarting(v().discover).click()
    await expect(h.shows(g.summary)).toBeVisible()
    const chip = h.buttonHolding(h.mark(g.category))
    await expect(chip).toBeVisible()
    await chip.click()
    await expect(h.shows(g.name)).toBeVisible()
    await clean(h)
    await h.tabStarting(v().marketplaces).click()
    for (const m of g.markets) await expect(h.showsText(m)).toBeVisible()
    await expect(h.showsText(g.marketError)).toBeVisible()
    await clean(h)
    still(h)
  })

  test('7 · Skill, template and meeting type words show as text in Settings → Templates', covers('hostile/TC-013', 'F-TPL-01', 'F-TPL-02', 'F-TPL-03', 'F-TPL-05', 'F-SKILL-01', 'F-SKILL-02'), async ({ page }) => {
    const t = H().templates
    const h = await open(page, route('templates'))
    await h.tabStarting(v().skills).click()
    await expect(h.buttonHolding(t.skill)).toBeVisible()
    await expect(h.shows(t.skillDesc)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    await clean(h)
    await h.buttonHolding(t.skill).click()
    for (const id of t.skillMd) await expect(h.showsText(h.mark(id))).toBeVisible()
    await expect(h.showsText(H().script)).toBeVisible()
    await clean(h)
    await h.tabStarting(v().minutes).click()
    await h.buttonHolding(t.template).click()
    const text = await h.values()
    for (const id of [t.header, t.section]) expect(text).toContain(h.mark(id))
    expect(text).toContain(H().markup)
    await clean(h)
    await h.tabStarting(v().types).click()
    await expect(h.shows(t.type)).toBeVisible()
    await expect(h.shows(t.typeAbout)).toBeVisible()
    await clean(h)
    const calls = new CallsPage(page, data)
    await calls.goto(route('calls'))
    await calls.newest(t.call).click()
    await expect.poll(async () => (await h.optionTexts(v().kinds)).some((o) => o.startsWith(h.piece(t.type, 20)))).toBe(true)
    await clean(h)
    still(h)
  })

  test('8 · Vocabulary terms and the meetings and people of learned terms show as text', covers('hostile/TC-014', 'F-VOC-01', 'F-VOC-02', 'F-VOC-03'), async ({ page }) => {
    const voc = H().vocabulary
    const h = await open(page, route('layout'))
    await h.button(v().reset).click()
    // Calls from the activity bar (no reload: the layout Reset is saved as it is).
    await h.activity(data.text.shell.activities[1]).click()
    const vocabulary = new VocabularyPage(page, data)
    await vocabulary.book.click()
    const p = vocabulary.panel
    await expect(vocabulary.tab).toHaveAttribute('aria-selected', 'true')
    await expect(h.shows(voc.every, p)).toBeVisible()
    await expect(h.showsText(H().cyrillic, p)).toBeVisible()
    await expect(h.shows(voc.learned, p)).toBeVisible()
    await expect(h.showsText(H().markup, p)).toBeVisible()
    await expect(h.showsText(h.mark(voc.subject), p)).toBeVisible()
    expect(await h.fitsBox(p)).toBe(true)
    expect(await h.direction(p)).toBe(H().ltr)
    await clean(h)
    await h.goto(route('transcription'))
    await clean(h)
    await h.goto(route('recording'))
    await clean(h)
    still(h)
  })

  test('9 · Call titles and folder names show as text in the list, header, palette and thread and client call lists', covers('hostile/TC-001', 'F-CALLS-01', 'F-CALLS-03', 'F-CALLS-05', 'F-THR-11', 'F-CLIENT-03', 'F-WS-03'), async ({ page }) => {
    const c = H().call
    const h = await open(page, route('calls'))
    const row = h.row(h.piece(c.marker))
    await expect(row).toBeVisible()
    await expect(h.showsText(H().markup, row)).toBeVisible()
    await clean(h)
    const long = h.rowWithText(h.mark(H().longCall))
    await expect(long).toContainText(h.piece(H().longCall, 20))
    expect(await h.within(long, h.callsList)).toBe(true)
    const rtl = h.rowWithText(h.mark(H().rtlCall))
    expect(await h.direction(rtl)).toBe(H().ltr)
    expect(await h.fitsBox(rtl)).toBe(true)
    const calls = new CallsPage(page, data)
    await calls.search.fill(h.mark(H().found))
    await expect(h.callRows).toHaveCount(c.found)
    for (const r of await h.callRows.all()) await expect(r).toContainText(`${H().foundBase} ${h.piece(H().found, 0).slice(0, 40)}`)
    await calls.search.fill('')
    await row.click()
    const day = new Date(Date.now() - c.day * 86_400_000)
    const ymd = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, '0')}-${String(day.getDate()).padStart(2, '0')}`
    await expect.poll(() => decodeURIComponent(h.hash())).toMatch(new RegExp(fill(v().callRoute, { day: ymd })))
    expect(decodeURIComponent(h.hash())).toContain(h.mark(c.marker))
    await expect(h.headingStarting(`${c.title} ${h.piece(c.marker)}`)).toBeVisible()
    await clean(h)
    await page.keyboard.press(v().palette)
    await h.paletteBox.fill(h.mark(c.marker))
    await expect(h.paletteOption(H().markup, v().callsGroup)).toBeVisible()
    await clean(h)
    await page.keyboard.press(data.text.shell.keys.escape)
    await expect(h.palette).toHaveCount(0)
    await h.goto(thread(v().tabs.calls))
    for (const id of H().thread.calls) await expect(h.showsText(h.mark(id))).toBeVisible()
    await clean(h)
    await h.goto(client(H().clients.umbrella, v().tabs.calls))
    await expect(h.shows(c.marker)).toBeVisible()
    await clean(h)
    still(h)
  })

  test('10 · The minutes frame shows hostile minutes as text, with only https links and no script of the data', covers('hostile/TC-005', 'F-MIN-01', 'F-MIN-03', 'F-NOTES-08'), async ({ page }) => {
    const m = H().minutes
    const h = await hostileCall(page)
    await expect(h.frameMain).toBeVisible()
    const h1 = await h.frameH1()
    expect(h1.startsWith(h.mark(m.heading))).toBe(true)
    expect(h1).toContain(H().markup)
    await expect(h.frameSays(h.mark(m.title))).toContainText(H().script)
    const made = await h.minutesMarkup(h.frameBody)
    expect(made).toMatchObject({ handlers: 0, imgs: 0, dataScripts: 0, badLinks: 0, frames: 0, fits: true })
    for (const l of made.links) expect(l).toEqual({ href: expect.stringMatching(new RegExp(`^${literal(H().evil)}`)), noopener: true })
    const text = await h.frameText()
    for (const x of m.asText) expect(text).toContain(x)
    for (const x of m.plain) expect(text).toContain(x)
    const cells = (await h.frameCells()).join(' ')
    for (const id of m.cells) expect(cells).toContain(h.mark(id))
    expect(await h.frameFigure()).toContain(h.mark(m.diagram))
    expect(await h.within(h.frameItem(h.mark(m.long)), h.frameMain)).toBe(true)
    await h.frameChip(m.chip, h.mark(m.summary)).click()
    await expect.poll(() => h.hash()).toMatch(new RegExp(`${literal(v().transcriptEnd)}$`))
    await expect.poll(() => h.highlighted()).toHaveLength(1)
    await h.tab(v().minutes).click()
    await expect(h.frameMain).toBeVisible()
    await clean(h)
    still(h)
  })

  test('11 · My notes, the raw my-notes.md and the notes under the minutes lines show as text', covers('hostile/TC-007', 'F-NOTES-01', 'F-NOTES-05', 'F-NOTES-08'), async ({ page }) => {
    const n = H().notes
    const h = await hostileCall(page)
    await h.button(v().notesButton).click()
    await expect(h.button(v().notesButton)).toHaveAttribute('aria-pressed', 'true')
    for (const id of n.rows) await expect(h.showsText(h.mark(id), h.rightPanel)).toBeVisible()
    expect(await h.direction(h.rightPanel)).toBe(H().ltr)
    expect(await h.fitsBox(h.rightPanel)).toBe(true)
    await clean(h)
    const before = data.files()
    await h.button(v().editAsText, h.rightPanel).click()
    const raw = h.rawNotes
    expect((await raw.inputValue()).startsWith(`# ${h.piece(n.heading, 0).slice(0, 40)}`)).toBe(true)
    expect(await raw.inputValue()).toContain(H().script)
    await h.button(v().timeline, h.rightPanel).click()
    await clean(h)
    await h.closePanel(h.rightPanel).click()
    const callout = h.frameItem(h.mark(n.callout))
    await expect(callout).toContainText(H().mdLink)
    expect(await h.made(h.frameBody, v().noteMade)).toBe(0)
    expect(Object.keys(before).filter((f) => f.endsWith('my-notes.md')).every((f) => data.files()[f] === before[f])).toBe(true)
    await clean(h)
    still(h)
  })

  test('12 · Transcript turns and speaker names show as text and keep their rows', covers('hostile/TC-003', 'F-TR-01', 'F-TR-02', 'F-TR-03'), async ({ page }) => {
    const t = H().transcript
    const h = await hostileCall(page, v().transcript)
    await expect.poll(() => h.hash()).toMatch(new RegExp(`${literal(v().transcriptEnd)}$`))
    await expect(h.turns).toHaveCount(t.texts.length)
    const turns = await h.turns.all()
    for (const [i, id] of t.speakers.entries()) await expect(turns[i]).toContainText(h.mark(id))
    for (const [i, id] of t.texts.entries()) await expect(turns[i]).toContainText(h.mark(id))
    await expect(turns[0]).toContainText(H().markup)
    await expect(turns[0]).toContainText(H().kinds.html.split(' ').slice(-1)[0])
    expect((await h.links()).filter((l) => !/^https:\/\//.test(l))).toEqual([])
    for (const turn of turns) {
      expect(await h.within(turn)).toBe(true)
      expect(await h.fitsBox(turn)).toBe(true)
    }
    expect(await h.direction(turns[2])).toBe(H().ltr)
    await h.box(v().find).fill(h.mark(t.find))
    await expect.poll(() => h.highlighted()).toEqual([0])
    await expect(turns[0]).toContainText(H().markup)
    await h.box(v().find).fill('')
    await clean(h)
    still(h)
  })

  test('13 · Meeting chat messages, senders and file names show as text', covers('hostile/TC-004', 'F-CALLS-14'), async ({ page }) => {
    const c = H().chat
    const h = await hostileCall(page, v().chat)
    await expect.poll(() => h.hash()).toMatch(new RegExp(`${literal(v().chatEnd)}$`))
    await expect(h.tabStarting(v().chat)).toHaveText(`${v().chat}${c.count}`)
    for (const id of c.pieces) await expect(h.showsText(h.mark(id))).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    expect((await h.links()).filter((l) => /^\s*javascript:/i.test(l))).toEqual([])
    await expect(h.showsText(H().mdLink)).toBeVisible()
    expect(await h.inside(h.mark(c.long[1]))).toBe(true)
    expect(await h.made(h.main, v().imgs)).toBe(0)
    expect(await h.made(h.main, v().bigHeadings)).toBeLessThanOrEqual(1)
    await clean(h)
    still(h)
  })

  test('14 · Live notes, answers and a failed minutes run\'s message show as text', covers('hostile/TC-015', 'F-LIVE-03', 'F-MIN-09', 'F-LIVE-05'), async ({ page }) => {
    const l = H().live
    const h = await hostileCall(page, v().transcript)
    await h.button(v().notesButton).click()
    await expect(h.region(v().liveNotes, h.rightPanel)).toBeVisible()
    await expect(h.showsText(v().asked, h.rightPanel)).toBeVisible()
    for (const id of l.pieces) await expect(h.showsText(h.mark(id), h.rightPanel)).toBeVisible()
    await expect(h.showsText(H().markup, h.rightPanel)).toBeVisible()
    expect(await h.inside(h.mark(l.long), h.rightPanel)).toBe(true)
    await clean(h)
    await h.tab(v().minutes).click()
    await expect(h.shows(l.run)).toBeVisible()
    // The run's buttons are Echo's own: Try again; none named by the run's message.
    await expect(h.button(v().tryAgain)).toBeVisible()
    await expect(h.buttonHolding(h.mark(l.run))).toHaveCount(0)
    await clean(h)
    still(h)
  })

  test('15 · An AI profile named with markup shows as text in Settings, the account chip and the model pickers', covers('hostile/TC-016', 'F-AI-01', 'F-AI-02'), async ({ page }) => {
    const a = H().ai
    const h = await open(page, route('ai'))
    const name = `${a.base} ${h.piece(a.profile, 0)}`
    await expect(h.shows(a.profile)).toBeVisible()
    await expect(h.showsText(H().markup)).toBeVisible()
    expect(await h.inside(h.mark(a.profile))).toBe(true)
    await clean(h)
    await h.goto(route('calls'))
    await expect.poll(async () => (await h.optionTexts(v().accountSelect, h.topBar)).some((o) => o.startsWith(name.slice(0, 40)))).toBe(true)
    await clean(h)
    await h.row(h.piece(H().call.marker)).click()
    for (const select of [v().callProfile, v().minutesProfile]) {
      const opts = await h.optionTexts(select)
      expect(opts.every((o) => !o.includes(H().marker) || o.startsWith(name.slice(0, 30)))).toBe(true)
    }
    await page.keyboard.press(v().palette)
    await h.paletteBox.fill(h.mark(a.profile))
    await clean(h)
    await page.keyboard.press(data.text.shell.keys.escape)
    still(h)
  })

  test('16 · The minutes page opened on its own shows hostile minutes as text', covers('hostile/TC-006', 'F-MIN-01', 'F-CALLS-17'), async ({ page }) => {
    const m = H().minutes
    const h = await open(page, route('today'))
    const folder = callFolder()
    await page.goto(data.base + fill(v().routes.minutes, { folder: encodeURIComponent(folder) }))
    expect(await h.title()).toContain(H().markup)
    const h1 = await h.pageH1()
    expect(h1.startsWith(h.mark(m.heading))).toBe(true)
    expect(h1).toContain(H().markup)
    const made = await h.minutesMarkup(h.body)
    expect(made).toMatchObject({ handlers: 0, imgs: 0, dataScripts: 0, badLinks: 0, frames: 0, fits: true })
    const text = await h.pageText()
    for (const x of m.asText) expect(text).toContain(x)
    for (const id of [...m.cells, m.diagram]) expect(text).toContain(h.mark(id))
    const csp = await h.policy()
    for (const x of m.csp) expect(csp).toContain(x)
    const script = /script-src ([^;]*)/.exec(csp)[1]
    expect(script).not.toContain(m.unsafe)
    expect(script.trim().split(/\s+/).every((x) => /^'sha256-/.test(x))).toBe(true)
    await h.goto(route('today'))
    await expect(new TodayPage(page, data).yourDay).toBeVisible()
    still(h)
  })
})
