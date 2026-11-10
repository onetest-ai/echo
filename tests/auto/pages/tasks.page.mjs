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

  /** A filter's chosen option, as its chip reads it ("Globex pilot", "Due"). */
  chosen(name, text) {
    return this.select(name).getByRole('option', { name: text, exact: true, selected: true })
  }

  /** A filter's chip, as it reads ("Globex pilot ▾"; its select's options follow it in the page). */
  chip(text) {
    return this.main.getByText(new RegExp(`^${literal(text)} ▾`))
  }

  /** A pile, by its name ("I owe"): its heading, its cards, its "Nothing here" and its fold. */
  pileArea(name) {
    return this.main.getByRole('region', { name, exact: true })
  }

  /** A pile's cards (its fold's "+ N" and "Show fewer" left out), in their order. */
  cards(name) {
    return this.pileArea(name).getByRole('button').filter({ hasNotText: /^(\+ \d+|Show fewer)$/ })
  }

  /** A pile's fold ("+ 1", "Show fewer"). */
  fold(name, text) {
    return this.pileArea(name).getByRole('button', { name: text, exact: true })
  }

  /** The folded pile of others' tasks ("Others · 1 — …"); any count when n is left out. */
  others(n) {
    return this.main.getByRole('button', { name: new RegExp(`^Others · ${n ?? '\\d+'} `) })
  }

  /** A text of the view, as it reads. */
  says(text) {
    return this.main.getByText(text, { exact: true })
  }

  // ── Asks, in the right panel ──

  /** A person of Asks: their name, when you meet next, and what to ask them. */
  asker(name) {
    return this.rightPanel.getByRole('group', { name, exact: true })
  }

  /** A thing to ask, by the words it starts with (its name starts with the dot drawn before it). */
  ask(words) {
    return this.rightPanel.getByRole('button', { name: new RegExp(`^(· )?${literal(words)}`) })
  }
}
