// Calls: the list of calls, a call's tabs, and its Minutes tab (the minutes page in its frame, Write minutes).
import { AppPage } from './app.page.mjs'

export class CallsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.list = page.getByRole('listbox', { name: 'Calls' })
    this.writeMinutes = page.getByRole('button', { name: 'Write minutes' })
    // The minutes page, in the frame titled "Minutes" (the tab and a menu item share the name, not the role).
    this.minutes = page.locator('iframe[title="Minutes"]').contentFrame()
  }

  async open() {
    await this.goto('#calls')
  }

  /** A call in the list, by its title. */
  call(title) {
    return this.list.getByRole('option', { name: new RegExp(title) })
  }

  async openCall(title, tab) {
    await this.call(title).click()
    if (tab) await this.tab(tab).click()
  }

  tab(name) {
    return this.page.getByRole('tab', { name: new RegExp(`^${name}`) })
  }
}
