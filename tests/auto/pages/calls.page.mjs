// Calls: the list of calls, a call's tabs, and its Minutes tab (the minutes page in its frame, Write minutes).
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class CallsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.list = page.getByRole('listbox', { name: 'Calls' })
    this.writeMinutes = page.getByRole('button', { name: 'Write minutes' })
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

  tab(name) {
    return this.page.getByRole('tab', { name: new RegExp(`^${literal(name)}`) })
  }
}
