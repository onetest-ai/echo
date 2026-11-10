// Calls: the list of calls, a call's tabs, and its Minutes tab (the minutes page in its frame, Write minutes).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

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

  /** A thread in the "+ Thread" menu (a checkbox item, ticked while the call is in it), or another item of a menu. */
  threadItem(name) {
    return this.page.getByRole('menu').getByRole('menuitemcheckbox', { name: new RegExp(`^(✓ )?${literal(name)}$`) })
  }

  get menuItems() {
    return this.page.getByRole('menu').getByRole('menuitem')
  }

  /** The transcript's turns, by their times. */
  get turns() {
    return this.main.getByRole('time')
  }
}
