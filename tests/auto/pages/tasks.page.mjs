// Tasks: its filters and its piles (I owe, Owed to me, …), each a heading with its count and the tasks under it.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'

export class TasksPage extends AppPage {
  async open() {
    await this.goto('#tasks')
  }

  /** A filter of the page ("Thread", "Who does it", "Order"). */
  select(name) {
    return this.main.getByRole('combobox', { name, exact: true })
  }

  /** A pile's heading, with the count it shows ("I owe 1"). */
  pile(name, n) {
    return this.main.getByRole('heading', { name: `${name} ${n}`, exact: true, level: 3 })
  }

  /** A task, by its title. */
  task(title) {
    return this.main.getByRole('button', { name: new RegExp(`^${literal(title)} `) })
  }
}
