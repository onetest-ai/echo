// Others' words everywhere (S-27, the hostile set): what the window must never do with them, read by the browser —
// a dialog or a page error (Playwright's events), a console error, a request leaving Echo's own server, markup that
// became an element (the cases' check A) — and where each piece shows as text. The pieces are a marker (HOSTILE-NN)
// and one kind of hostile text (data/hostile.json).
import { AppPage } from './app.page.mjs'

/** The console's one allowed message: the browser's own favicon.ico 404. */
const ALLOWED = (m) => /favicon\.ico/.test(m.location().url || '')

export class HostilePage extends AppPage {
  /** Arms the guard at once: every dialog (dismissed), page error, console error and request off Echo's server from
   *  now on is recorded, for `quiet()` to read. */
  constructor(page, data) {
    super(page, data)
    this.h = data.set
    this.seen = { dialogs: [], pageErrors: [], console: [], outside: [] }
    const origin = new URL(data.base).origin
    page.on('dialog', async (d) => { this.seen.dialogs.push(`${d.type()}: ${d.message()}`); await d.dismiss().catch(() => {}) })
    page.on('pageerror', (e) => this.seen.pageErrors.push(String(e)))
    page.on('console', (m) => { if (m.type() === 'error' && !ALLOWED(m)) this.seen.console.push(`${m.text()} ${m.location().url || ''}`.trim()) })
    page.on('request', (r) => { const u = r.url(); if (!/^(data|blob|about):/.test(u) && new URL(u).origin !== origin) this.seen.outside.push(u) })
  }

  /** What the guard recorded since the last read (check D, the console and the requests), emptied. */
  quiet() {
    const r = this.seen
    this.seen = { dialogs: [], pageErrors: [], console: [], outside: [] }
    return r
  }

  /** A piece's text: its marker and the hostile words after it (the first `n` characters; all with `n` 0). */
  piece(id, n = 40) {
    const k = this.h.kinds
    const kind = this.h.pieces[id]
    const body = kind === 'long' ? k.long.repeat(k.longLength) : k[kind]
    const s = `${this.mark(id)} ${body}`
    if (!n) return s
    // Markdown's link text is drawn (or linked) apart: its marker and its first words before the link.
    return kind === 'markdown' ? s.slice(0, s.indexOf('[')).trim() : s.slice(0, n)
  }

  /** Where a piece (or a text) shows as text: the first element of a part of the page holding it. */
  shows(id, where) {
    return (where || this.main).getByText(this.piece(id)).filter({ visible: true }).first()
  }

  showsText(text, where) {
    return (where || this.main).getByText(text).filter({ visible: true }).first()
  }

  /** Every element of a part holding a text, drawn inside that part's right edge (a 10 000-character word wraps). */
  async inside(text, where) {
    const box = await (where || this.main).boundingBox()
    for (const e of await (where || this.main).getByText(text).all()) {
      const b = await e.boundingBox()
      if (b && b.x + b.width > box.x + box.width + 1) return false
    }
    return true
  }

  /** The direction an element is laid out in ("ltr"). */
  async direction(locator) {
    return locator.evaluate((el) => getComputedStyle(el).direction)
  }

  /** Check A of the cases: no inline handler, no inline script, no javascript: or vbscript: address, no broken image
   *  made from the data, and the page no wider than the window. */
  async markup() {
    return this.page.evaluate(() => ({
      handlers: document.querySelectorAll('[onerror],[onload],[onclick],[onmouseover],[onfocus],[onmouseenter]').length,
      inlineScripts: document.querySelectorAll('script:not([src])').length,
      badUrls: [...document.querySelectorAll('[href],[src],[action],[formaction]')].filter((e) => /^\s*(javascript|vbscript):/i.test(e.getAttribute('href') || e.getAttribute('src') || e.getAttribute('action') || e.getAttribute('formaction') || '')).length,
      brokenImgs: document.querySelectorAll('img[src="x"]').length,
      fits: document.documentElement.scrollWidth <= window.innerWidth,
    }))
  }

  /** Links of a part of the page to an address that starts so (none to the data's evil.example in Your day). */
  async linksTo(prefix, where) {
    return (where || this.main).evaluate((el, p) => el.querySelectorAll(`a[href^="${p}"]`).length, prefix)
  }

  /** A part's elements made from Markdown or markup (headings, images, scripts, frames), by tag. */
  async made(where, tags) {
    return (where || this.main).evaluate((el, t) => el.querySelectorAll(t).length, tags)
  }

  /** A part's links, by address. */
  async links(where) {
    return (where || this.main).evaluate((el) => [...el.querySelectorAll('a[href]')].map((a) => a.getAttribute('href')))
  }

  /** Whether a part is no wider than its box (a long word didn't widen it). */
  async fitsBox(where) {
    return where.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)
  }

  /** The minutes in the call's frame and the minutes page on its own: what the data made of elements, its links with
   *  their rel, and whether it fits its window (the frame's checks of the cases). */
  get frame() {
    return this.page.locator('iframe[title="Minutes"]').contentFrame()
  }

  async minutesMarkup(body) {
    return body.evaluate(() => ({
      handlers: document.querySelectorAll('[onerror],[onload],[onclick]').length,
      imgs: document.querySelectorAll('img:not(#lightbox img)').length,
      dataScripts: [...document.querySelectorAll('script')].filter((s) => /alert\(/.test(s.textContent)).length,
      badLinks: [...document.querySelectorAll('a[href]')].filter((a) => !/^https:\/\//.test(a.getAttribute('href'))).length,
      links: [...document.querySelectorAll('a[href]')].map((a) => ({ href: a.getAttribute('href'), noopener: a.relList.contains('noopener') })),
      frames: document.querySelectorAll('iframe,object,embed,form').length,
      fits: document.documentElement.scrollWidth <= window.innerWidth,
    }))
  }

  /** The minutes page's own policy (its Content-Security-Policy meta tag) and title. */
  async policy() {
    return this.page.evaluate(() => document.querySelector('meta[http-equiv="Content-Security-Policy"]')?.content || '')
  }

  async title() {
    return this.page.title()
  }

  /** A row of the Calls list, a button, a tab, an option, a heading: by a text its name holds. */
  row(text) {
    return this.page.getByRole('listbox', { name: 'Calls' }).getByRole('option', { name: text }).first()
  }

  button(name, where) {
    return (where || this.main).getByRole('button', { name, exact: true })
  }

  buttonHolding(text, where) {
    return (where || this.main).getByRole('button', { name: text }).first()
  }

  tab(name, where) {
    return (where || this.main).getByRole('tab', { name, exact: true })
  }

  tabStarting(name, where) {
    return (where || this.main).getByRole('tab', { name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`) })
  }

  options(select) {
    return this.page.getByRole('combobox', { name: select, exact: true }).getByRole('option')
  }

  heading(name, where) {
    return (where || this.main).getByRole('heading', { name, exact: true })
  }

  /** A text box of the page, by its name; its value. */
  box(name, where) {
    return (where || this.page).getByRole('textbox', { name, exact: true })
  }

  /** The actions palette (⌘K). */
  get palette() {
    return this.page.getByRole('dialog', { name: 'Actions' })
  }

  /** A region of the page, by its name (a thread's brief, a client's About). */
  region(name, where) {
    return (where || this.main).getByRole('region', { name, exact: true })
  }

  /** A heading that starts with these words (a long name cut short). */
  headingStarting(words, where) {
    return (where || this.main).getByRole('heading', { name: new RegExp(`^${words.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`) }).first()
  }

  /** An item of the open menu whose name ends so ("Open Umbrella"). */
  menuItemEnding(words) {
    return this.page.getByRole('menuitem', { name: new RegExp(`${words.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`) }).first()
  }

  /** Whether an element is drawn inside a part's right edge. */
  async within(locator, where) {
    // Both read again while a redraw has one of them detached (no box).
    for (let i = 0; ; i++) {
      const box = await (where || this.main).boundingBox()
      const b = await locator.boundingBox()
      if (box && b) return b.x + b.width <= box.x + box.width + 1
      if (i === 50) throw new Error('no box to compare')
      await locator.waitFor()
    }
  }

  /** The headings of a part of the page, by name, in order. */
  async headingNames(where) {
    const out = []
    for (const e of await (where || this.main).getByRole('heading').all()) out.push((await e.textContent()).trim())
    return out
  }

  /** A group of the page by its name, and one of its buttons ("View" › "List"). */
  viewButton(group, name) {
    return this.main.getByRole('group', { name: group, exact: true }).getByRole('button', { name, exact: true })
  }

  /** A row of a table (a person), a list item (a card, a chat message) or an option, holding a text. */
  rowHolding(text, where) {
    return (where || this.main).getByRole('row').filter({ hasText: text }).first()
  }

  itemHolding(text, where) {
    return (where || this.main).getByRole('listitem').filter({ hasText: text }).first()
  }

  optionHolding(text, where) {
    return (where || this.page).getByRole('option', { name: text }).first()
  }

  /** The buttons of a part of the page that match a pattern ("+ 9" folds). */
  buttonsLike(re, where) {
    return (where || this.main).getByRole('button', { name: re })
  }

  /** A select's options as they read (`where`: the top bar, the page). */
  async optionTexts(select, where) {
    const out = []
    for (const o of await (where || this.page).getByRole('combobox', { name: select, exact: true }).getByRole('option').all()) out.push((await o.textContent()).trim())
    return out
  }

  /** A text box's value. */
  value(name, where) {
    return (where || this.main).getByRole('textbox', { name, exact: true })
  }

  /** The text boxes of a part of the page, their values joined (a template's editor). */
  async values(where) {
    const out = []
    for (const b of await (where || this.main).getByRole('textbox').all()) out.push(await b.inputValue())
    return out.join(' ')
  }

  /** The transcript's turns (groups named by their time and speaker), and the turns Find picked out. */
  get turns() {
    // A turn whose speaker is a 10 000-character name has no accessible name: turns are the groups holding a time.
    return this.main.getByRole('group').filter({ has: this.page.getByRole('time') })
  }

  /** The minutes frame's main text and its table cells, read as the person reads them. */
  async frameText() {
    return this.frame.getByRole('main').innerText()
  }

  async frameCells() {
    return this.frame.getByRole('cell').allInnerTexts()
  }

  /** The frame's own page body (for its markup checks). */
  get frameBody() {
    return this.frame.locator('body')
  }

  /** The palette's box. */
  get paletteBox() {
    return this.palette.getByRole('textbox', { name: 'Action' })
  }

  /** A person's name as Echo shows names (its "(…)" and "[…]" parts dropped, a "Last, First" turned round): the
   *  piece's marker and first words, or its marker alone where the words move. */
  person(id) {
    const kind = this.h.pieces[id]
    return kind === 'html' ? `${this.mark(id)} ${this.h.personMarkup}` : kind === 'emoji' ? `${this.mark(id)} ` : this.piece(id)
  }

  showsPerson(id, where) {
    return (where || this.main).getByText(this.person(id)).filter({ visible: true }).first()
  }

  /** A button whose text starts so (a tree row by its item id; a 10 000-character title is too long to name it). */
  buttonWithText(text, where) {
    return (where || this.main).getByRole('button').filter({ hasText: new RegExp(`^${text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`) }).first()
  }

  /** The view's own content: the main element shown (a thread's view keeps a hidden one of the list's layout). */
  get main() {
    return this.page.getByRole('main').filter({ visible: true }).first()
  }

  /** A row of the Calls list holding a text (a 10 000-character title has no name: found by its text). */
  rowWithText(text) {
    return this.page.getByRole('listbox', { name: 'Calls' }).getByRole('option').filter({ hasText: text }).first()
  }

  get callsList() {
    return this.page.getByRole('listbox', { name: 'Calls' })
  }

  get callRows() {
    return this.callsList.getByRole('option')
  }

  /** The palette's options of a group holding a text. */
  paletteOption(text, group) {
    return this.palette.getByRole('group', { name: group, exact: true }).getByRole('option').filter({ hasText: text }).first()
  }

  /** The minutes frame: its first heading's words (the page hides its own title in the frame), a text of it, its
   *  figure (a diagram), its time chips, a list item holding words. */
  async frameH1() {
    // Hidden in the frame (the call's header shows the title), so not a heading a person reads there: its element.
    return (await this.frame.locator('h1').first().textContent({ timeout: 10_000 })).trim()
  }

  frameSays(text) {
    return this.frame.getByText(text).first()
  }

  async frameFigure() {
    return this.frame.getByRole('figure').innerText()
  }

  frameItem(text) {
    return this.frame.getByRole('listitem').filter({ hasText: text }).first()
  }

  get frameMain() {
    return this.frame.getByRole('main')
  }

  /** A time chip of a minutes line (the line holding these words). */
  frameChip(time, words) {
    return this.frameItem(words).getByText(time, { exact: true }).first()
  }

  /** The transcript's turns drawn highlighted (by their places, from 0). */
  async highlighted() {
    const out = []
    for (const [i, t] of (await this.turns.all()).entries()) if (await t.evaluate((el) => getComputedStyle(el).backgroundColor !== 'rgba(0, 0, 0, 0)')) out.push(i)
    return out
  }

  /** The minutes page on its own: its first heading's words (the case reads its h1), its main text, its body. */
  async pageH1() {
    return (await this.page.locator('h1').first().textContent()).trim()
  }

  async pageText() {
    return this.page.getByRole('main').first().innerText()
  }

  get body() {
    return this.page.locator('body')
  }

  /** A select of the page, by its name (the Work view's Layout). */
  select(name) {
    return this.page.getByRole('combobox', { name, exact: true })
  }

  /** My notes as the file's text (after Edit as text): its box. */
  get rawNotes() {
    return this.rightPanel.getByRole('textbox').first()
  }

  /** A piece's marker ("HOSTILE-48"). */
  mark(id) {
    return `${this.h.marker}${id}`
  }
}
