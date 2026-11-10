// Calls: the list of calls, a call's tabs, and its Minutes tab (the minutes page in its frame, Write minutes).
import { AppPage } from './app.page.mjs'
import { fill, literal } from '../support/data.mjs'
import { readLines } from '../support/aria.mjs'

export class CallsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.list = page.getByRole('listbox', { name: 'Calls' })
    this.writeMinutes = page.getByRole('button', { name: 'Write minutes', exact: true })
    this.writeNow = page.getByRole('button', { name: 'Write now', exact: true })
    this.callActions = page.getByRole('button', { name: 'Call actions' })
    // The minutes page: the frame titled "Minutes" (an iframe has no role; the tab shares its title).
    this.minutes = page.locator('iframe[title="Minutes"]').contentFrame()
  }

  async open() {
    await this.goto('#calls')
  }

  /** A call in the list, by its title. */
  call(title) {
    return this.list.getByRole('option', { name: new RegExp(literal(title)) })
  }

  async openCall(title, tab) {
    await this.call(title).click()
    await this.list.getByRole('option', { name: new RegExp(literal(title)), selected: true }).waitFor()
    if (tab) await this.tab(tab).click()
  }

  /** A call's minutes as their own page (`/minutes/<call>`, what Open in browser shows), not in the window's frame. */
  async openMinutesPage(folder) {
    await this.page.goto(`${this.data.base}/minutes/${folder}`)
  }

  /** Call actions → an item of its menu ("Write minutes again", …). */
  async callAction(item) {
    await this.callActions.click()
    await this.page.getByRole('menuitem', { name: item }).click()
  }

  tab(name) {
    return this.page.getByRole('tab', { name: new RegExp(`^${literal(name)}`) })
  }

  // ── A call's header: its kind, its threads, its buttons; the list's groups ──

  /** A day group of the list ("Today", "Yesterday"). */
  listGroup(name) {
    return this.list.getByText(name, { exact: true })
  }

  /** The kind of meeting: its select, and its chip by the tooltip it has (a guess, or yours). */
  get kind() {
    return this.main.getByRole('combobox', { name: 'Kind of meeting' })
  }

  kindChip(tooltip) {
    return this.main.getByTitle(tooltip, { exact: true })
  }

  /** A button of the header ("+ Thread", "What moved", a thread's chip), by its name. */
  headerButton(name) {
    return this.main.getByRole('button', { name, exact: true })
  }

  /** A thread's chip in the header (it reads the thread's name twice when the thread wears a picture, named by it). */
  threadChip(name) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(name)}( ${literal(name)})?$`) })
  }

  /** A thread in the "+ Thread" menu (a checkbox item, ticked while the call is in it), or another item of a menu. */
  threadItem(name) {
    return this.page.getByRole('menu').getByRole('menuitemcheckbox', { name: new RegExp(`^(✓ )?${literal(name)}$`) })
  }

  get menuItems() {
    return this.page.getByRole('menu').getByRole('menuitem')
  }

  /** An item of the open menu, by its name ("New thread from this call…"). */
  menuItem(name) {
    return this.page.getByRole('menu').getByRole('menuitem', { name, exact: true })
  }

  // ── Coming up today: the list's fold of the meetings still to come, and their rows ──

  /** The fold's head ("Coming up today 2 Next 02:04 PM Globex design review"). */
  get comingUp() {
    return this.list.getByRole('button', { name: new RegExp(`^${literal(this.data.text.upcoming.head)}`) })
  }

  /** The list's rows (the fold's head, then the calls), in order. */
  get rows() {
    return this.list.getByRole('button').or(this.list.getByRole('option'))
  }

  /** A meeting still to come, by its subject (its row reads "Coming up <subject> <time> …"). */
  upcoming(subject) {
    return this.list.getByRole('option', { name: new RegExp(`^${literal(this.data.text.upcoming.rowPrefix)} ${literal(subject)} `) })
  }

  /** The meetings still to come, listed. */
  get upcomingRows() {
    return this.list.getByRole('option', { name: new RegExp(`^${literal(this.data.text.upcoming.rowPrefix)} `) })
  }

  /** A thread's icon on a row (its tooltip names the thread). */
  rowIcon(row, thread) {
    return row.getByTitle(thread, { exact: true })
  }

  /** Whether a row draws a thread's icon before one of its texts (its time), from the left. */
  async iconBefore(row, thread, text) {
    const icon = await this.rowIcon(row, thread).boundingBox()
    const at = await this.rowText(row, text).boundingBox()
    return !!icon && !!at && icon.x < at.x
  }

  /** A text of a row, as it reads (its time). */
  rowText(row, text) {
    return row.getByText(text, { exact: true })
  }

  get search() {
    return this.page.getByRole('textbox', { name: this.data.text.calls.search, exact: true })
  }

  /** The transcript's turns, by their times. */
  get turns() {
    return this.main.getByRole('time')
  }

  // ── The list as it reads, a call's header and minutes, the transcript's turns, My notes, a call's delete ──

  /** What the list reads from the top, in order: its rows' names and its day headings. */
  async lines() {
    return readLines(await this.list.ariaSnapshot())
  }

  /** The calls listed (the meetings still to come left out). */
  get callRows() {
    return this.list.getByRole('option', { name: new RegExp(`^(?!${literal(this.data.text.upcoming.rowPrefix)} )`) })
  }

  /** A text of the view, as it reads (a fact of the call's header, a line of a tab); one that starts with these words;
   *  a heading of a tab ("No minutes yet"). */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  saysStarting(words) {
    return this.main.getByText(new RegExp(`^${literal(words)}`))
  }

  paneHeading(name) {
    return this.main.getByRole('heading', { name, level: 3, exact: true })
  }

  /** The call's tabs, in order. */
  get callTabs() {
    return this.main.getByRole('tab')
  }

  /** A text of the right panel, as it reads (its thread in What moved, My notes' saved state). */
  panelSays(text) {
    return this.rightPanel.getByText(text, { exact: true })
  }

  /** The minutes' section headings, in order; an item of theirs, by the words it holds; its time chip. */
  get sections() {
    return this.minutes.getByRole('heading', { level: 2 })
  }

  minutesItem(words) {
    return this.minutes.getByRole('listitem').filter({ hasText: words })
  }

  timeChip(words, time) {
    return this.minutesItem(words).getByText(time, { exact: true })
  }

  /** A text of the minutes, as it reads (their AI notice). */
  minutesSay(text) {
    return this.minutes.getByText(text, { exact: true })
  }

  /** Whether the minutes fill the tab: their frame as wide as the view, no column beside them. */
  async minutesFill() {
    const frame = await this.page.locator('iframe[title="Minutes"]').boundingBox()
    const view = await this.main.boundingBox()
    return Math.abs(frame.width - view.width) <= 2
  }

  /** Find in transcript. */
  get find() {
    return this.main.getByRole('textbox', { name: 'Find in transcript', exact: true })
  }

  /** A turn of the transcript, by its time and speaker ("0:37 Me"); every turn. */
  turn(name) {
    return this.main.getByRole('group', { name, exact: true })
  }

  get turnGroups() {
    return this.main.getByRole('group', { name: /^\d+:\d\d / })
  }

  async turnNames() {
    const names = []
    for (const t of await this.turnGroups.all()) names.push(await t.getAttribute('aria-label'))
    return names
  }

  /** The turns drawn highlighted (a background of their own), by their names. */
  async highlighted() {
    const names = []
    for (const t of await this.turnGroups.all()) {
      if (await t.evaluate((el) => getComputedStyle(el).backgroundColor !== 'rgba(0, 0, 0, 0)')) names.push(await t.getAttribute('aria-label'))
    }
    return names
  }

  /** My notes in the right panel: the box for a new note, a note by its text, a text of the panel (its saved state). */
  get newNote() {
    return this.rightPanel.getByRole('textbox', { name: 'New note', exact: true })
  }

  note(text) {
    return this.rightPanel.getByText(text, { exact: true })
  }

  /** Hold the window's requests to the app (a save) until `release()`: what the window says while one is under way
   *  can be read. */
  async holdRequests() {
    const gate = new Promise((r) => { this.letGo = r })
    await this.page.route('**/ipc', async (route) => { await gate; await route.continue() })
  }

  /** Let the held requests go and stop holding (at once, and again harmlessly). */
  async release() {
    if (this.letGo) this.letGo()
    await this.page.unrouteAll({ behavior: 'wait' })
  }

  // ── A call in each state (S-23): the newest row of a title, its clock chip, the Minutes box, the header's facts,
  //    What moved's threads and cards, Chat, Screens, live notes in My notes ──

  /** The newest row of a title: the list's first whose name is its icon, then the title ("Memo Memo …"). */
  newest(title) {
    return this.list.getByRole('option', { name: new RegExp(`^\\S+ ${literal(title)} `) }).first()
  }

  async openNewest(title, tab) {
    await this.newest(title).click()
    await this.heading(title).waitFor()
    if (tab) await this.tab(tab).click()
  }

  /** An element of a row (or of the whole list) by its tooltip ("Minutes at 03:50 PM"), as a pattern. */
  titled(re, row) {
    return (row || this.list).getByTitle(re)
  }

  /** A heading of the tab (level 3) that matches a pattern ("Minutes in 13 min (03:50 PM)"). */
  paneHeadingLike(re) {
    return this.main.getByRole('heading', { level: 3, name: re })
  }

  /** A text of the view that matches a pattern (the countdown's ring, "13min"). */
  saysLike(re) {
    return this.main.getByText(re)
  }

  /** A select of the view, by its name; its option chosen. */
  select(name) {
    return this.main.getByRole('combobox', { name, exact: true })
  }

  chosen(select, name) {
    return this.select(select).getByRole('option', { name, exact: true, selected: true })
  }

  /** A menu's items (Not yet ▾'s), and one by its name. */
  get menu() {
    return this.main.getByRole('menu')
  }

  menuItemNamed(name) {
    return this.menu.getByRole('menuitem', { name, exact: true })
  }

  /** The minutes' frame on the tab (there or not). */
  get minutesFrame() {
    return this.page.locator('iframe[title="Minutes"]')
  }

  /** The frame's width, and the tab's (the view below the call's header), in pixels. */
  async minutesWidth() {
    return (await this.minutesFrame.boundingBox()).width
  }

  /** A button and a select drawn apart (no overlap), and the button wide enough to read its words in full. */
  async buttonBeside(button, select) {
    const b = await button.boundingBox()
    const s = await select.boundingBox()
    const overlap = b.x < s.x + s.width && s.x < b.x + b.width && b.y < s.y + s.height && s.y < b.y + b.height
    const cut = await button.evaluate((el) => el.scrollWidth > el.clientWidth + 1)
    return { width: Math.round(b.width), overlap, cut }
  }

  /** The call's header facts (its date, length, kind, import, people), as the group reads them line by line. */
  get facts() {
    return this.main.getByRole('group', { name: this.data.text.callStates.facts, exact: true })
  }

  async factLines() {
    const snap = await this.facts.ariaSnapshot()
    // A group of one text reads on its own line ("- group "About this call": Memo …").
    if (!snap.includes('\n')) return [snap.replace(/^- group "[^"]*":\s*/, '').replace(/^"(.*)"$/, '$1')]
    return readLines(snap)
  }

  /** What moved: a thread's line, a change's card (named by its label, item id and title), the cards in order. */
  movedThread(name) {
    return this.rightPanel.getByRole('group', { name, exact: true })
  }

  movedCard(name) {
    return this.rightPanel.getByRole('group', { name, exact: true })
  }

  get movedCards() {
    return this.rightPanel.getByRole('group', { name: / [A-Z]\d{3} / })
  }

  async movedCardNames() {
    const names = []
    for (const c of await this.movedCards.all()) names.push(await c.getAttribute('aria-label'))
    return names
  }

  /** A button of a card or a line (its time chip, its item, Sync now), by its name. */
  inside(group, name) {
    return group.getByRole('button', { name, exact: true })
  }

  /** The Chat tab's messages, each as it reads: who, when, what, the files. */
  get messages() {
    return this.main.getByRole('article')
  }

  async messageLines(i) {
    return readLines(await this.messages.nth(i).ariaSnapshot()).map((l) => l.replace(/^(time|paragraph):\s*/, '').replace(/^"(.*)"$/, '$1'))
  }

  /** The Screens tab's frames (buttons named by their time) and the image one opens large. */
  get frames() {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(fill(this.data.text.callStates.shared, { t: '' }))}`) })
  }

  frame(t) {
    const s = fill(this.data.text.callStates.shared, { t })
    return this.main.getByRole('button', { name: `${s} ${t}`, exact: true })
  }

  frameImage(t) {
    return this.main.getByRole('img', { name: fill(this.data.text.callStates.shared, { t }), exact: true })
  }

  get largeFrame() {
    return this.page.getByRole('img', { name: this.data.text.callStates.sharedLarge, exact: true })
  }

  /** The tab's own width and the transcript's: whether the transcript fills it (within a margin). */
  async transcriptFills(margin) {
    const view = await this.main.boundingBox()
    const turn = await this.turnGroups.first().boundingBox()
    return turn.width >= view.width - margin
  }

  /** The turn nearest to the middle of the tab's visible part (below the call's header), by its time in seconds;
   *  whether the first turn has scrolled out of sight above it. */
  async centredTurn() {
    const top = await this.main.getByRole('tablist').boundingBox()
    const view = await this.main.boundingBox()
    // The tabs end the call's header; while the header is out of sight the view's own top is the visible part's.
    const from = top ? top.y + top.height : view.y
    const mid = (from + view.y + view.height) / 2
    let best = null
    for (const t of await this.turnGroups.all()) {
      const b = await t.boundingBox()
      const d = Math.abs(b.y + b.height / 2 - mid)
      if (!best || d < best.d) best = { d, name: await t.getAttribute('aria-label') }
    }
    const first = await this.turnGroups.first().boundingBox()
    const [m, s] = best.name.split(' ')[0].split(':').map(Number)
    return { at: m * 60 + s, scrolled: first.y + first.height <= from }
  }

  /** My notes' list (the right panel's), its rows as each reads, its heading line; the live notes' region. */
  get notesList() {
    return this.rightPanel.getByRole('list', { name: this.data.text.callStates.notesList, exact: true })
  }

  get noteRows() {
    return this.notesList.getByRole('listitem')
  }

  async noteTexts() {
    const out = []
    for (const r of await this.noteRows.all()) out.push((await r.innerText()).replace(/\s+/g, ' ').trim())
    return out
  }

  noteRow(text) {
    return this.noteRows.filter({ hasText: text })
  }

  liveRegion(where) {
    return (where || this.rightPanel).getByRole('region', { name: this.data.text.callStates.liveNotes, exact: true })
  }

  /** What the live notes' region reads, line by line (Now, its words, the status line of parts joined by dots). */
  async liveLines() {
    return readLines(await this.liveRegion().ariaSnapshot()).map((l) => l.replace(/^paragraph:\s*/, '').replace(/^"(.*)"$/, '$1'))
  }

  /** The Inbox's item (in the right panel), by its title. */
  inboxItem(title) {
    return this.rightPanel.getByRole('group', { name: title, exact: true })
  }

  /** The prep on its tab (an article). */
  get prepArticle() {
    return this.main.getByRole('article')
  }

  /** A select's options, in order; the open menu's items, in order. */
  options(select) {
    return this.select(select).getByRole('option')
  }

  get menuItemsShown() {
    return this.menu.getByRole('menuitem')
  }

  /** The header's facts: an element by its tooltip. */
  factTitled(title) {
    return this.facts.getByTitle(title, { exact: true })
  }

  /** A text of the list ("Nothing matches …"). */
  listSays(text) {
    return this.list.getByText(text, { exact: true })
  }

  /** A text of the right panel that contains these words (Details' item id and title). */
  panelHas(text) {
    return this.rightPanel.getByText(text).first()
  }

  /** The right panel's tab shown. */
  get panelTabShown() {
    return this.rightPanel.getByRole('tab', { selected: true })
  }

  /** A text of a group or a row, as it reads (a live row's time chip). */
  within(group, text) {
    return group.getByText(text, { exact: true })
  }

  /** The note editor in My notes (a note's ✎ opens it). */
  get noteEditor() {
    return this.rightPanel.getByRole('textbox', { name: this.data.text.callStates.editNote, exact: true })
  }

  /** My notes' live rows (✦). */
  get liveRows() {
    return this.noteRows.filter({ hasText: this.data.text.callStates.liveMark })
  }

  /** The live notes' ⋯ menu, inside their region; a text of it, its select, an item. */
  get liveMenu() {
    return this.liveRegion().getByRole('menu', { name: this.data.text.callStates.liveNotes, exact: true })
  }

  liveMenuSays(text) {
    return this.liveMenu.getByText(text, { exact: true })
  }

  liveMenuItem(name) {
    return this.liveMenu.getByRole('menuitem', { name, exact: true })
  }

  liveMenuSelect(name) {
    return this.liveMenu.getByRole('combobox', { name, exact: true })
  }

  get liveMenuButton() {
    return this.liveRegion().getByRole('button', { name: this.data.text.callStates.liveMenu, exact: true })
  }
}
