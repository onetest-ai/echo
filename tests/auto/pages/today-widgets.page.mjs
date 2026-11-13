// Today's widgets (S-24 stages 41–52): each widget a region named as Today draws it, where it sits (its rows, the held
// first column), Your day's lines, edit mode and Add a widget, and the window server's own answers (`/api/…`).
import { TodayPage } from './today.page.mjs'
import { literal } from '../support/data.mjs'

/** In the page: Today's named regions (the main landmark's `section`s with a label and `role="region"`s, not under
 *  aria-hidden), the first of each of these names, by name; `frames`: only edit mode's frames (holding a Size group).
 *  A source text, so `onWidget` and `boxes` can find and read in one evaluation. */
const REGIONS = `(names, frames = false) => {
  const main = document.querySelector('main, [role="main"]'), out = {}
  if (!main) return out
  const nameOf = (el) => {
    const by = el.getAttribute('aria-labelledby')
    return (el.getAttribute('aria-label') ?? (by ? by.split(/\\s+/).map((id) => document.getElementById(id)?.textContent ?? '').join(' ') : '')).trim()
  }
  for (const el of main.querySelectorAll('section[aria-label], section[aria-labelledby], [role="region"]')) {
    if (el.closest('[aria-hidden="true"]')) continue
    if (frames && !el.querySelector('[role="group"][aria-label="Size"]')) continue
    const name = nameOf(el)
    if (names.includes(name) && !(name in out)) out[name] = el
  }
  return out
}`

export class TodayWidgetsPage extends TodayPage {
  constructor(page, data) {
    super(page, data)
    this.w = data.text.widgets
    this.editToday = this.main.getByRole('button', { name: 'Edit Today', exact: true })
    this.addWidget = this.main.getByRole('button', { name: 'Add a widget', exact: true })
    this.resetLayout = this.main.getByRole('button', { name: 'Reset to the default', exact: true })
    this.done = this.main.getByRole('button', { name: 'Done', exact: true })
    this.dropEnd = this.main.getByText(/^Drop here for a new last row/)
    // Add a widget (the right panel in edit mode): search, parts, a row a widget, the picked one's preview and Add.
    this.widgetTab = this.rightPanel.getByRole('tab', { name: 'Add a widget', exact: true })
    this.widgetSearch = this.rightPanel.getByRole('searchbox', { name: 'Search widgets', exact: true })
    this.parts = this.rightPanel.getByRole('group', { name: 'Parts', exact: true })
    this.widgetList = this.rightPanel.getByRole('list', { name: 'Widgets', exact: true })
    this.pickSizes = this.rightPanel.getByRole('group', { name: 'Size', exact: true })
    this.panelDone = this.rightPanel.getByRole('button', { name: 'Done', exact: true })
    // The right panel's tab "Meeting", gone since a meeting's card opens beside it (a click on a line or a block), and
    // Calendar's menu (⋯ or a right-click).
    this.meetingTab = this.rightPanel.getByRole('tab', { name: this.w.day.meetingTab, exact: true })
    this.menu = page.getByRole('menu')
  }

  /** What the window's server answers at `/api/<path>` (`widgets`, `today`), as the page gets it (its cookie). */
  async api(path) {
    const r = await this.page.request.get(`${this.data.base}/api/${path}`)
    return r.json()
  }

  /** Today, loaded afresh (a reload after a change made with `echoctl today`), once its head is drawn. */
  async reloadToday() {
    await this.page.reload()
    await this.editToday.or(this.done).first().waitFor()
  }

  /** Today opened, once its head is drawn (Needs you now may be off Today). */
  async openToday() {
    await this.goto('#today')
    await this.editToday.or(this.done).first().waitFor()
  }

  /** A widget on Today, by the name it is drawn with ("Your day", "Next meeting"); in edit mode, its frame. */
  widget(name) {
    return this.main.getByRole('region', { name, exact: true }).first()
  }

  /** Run `fn(el, arg)` in the page on the widget of this name (as `widget` finds it, once it is drawn), finding it in
   *  the same step: Today may be drawn again at any moment, and a widget found in one step and read in the next could be
   *  gone in between, read as empty or not drawn (#476). `fn` runs in the page: only what it is given. */
  async onWidget(name, fn, arg = null) {
    await this.widget(name).waitFor()
    return this.page.evaluate(`(() => { const el = (${REGIONS})(${JSON.stringify([name])})[${JSON.stringify(name)}]; return el ? (${fn})(el, ${JSON.stringify(arg)}) : null })()`)
  }

  /** The boxes of Today's widgets of these names, by name (the first of each name, as `widget` finds it; one not drawn
   *  or not visible is left out). Found and measured in one step in the page, by their roles (the main landmark, its
   *  named regions): Today may be drawn again at any moment (a second load of its data lands), and a widget found in
   *  one step and measured in the next could be gone in between, read as not drawn (#476).
   *  `frames`: edit mode's frames (a region holding a Size group), as `frame` finds them. */
  async boxes(names, { frames = false } = {}) {
    const at = (r) => ({ x: r.x, y: r.y, width: r.width, height: r.height })
    return this.page.evaluate(`(() => { const out = {}; for (const [name, el] of Object.entries((${REGIONS})(${JSON.stringify(names)}, ${frames}))) { const r = el.getBoundingClientRect(); if (r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden') out[name] = (${at})(r) } return out })()`)
  }

  /** Where the widgets of these names are: rows top to bottom (a row: those starting level, left to right), each a
   *  name; a widget not drawn is left out. */
  async rows(names) {
    const at = await this.boxes(names)
    const boxes = names.filter((name) => at[name]).map((name) => ({ name, ...at[name] }))
    boxes.sort((a, b) => a.y - b.y || a.x - b.x)
    const rows = []
    for (const b of boxes) {
      const row = rows.find((r) => Math.abs(r.y - b.y) <= 4)
      if (row) row.items.push(b)
      else rows.push({ y: b.y, items: [b] })
    }
    return rows.map((r) => r.items.sort((a, b) => a.x - b.x).map((b) => b.name))
  }

  /** A widget's box (x, y, width, height), once it is drawn; read as `boxes` reads it. */
  async box(name) {
    await this.widget(name).waitFor()
    return (await this.boxes([name]))[name] ?? null
  }

  /** Your day as it holds the first column (#367/#372): `held` (it sits sticky in a column of its own), its column's
   *  width and height and those of what is beside it, its own height, the view's height. */
  async hold(name = this.w.name.yourDay) {
    return this.onWidget(name, (el) => {
      const col = el.parentElement, beside = col.nextElementSibling
      const r = (x) => (x ? x.getBoundingClientRect() : { width: 0, height: 0 })
      return {
        held: getComputedStyle(el).position === 'sticky',
        width: r(col).width,
        column: r(col).height,
        besideWidth: r(beside).width,
        beside: r(beside).height,
        height: el.getBoundingClientRect().height,
        top: el.getBoundingClientRect().top,
        view: window.innerHeight,
      }
    })
  }

  /** The part of a widget that scrolls inside it (Your day's list, My work's cards): how far it is scrolled, whether it
   *  holds more than it shows, and whether the fade at its bottom ("there is more") is drawn. */
  async inner(name) {
    return this.onWidget(name, (el) => {
      const all = [el, ...el.querySelectorAll('*')]
      const s = all.find((x) => ['auto', 'scroll'].includes(getComputedStyle(x).overflowY) && x.scrollHeight > x.clientHeight + 1)
        || all.find((x) => ['auto', 'scroll'].includes(getComputedStyle(x).overflowY))
      if (!s) return null
      const after = getComputedStyle(s, '::after').content
      return { top: s.scrollTop, scrolls: s.scrollHeight > s.clientHeight + 1, atEnd: s.scrollTop + s.clientHeight >= s.scrollHeight - 1, fade: after !== 'none' && after !== 'normal' }
    })
  }

  /** Scroll a widget's inner list to its end with the wheel, as a person does, until it is there. */
  async scrollInnerToEnd(name) {
    const b = await this.box(name)
    await this.page.mouse.move(b.x + b.width / 2, b.y + b.height - 40)
    for (let i = 0; i < 20 && !(await this.inner(name)).atEnd; i++) await this.page.mouse.wheel(0, 400)
  }

  /** Scroll Today itself with the wheel, over a widget beside Your day. */
  async scrollToday(over, dy) {
    const b = await this.box(over)
    await this.page.mouse.move(b.x + b.width / 2, b.y + Math.min(b.height / 2, 40))
    await this.page.mouse.wheel(0, dy)
  }

  /** Whether the line now of Your day's list is in view inside the list (it opens on now). */
  async nowInView() {
    // The line and the list read in one step (`onWidget`), never from two drawings of Your day.
    await this.nowLine.waitFor()
    return this.onWidget(this.w.name.yourDay, (el) => {
      const list = el.querySelector('[role="list"][aria-label="Where you\'ll be"]')
      const line = list && [...el.querySelectorAll('[role="separator"]')].find((x) => /^Now /.test(x.getAttribute('aria-label') || ''))
      if (!list || !line) return false
      const [a, b] = [line.getBoundingClientRect(), list.getBoundingClientRect()]
      return a.height > 0 && b.height > 0 && a.y >= b.y - 1 && a.y + a.height <= b.y + b.height + 1
    })
  }

  /** Whether anything inside a widget is drawn past its frame or cut (a text wider than its box). */
  async overflows(name) {
    return this.onWidget(name, (el) => {
      const f = el.getBoundingClientRect()
      return [...el.querySelectorAll('*')].filter((x) => {
        const b = x.getBoundingClientRect()
        if (!b.width && !b.height) return false
        const out = b.left < f.left - 1 || b.right > f.right + 1 || b.top < f.top - 1 || b.bottom > f.bottom + 1
        const cut = x.children.length === 0 && x.scrollWidth > x.clientWidth + 1 && getComputedStyle(x).overflow !== 'visible'
        return out || cut
      }).map((x) => x.textContent.trim().slice(0, 30) || x.tagName)
    })
  }

  // ── Your day's lines ──

  /** A line of Your day by its subject; of several of one subject, the first. */
  line(subject) {
    return this.yourDay.getByRole('listitem', { name: subject, exact: true }).first()
  }

  /** The lines' times, in order, as minutes of the day ("02:23 PM" → 863), the work line's left out. */
  async lineTimes() {
    const out = []
    for (const l of await this.yourDay.getByRole('listitem').all()) {
      const name = await l.getAttribute('aria-label')
      if (name.startsWith('Your work: ')) continue
      const m = /^(\d\d):(\d\d)(?: ([AP]M))?/.exec((await l.innerText()).trim())
      out.push(((+m[1] % 12) + (m[3] === 'PM' ? 12 : 0) + (m[3] ? 0 : Math.floor(+m[1] / 12) * 12)) * 60 + +m[2])
    }
    return out
  }

  /** How a line is drawn: its time's colour, its bar (solid: its background; dashed: its border, style and colour). */
  async lineLook(subject, which = 0) {
    return this.yourDay.getByRole('listitem', { name: subject, exact: true }).nth(which).evaluate((el) => {
      const [time, bar] = el.children
      const t = getComputedStyle(time), b = getComputedStyle(bar)
      return { time: t.color, bar: b.borderTopStyle === 'dashed' ? 'dashed' : 'solid', barColour: b.borderTopStyle === 'dashed' ? b.borderTopColor : b.backgroundColor }
    })
  }

  /** A line's text that reads so, or one of its links ("new minutes · 1 decision"). */
  lineSays(subject, text) {
    return this.line(subject).getByText(text, { exact: true })
  }

  lineLink(subject, name) {
    return this.line(subject).getByRole('button', { name, exact: true })
  }

  /** Show / Hide the skipped. */
  skipped(name) {
    return this.yourDay.getByRole('button', { name, exact: true })
  }

  /** A meeting's card, Calendar's, as a popover in Today's view, by its subject. */
  meetingCard(subject) {
    return this.main.getByRole('complementary', { name: subject, exact: true })
  }

  /** Where a meeting's card is, as Calendar's beside its block: inside Today's view, beside what opened it (right or
   *  left of it, apart), level with it as far as the view lets it (12 px from its edges). */
  async meetingCardPlace(subject, anchor) {
    const card = await this.meetingCard(subject).boundingBox()
    const at = await anchor.boundingBox()
    const g = await this.main.boundingBox()
    const bottom = Math.min(g.y + g.height, this.page.viewportSize().height)
    const level = Math.min(Math.max(g.y + 12, at.y), Math.max(g.y + 12, bottom - card.height - 12))
    return {
      inside: card.x >= g.x && card.y >= g.y && card.x + card.width <= g.x + g.width && card.y + card.height <= bottom,
      beside: card.x >= at.x + at.width || card.x + card.width <= at.x,
      level: Math.abs(card.y - level) <= 1,
    }
  }

  /** The open menu's items, as they read. */
  async menuItems() {
    await this.menu.waitFor()
    const out = []
    for (const i of await this.menu.getByRole('menuitem').all()) out.push((await i.innerText()).trim())
    return out
  }

  // ── Your day as a calendar and as a strip ──

  /** The calendar's grid (the hours down) and the strip's bar (the day in one bar). */
  get calendarList() {
    return this.main.getByRole('list', { name: 'Your day by the hour', exact: true })
  }

  get calendarBlocks() {
    return this.calendarList.getByRole('listitem')
  }

  stripBar() {
    return this.main.getByRole('list', { name: 'Your day in one bar', exact: true })
  }

  stripBlocks() {
    return this.stripBar().getByRole('listitem')
  }

  /** A meeting's block in a widget (the calendar's region "Your day", the strip's), by its subject. */
  block(widget, subject) {
    const list = widget === this.w.name.strip ? this.stripBar() : this.calendarList
    return list.getByRole('listitem', { name: subject, exact: true })
  }

  /** How a block is drawn: dashed or solid, and the colour of its border. */
  async blockLook(widget, subject) {
    // Read from a block in the page: one replaced by a redraw at that moment has no style (an empty colour) — read
    // again, a few times, until the page holds it.
    for (let i = 0; ; i++) {
      const look = await this.block(widget, subject).first().evaluate((el) => {
        const s = getComputedStyle(el)
        return { border: s.borderTopStyle === 'dashed' ? 'dashed' : 'solid', colour: s.borderTopColor, on: el.isConnected }
      })
      if ((look.on && look.colour) || i >= 20) return { border: look.border, colour: look.colour }
      await new Promise((r) => setTimeout(r, 100))
    }
  }

  /** The strip's hour ticks and the calendar's hours, as numbers, in order. */
  async stripTicks() {
    return (await this.widget(this.w.name.strip).getByText(/^\d\d$/).allInnerTexts()).map(Number)
  }

  async calendarHours() {
    return (await this.calendarList.getByText(/^\d\d$/).allInnerTexts()).map(Number)
  }

  /** The calendar's line now inside its scrolled box (it opens a little before now). */
  async calendarNowInView() {
    const line = await this.calendarList.getByRole('separator').boundingBox()
    const box = await this.widget(this.w.name.yourDay).boundingBox()
    return !!line && line.y >= box.y && line.y <= box.y + box.height
  }

  // ── Edit Today and Add a widget ──

  /** A widget's frame in edit mode, by its label ("My work · this week"); its moves, sizes and ×. */
  frame(label) {
    return this.main.getByRole('region', { name: label, exact: true }).filter({ has: this.page.getByRole('group', { name: 'Size', exact: true }) })
  }

  frameSize(label, size) {
    return this.frame(label).getByRole('button', { name: `${label}: size ${size}`, exact: true })
  }

  frameMove(label, says) {
    return this.frame(label).getByRole('button', { name: `Move ${label} ${says}`, exact: true })
  }

  frameRemove(label) {
    return this.frame(label).getByRole('button', { name: `Take ${label} off Today`, exact: true })
  }

  /** The frames in edit mode, top to bottom and left to right, by their labels. */
  async frameRows(labels) {
    const rows = []
    const at = await this.boxes(labels, { frames: true })
    const boxes = labels.filter((label) => at[label]).map((label) => ({ label, ...at[label] }))
    boxes.sort((a, b) => a.y - b.y || a.x - b.x)
    for (const b of boxes) {
      const row = rows.find((r) => Math.abs(r.y - b.y) <= 4)
      if (row) row.items.push(b)
      else rows.push({ y: b.y, items: [b] })
    }
    return rows.map((r) => r.items.sort((a, b) => a.x - b.x).map((b) => b.label))
  }

  /** Every frame ends inside its row: none reaches under the top of a frame below it in the same column. */
  async framesInRows(labels) {
    const at = await this.boxes(labels, { frames: true })
    const boxes = labels.filter((label) => at[label]).map((label) => ({ label, ...at[label] }))
    const under = (a, b) => b.y > a.y + 4 && b.x < a.x + a.width - 1 && a.x < b.x + b.width - 1
    return boxes.every((a) => boxes.filter((b) => under(a, b)).every((b) => a.y + a.height <= b.y + 1))
  }

  /** Drag a frame onto another's edge or half: `above`, `below` (its top / bottom quarter), `before`, `after` (its left /
   *  right half), as `dropSide` reads it; or onto the zone after the last widget. */
  async dragFrame(from, to, side) {
    await from.dragTo(to, { targetPosition: await this.spot(to, side) })
  }

  async spot(target, side) {
    const b = await target.boundingBox()
    const at = { above: [0.5, 0.06], below: [0.5, 0.94], before: [0.25, 0.5], after: [0.75, 0.5] }[side] || [0.5, 0.5]
    return { x: b.width * at[0], y: b.height * at[1] }
  }

  /** The line drawn while a drag is over a frame, before the drop: which side it marks. */
  async hoverDrag(from, to, side) {
    const a = await from.boundingBox()
    await this.page.mouse.move(a.x + 12, a.y + 10)
    await this.page.mouse.down()
    await this.page.mouse.move(a.x + 30, a.y + 24, { steps: 4 })
    // Measured once the drag has begun (the page may move as it starts).
    const b = await to.boundingBox()
    const p = await this.spot(to, side)
    await this.page.mouse.move(b.x + p.x, b.y + p.y, { steps: 8 })
    // A drag's events lag the pointer: nudged there until the page has seen it over that spot.
    for (let i = 0; i < 10 && (await this.markOn(to)) !== side; i++) await this.page.mouse.move(b.x + p.x + (i % 2), b.y + p.y, { steps: 2 })
  }

  /** The line Today draws on a frame while a drag is over it: on its top or bottom edge (`above`, `below`), its left or
   *  right one (`before`, `after`), or null. */
  async dropMark(label) {
    return this.markOn(this.frame(label))
  }

  async markOn(frame) {
    return frame.evaluate((el) => {
      const s = getComputedStyle(el, '::after')
      if (s.content === 'none' || s.content === 'normal') return null
      if (s.height === '3px' && s.width !== '3px') return s.top === '-6px' ? 'above' : 'below'
      return s.left === '-6px' ? 'before' : 'after'
    })
  }

  /** A row of Add a widget, by its widget's label; its Add button; whether it says "on Today". */
  catalogRow(label) {
    return this.widgetList.getByRole('listitem', { name: label, exact: true })
  }

  catalogAdd(label) {
    return this.widgetList.getByRole('button', { name: `Add ${label}`, exact: true })
  }

  catalogOn(label) {
    return this.catalogRow(label).getByText('on Today', { exact: true })
  }

  /** The labels Add a widget lists, in order. */
  async catalogLabels() {
    const out = []
    for (const r of await this.widgetList.getByRole('listitem').all()) out.push(await r.getAttribute('aria-label'))
    return out
  }

  part(name) {
    return this.parts.getByRole('button', { name, exact: true })
  }

  pickSize(size) {
    // Each size names its widget ("Threads at a glance: size S"): the one ending in the size.
    return this.pickSizes.getByRole('button', { name: new RegExp(`: size ${size}$`) })
  }

  /** The preview of the picked widget at a size: what can be dragged onto Today (its name says where: on Today). */
  preview(label, size) {
    return this.rightPanel.getByLabel(`Preview of ${label} at ${size} on Today`, { exact: true })
  }

  /** The element that has the keyboard: its accessible name (aria-label, else its text) and its role's tag. */
  async focused() {
    return this.page.evaluate(() => {
      const el = document.activeElement
      return { name: (el && (el.getAttribute('aria-label') || el.textContent || '').trim()) || '', tag: el ? el.tagName : '' }
    })
  }

  // ── The day's choices, Threads at a glance, Done this week, Recorded this week ──

  /** A line of The day's choices: meetings at once ("At once: A or B"), an invite ("Invite: <subject>"). */
  choice(name) {
    return this.widget(this.w.name.choices).getByRole('listitem', { name, exact: true })
  }

  /** The choices' lines, in order, by their names. */
  async choices() {
    const out = []
    for (const l of await this.widget(this.w.name.choices).getByRole('listitem').all()) out.push(await l.getAttribute('aria-label'))
    return out
  }

  choiceButton(name, button) {
    return this.choice(name).getByRole('button', { name: button, exact: true })
  }

  /** The buttons of a choice's line, in order: their texts and whether each is the filled (suggested) one. */
  async choiceButtons(name) {
    const out = []
    for (const b of await this.choice(name).getByRole('button').all()) {
      out.push({ name: (await b.innerText()).trim(), filled: await b.evaluate((el) => getComputedStyle(el).backgroundColor), disabled: await b.isDisabled() })
    }
    return out
  }

  /** A choice's time, its colour. */
  async choiceTime(name) {
    return this.choice(name).evaluate((el) => ({ text: el.firstElementChild.textContent.trim(), colour: getComputedStyle(el.firstElementChild).color }))
  }

  /** A meeting's name on a choice's line (a click opens its card). */
  choiceMeeting(name, subject) {
    // (The suggested one is named again in the line's sentence: the first is the name a click opens.)
    return this.choice(name).getByText(subject, { exact: true }).first()
  }

  /** A thread's line of Threads at a glance (its name says its counts), its link, its bar's segments by their titles. */
  glanceLine(thread) {
    return this.widget(this.w.name.glance).getByRole('listitem', { name: new RegExp(`^${literal(thread)}: `) })
  }

  async glanceLines() {
    const out = []
    for (const l of await this.widget(this.w.name.glance).getByRole('listitem').all()) out.push(await l.getAttribute('aria-label'))
    return out
  }

  glanceLink(thread) {
    return this.glanceLine(thread).getByRole('link', { name: thread, exact: true })
  }

  segment(thread, title) {
    return this.glanceLine(thread).getByTitle(title, { exact: true })
  }

  /** A segment's width and colour, once it is drawn; both read in one step in the page, by the glance's region, the
   *  thread's line (its list item) and the segment's title, as `segment` finds it: never from a segment drawn over in
   *  between (#476). */
  async segmentLook(thread, title) {
    await this.segment(thread, title).waitFor()
    return this.page.evaluate(([glance, thread, title]) => {
      const region = [...document.querySelectorAll('main section[aria-label], main [role="region"]')].find((el) => el.getAttribute('aria-label') === glance)
      const line = region && [...region.querySelectorAll('[role="listitem"]')].find((el) => (el.getAttribute('aria-label') || '').startsWith(`${thread}: `))
      const seg = line && [...line.querySelectorAll('[title]')].find((el) => el.getAttribute('title') === title)
      return seg ? { width: seg.getBoundingClientRect().width, colour: getComputedStyle(seg).backgroundColor } : null
    }, [this.w.name.glance, thread, title])
  }

  /** The total at a line's end. */
  glanceTotal(thread) {
    return this.glanceLine(thread).getByText(/^\d+$/)
  }

  /** A day's bar of Recorded this week ("Wednesday: 17 calls (today)"), its height, and whether its day name is bold. */
  weekDays() {
    return this.widget(this.w.name.week).getByRole('list', { name: 'Calls a day, Monday to Sunday', exact: true }).getByRole('listitem')
  }

  async weekBars() {
    const out = []
    for (const d of await this.weekDays().all()) {
      out.push(await d.evaluate((el) => {
        const [bar, day] = el.children
        const fill = bar.firstElementChild
        return { name: el.getAttribute('aria-label'), height: fill.getBoundingClientRect().height, full: bar.getBoundingClientRect().height, bold: Number(getComputedStyle(day).fontWeight) >= 600 }
      }))
    }
    return out
  }

  /** A date's weekday as the window names it ("Wednesday"): a `YYYY-MM-DD`. */
  async weekday(date) {
    return this.page.evaluate((d) => new Date(`${d}T12:00:00`).toLocaleDateString([], { weekday: 'long' }), date)
  }

  /** The command a notice shows as code ("echoctl calendar login"). */
  noticeCode() {
    return this.page.getByRole('status', { name: 'Notices', exact: true }).getByRole('code')
  }

  /** A plugin's status widget's dot: its colour (the dot is drawn, not read: it has no text of its own). */
  async dotColour(name) {
    return this.onWidget(name, (el) => {
      // An HTML dot, not a part of an icon (the header's refresh icon is drawn in SVG).
      const dot = [...el.querySelectorAll('*')].find((x) => x instanceof HTMLElement && !x.children.length && !x.textContent.trim() && x.getBoundingClientRect().width > 0 && x.getBoundingClientRect().width <= 16)
      return dot ? getComputedStyle(dot).backgroundColor : null
    })
  }

  /** A plugin's chart (its image, named by its points); its marks as drawn: what a hover on each says, and its colour. */
  chart(name) {
    // The chart's own image (svg), not the header's update stamp.
    return this.widget(name).locator('svg[role="img"]')
  }

  async chartMarks(name) {
    return this.chart(name).evaluate((svg) => [...svg.children].filter((m) => m.firstElementChild && m.firstElementChild.tagName === 'title')
      .map((m) => ({ says: m.firstElementChild.textContent, fill: getComputedStyle(m).fill, opacity: getComputedStyle(m).opacity })))
  }

  /** A widget's link, by its text. */
  widgetLink(name, text) {
    return this.widget(name).getByRole('link', { name: text, exact: true })
  }

  /** A widget's images (a progress bar, an `<img>`), not its header's update stamp. */
  widgetImages(name) {
    return this.widget(name).locator('img, [role="img"]:not(.pw-upd)')
  }

  // ── What a widget holds, for checks that read it (texts, headings, lists, titles) ──

  /** A text of a widget that matches a pattern (a time line, a sentence starting so). */
  widgetMatch(name, pattern) {
    return this.widget(name).getByText(pattern)
  }

  /** A heading inside a widget ("Walk in knowing", "The day's choices 5"), at a level. */
  widgetHeading(name, heading, level) {
    return this.widget(name).getByRole('heading', { name: heading, exact: true, level })
  }

  /** Any heading of a widget that starts with these words (to check it is gone). */
  widgetHeadings(name, words) {
    return this.widget(name).getByRole('heading', { name: new RegExp(`^${literal(words)}`) })
  }

  /** The items of a widget's list: its first or last list (0, -1), or the list of that name. */
  widgetItems(name, list = 0) {
    const lists = this.widget(name).getByRole('list', typeof list === 'string' ? { name: list, exact: true } : undefined)
    return (typeof list === 'string' ? lists : list < 0 ? lists.last() : lists.nth(list)).getByRole('listitem')
  }

  /** What a hover over a part of a widget says (its title). */
  widgetTitled(name, title) {
    return this.widget(name).getByTitle(title, { exact: true })
  }

  /** A widget's buttons whose names start with these words (a thread's chip: its icon's name, then its name). */
  widgetButtonStarting(name, words) {
    return this.widget(name).getByRole('button', { name: new RegExp(`^${literal(words)}\\b`) })
  }

  /** Every region of that name on Today (one widget's place: 1). */
  widgetsNamed(name) {
    return this.main.getByRole('region', { name, exact: true })
  }

  /** A widget's paragraph (a card's text) and its links in a list's item. */
  widgetParagraph(name) {
    return this.widget(name).getByRole('paragraph')
  }

  itemLinks(name, i) {
    return this.widgetItems(name).nth(i).getByRole('link')
  }

  /** A heading of the Details on the right (a task's title). */
  detailsHeading(title) {
    return this.rightPanel.getByRole('heading', { name: title, level: 3, exact: true })
  }

  // ── Your day's lines, by position and by what they say ──

  /** Every line of that subject; the line at a position; a line's text that matches. */
  linesOf(subject) {
    return this.yourDay.getByRole('listitem', { name: subject, exact: true })
  }

  lineAt(i) {
    return this.yourDay.getByRole('listitem').nth(i)
  }

  async lineCount() {
    return this.yourDay.getByRole('listitem').count()
  }

  lineMatch(subject, pattern) {
    return this.line(subject).getByText(pattern)
  }

  lineAtMatch(i, pattern) {
    return this.lineAt(i).getByText(pattern)
  }

  /** Show N skipped (its count read). */
  get showSkipped() {
    return this.yourDay.getByRole('button', { name: new RegExp(this.w.day.showSkipped) })
  }

  /** A meeting's card: its heading, a text starting so, a select, a button. */
  meetingCardHeading(subject) {
    return this.meetingCard(subject).getByRole('heading', { name: subject, exact: true })
  }

  meetingCardMatch(subject, pattern) {
    return this.meetingCard(subject).getByText(pattern)
  }

  meetingCardSelect(subject, name) {
    return this.meetingCard(subject).getByRole('combobox', { name, exact: true })
  }

  meetingCardButton(subject, name) {
    return this.meetingCard(subject).getByRole('button', { name, exact: true })
  }

  /** The strip's line now. */
  get stripNow() {
    return this.widget(this.w.name.strip).getByRole('separator', { name: new RegExp(`^${literal(this.w.day.nowPrefix)}`) })
  }

  // ── Edit mode's frames and Add a widget ──

  /** A frame's text as it reads; the first button of the widget inside it. */
  frameSays(label, text) {
    return this.frame(label).getByText(text, { exact: true })
  }

  frameInnerButton(label) {
    return this.frame(label).getByRole('region', { name: label, exact: true }).getByRole('button').first()
  }

  /** Add a widget's part buttons; the picked widget's preview, whichever it is. */
  get partButtons() {
    return this.parts.getByRole('button')
  }

  get anyPreview() {
    return this.rightPanel.getByLabel(new RegExp(`^${literal(this.w.edit.preview)}`))
  }

  // ── The day's choices ──

  /** A text of a choice's line, as it reads or containing these words; its buttons. */
  choiceSays(name, text) {
    return this.choice(name).getByText(text, { exact: true })
  }

  choiceContains(name, text) {
    return this.choice(name).getByText(text)
  }

  choiceAllButtons(name) {
    return this.choice(name).getByRole('button')
  }

  /** A choice's buttons read at one moment: their texts and whether each is disabled. */
  async choiceStates(name) {
    return this.choice(name).evaluate((el) => [...el.querySelectorAll('button')].map((b) => ({ text: b.textContent.trim(), disabled: b.disabled })))
  }

  /** A text of a widget, as it reads; one that matches. */
  widgetSays(name, text) {
    return this.widget(name).getByText(text, { exact: true })
  }

  widgetButton(name, button) {
    return this.widget(name).getByRole('button', { name: button, exact: true })
  }

  /** A widget's colour of a text (a plugin's tone, a dot). */
  async textColour(name, text) {
    return this.colourOf(this.widgetSays(name, text))
  }

  // ── A widget's settings (⚙ on its frame) and a plugin's own page ──

  /** A frame's ⚙ ("Done this week: settings"), the settings it opens under its header, one of them by its label. */
  settingsButton(label) {
    return this.frame(label).getByRole('button', { name: `${label}: settings`, exact: true })
  }

  settingsOf(label) {
    return this.frame(label).getByRole('group', { name: `Settings of ${label}`, exact: true })
  }

  setting(label, name) {
    return this.settingsOf(label).getByRole('combobox', { name: new RegExp(`^${literal(name)}\\b`) })
  }

  /** A plugin's own page (kind page) as its window half drew it: its boxes' texts, the size and theme Echo gave it (what
   *  its box says), and whether any text of the widget is italic (its text escaped: none). */
  async pageWidget(name) {
    return this.onWidget(name, (el) => {
      const box = el.querySelector('[data-size]')
      return box && {
        size: box.dataset.size,
        theme: box.dataset.theme,
        boxes: [...box.children].map((b) => b.innerText.replace(/\s+/g, ' ').trim()),
        italic: [...el.querySelectorAll('*')].some((x) => getComputedStyle(x).fontStyle === 'italic'),
      }
    })
  }
}
