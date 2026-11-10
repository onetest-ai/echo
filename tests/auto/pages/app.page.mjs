// The Echo window as a whole: opening it on the data, and its frame every view shares — the top bar, the activity
// bar, the right and bottom panels, the status bar.
import { literal } from '../support/data.mjs'

export class AppPage {
  /** @param {import('@playwright/test').Page} page  @param data the spec file's data (support/data.mjs) */
  constructor(page, data) {
    this.page = page
    this.data = data
    this.statusBar = page.getByRole('contentinfo')
    this.topBar = page.getByRole('banner')
    this.expandSidebar = this.topBar.getByRole('button', { name: 'Expand the sidebar' })
    this.foldSidebar = this.topBar.getByRole('button', { name: 'Fold the sidebar' })
    this.record = this.topBar.getByRole('button', { name: 'Record next call' })
    this.memo = this.topBar.getByRole('button', { name: 'Record a memo' })
    this.account = this.topBar.getByRole('button', { name: /^AI profile:/ })
    this.inbox = this.topBar.getByRole('button', { name: 'Inbox', exact: true })
    this.activities = page.getByRole('navigation', { name: 'Activities' })
    this.rightPanel = page.getByRole('complementary', { name: 'Right panel' })
    this.bottomPanel = page.getByRole('region', { name: 'Bottom panel' })
  }

  /** A button of the activity bar, by its name. */
  activity(name) {
    return this.activities.getByRole('button', { name, exact: true })
  }

  /** An activity's name as drawn beside its icon (hidden while the bar is folded). */
  activityLabel(name) {
    return this.activities.getByText(name, { exact: true })
  }

  /** A panel's tab, by its name. */
  panelTab(panel, name) {
    return panel.getByRole('tab', { name, exact: true })
  }

  closePanel(panel) {
    return panel.getByRole('button', { name: 'Close the panel' })
  }

  /** A panel's button that moves its view to the other panel ("right", "bottom"). */
  movePanel(panel, to) {
    return panel.getByRole('button', { name: `Move to the ${to} panel` })
  }

  /** The route the window shows (`#today`, `#call/<folder>/minutes`). */
  hash() {
    return new URL(this.page.url()).hash
  }

  /** The view's main heading (level 2), by its name. */
  heading(name) {
    return this.main.getByRole('heading', { name, level: 2, exact: true })
  }

  /** A button of the status bar, by the word its name starts with ("Actions", "Claude"). */
  statusBarButton(word) {
    return this.statusBar.getByRole('button', { name: new RegExp(`(^|\\s)${word}\\b`) })
  }

  /** Do an action from ⌘K: open the palette, type what the action says, choose the action whose name starts so. */
  async action(words) {
    await this.page.keyboard.press('ControlOrMeta+k')
    await this.page.getByLabel('Action', { exact: true }).fill(words)
    await this.page.getByRole('option', { name: new RegExp(`^${literal(words)}`) }).first().click()
  }

  /** The top bar at the window's size: its four controls (the sidebar button, the Record pill — Record and Memo —, the
   *  account chip, Inbox) on one line, in order, apart, inside the window. */
  async topBarLine() {
    const parts = { flip: this.expandSidebar.or(this.foldSidebar), record: this.record, memo: this.memo, acct: this.account, inbox: this.inbox }
    const r = {}
    for (const [k, l] of Object.entries(parts)) r[k] = (await l.count()) ? await l.boundingBox() : null
    const names = Object.keys(r).filter((k) => r[k])
    const right = (b) => b.x + b.width
    const hit = (a, b) => a.x < right(b) && b.x < right(a) && a.y < b.y + b.height && b.y < a.y + a.height
    const mid = (b) => b.y + b.height / 2
    const width = this.page.viewportSize().width
    return {
      missing: Object.keys(r).filter((k) => !r[k]),
      overlap: names.some((a, i) => names.slice(i + 1).some((b) => hit(r[a], r[b]))),
      sameLine: names.every((k) => Math.abs(mid(r[k]) - mid(r.inbox)) <= 2),
      inOrder: right(r.flip) <= r.record.x && right(r.memo) <= r.acct.x && right(r.acct) <= r.inbox.x,
      inside: right(r.inbox) <= width,
    }
  }

  /** A text of the page, as it reads. */
  text(t) {
    return this.page.getByText(t, { exact: true })
  }

  /** A text of the page that starts with these words. */
  textStarting(t) {
    return this.page.getByText(new RegExp(`^${literal(t)}`))
  }

  /** The window at a route (`#today`, `#calls`, …): its cookie first, as a browser gets it. */
  async goto(hash = '') {
    await this.page.goto(this.data.base + '/')
    await this.page.goto(this.data.base + '/' + hash)
  }

  /** The view's own content (the primary area's main element). */
  get main() {
    return this.page.getByRole('main').first()
  }

  /** A time as the window writes it ("11:07 AM", by the browser's locale): an ISO time from a file or the CLI. */
  async clock(iso) {
    return this.page.evaluate((t) => new Date(t).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }), iso)
  }

  /** A date in the coming week as the board writes it ("Tue, Nov 10"): a `YYYY-MM-DD` from a file or the CLI. */
  async day(date) {
    return this.page.evaluate((d) => new Date(d + 'T12:00').toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }), date)
  }

  /** A date as a prep writes it beside an item ("Nov 10"): a `YYYY-MM-DD` from a file or the CLI. */
  async shortDay(date) {
    return this.page.evaluate((d) => new Date(d + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }), date)
  }

  /** Let a minute pass on the page's clock (after `installClock`): timers fire, the view's minute-based lines move. */
  async installClock() {
    await this.page.clock.install()
  }

  async minutePasses() {
    await this.page.clock.fastForward('01:05')
  }
}
