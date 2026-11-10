// Settings: its sections (the list on the left) and Layout (where each panel view opens, Reset).
import { AppPage } from './app.page.mjs'

export class SettingsPage extends AppPage {
  constructor(page, data) {
    super(page, data)
    this.sections = this.main.getByRole('complementary')
    this.reset = this.main.getByRole('button', { name: 'Reset', exact: true })
    // Layout's rows: a view's Right and Bottom, in a group named as the view.
    this.layoutRows = this.main.getByRole('group').filter({ has: page.getByRole('button', { name: 'Bottom', exact: true }) })
  }

  async open(route) {
    await this.goto(route)
  }

  /** A section of the list on the left, by its name. */
  section(name) {
    return this.sections.getByRole('button', { name, exact: true })
  }

  /** Layout: a view's place button ("Right", "Bottom"). */
  place(view, where) {
    return this.main.getByRole('group', { name: view, exact: true }).getByRole('button', { name: where, exact: true })
  }
}
