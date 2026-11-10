// Calls: the list of calls, a call's tabs, and its Minutes tab (the minutes page in its frame, Write minutes).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'
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
}
