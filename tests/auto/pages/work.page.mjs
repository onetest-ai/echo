// A thread's Work tab and a work item's details in the right panel.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class WorkPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.details = page.getByRole('complementary', { name: 'Right panel' })
  }

  async open(thread) {
    await this.goto(`#project/${thread}/work`)
  }

  /** Open a work item's details by its title. */
  async openItem(title) {
    await this.main.getByText(title, { exact: true }).first().click()
    await this.page.getByRole('heading', { name: title, level: 3 }).waitFor()
  }

  /** A field's value in the details, by the field's name ("Owner", "Handed over by", "Due"). */
  value(field) {
    return this.details.getByRole('button', { name: new RegExp(`^${literal(field)}:`) })
  }

  /** Open a field for a change: its box, by the field's name. */
  async edit(field) {
    await this.value(field).click()
    return this.details.getByLabel(field, { exact: true })
  }

  /** The names a field's box suggests as it is typed in. */
  async suggestions(box) {
    return box.evaluate((el) => (el.list ? [...el.list.options].map((o) => o.value) : []))
  }
}
