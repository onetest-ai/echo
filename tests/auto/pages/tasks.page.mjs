// Tasks: its filters and its piles (I owe, Owed to me, …), each a heading with its count and the tasks under it.
import { AppPage } from './app.page.mjs'
import { literal } from '../support/data.mjs'
import { readLines } from '../support/aria.mjs'

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

  // ── A full board: a card by its title and thread, what it reads, its marks ──

  /** A pile's card (or Others' while it is open: its region is named "Others"), by its title as written and its
   *  thread. */
  card(pile, title, thread) {
    return this.pileArea(pile).getByRole('button').filter({ has: this.page.getByText(title, { exact: true }) }).filter({ hasText: thread })
  }

  /** A card in any pile, by its title as written and its thread. */
  anyCard(title, thread) {
    return this.main.getByRole('button').filter({ has: this.page.getByText(title, { exact: true }) }).filter({ hasText: thread })
  }

  /** What a card reads, in order: its title, the line under it, how it is done (the user's own tasks), its thread
   *  (twice for a thread with a picture of its own, which is named by it), how long it has been quiet, its due. */
  reads({ title, line, mode, thread, logo, quiet, due }) {
    return [title, line, mode, thread, logo && thread, quiet, due].filter(Boolean).join(' ')
  }

  /** A pile's "+ N" or "Show fewer", whichever it shows. */
  more(pile) {
    return this.pileArea(pile).getByRole('button', { name: /^(\+ \d+|Show fewer)$/ })
  }

  /** A card's due chip (titled "Due" or "Overdue"), or its "no date". */
  due(card, title) {
    return card.getByTitle(title, { exact: true })
  }

  undated(card) {
    return card.getByText('no date', { exact: true })
  }

  /** A card's due chip, whichever it is ("Due" or "Overdue"). */
  dueOf(card) {
    return card.getByTitle(/^(Due|Overdue)$/)
  }

  /** A pile's cards of one thread. */
  cardsOf(pile, thread) {
    return this.cards(pile).filter({ hasText: thread })
  }

  /** Every due chip of the view (its title "Due" or "Overdue"), and every "no date". */
  get dueChips() {
    return this.main.getByTitle(/^(Due|Overdue)$/)
  }

  get noDates() {
    return this.main.getByText('no date', { exact: true })
  }

  /** The chips of late tasks (titled "Overdue"). */
  get lateChips() {
    return this.main.getByTitle('Overdue', { exact: true })
  }

  /** A card's quiet mark ("quiet 5 d", titled "No word about it in 5 days"); every quiet mark of the view, and the
   *  cards of a thread that carry one. */
  quiet(card) {
    return card.getByTitle(/^No word about it in \d+ days$/)
  }

  get quietMarks() {
    return this.main.getByTitle(/^No word about it in \d+ days$/)
  }

  quietIn(thread) {
    return this.main.getByRole('button').filter({ has: this.page.getByTitle(/^No word about it in \d+ days$/) }).filter({ hasText: thread })
  }

  /** The red word "blocked" in the lines of a pile's cards. */
  blockedWords(pile) {
    return this.pileArea(pile).getByText('blocked', { exact: true })
  }

  /** How a card is done, its icon (named "Claude can do it", "You do it by hand: …"); the icons of a pile. */
  mode(card) {
    return card.getByLabel(/^(Claude can do it|You do it by hand)/)
  }

  modes(pile) {
    return this.pileArea(pile).getByLabel(/^(Claude can do it|You do it by hand)/)
  }

  /** The names the cards of a pile read, in order. */
  async names(pile) {
    const heading = new RegExp(`^${literal(pile)} \\d+$`)
    return readLines(await this.pileArea(pile).ariaSnapshot()).filter((l) => !heading.test(l) && !/^(\+ \d+|Show fewer)$/.test(l))
  }

  /** A filter's options, in order. */
  options(name) {
    return this.select(name).getByRole('option')
  }

  // ── Asks, in the right panel ──

  /** The people Asks names, in order. */
  get askers() {
    return this.rightPanel.getByRole('group')
  }

  /** The names of the people Asks lists, in order. */
  async askerNames() {
    const names = []
    for (const g of await this.askers.all()) names.push(await g.getAttribute('aria-label'))
    return names
  }

  /** When you meet a person next, as their block says it beside their name ("today · Weekly sync", "no meeting this
   *  week"). */
  askWhen(name) {
    return this.asker(name).getByText(/^(today|\w{3}) · .+$|^no meeting this week$/)
  }

  /** A person's things to ask, in order; the reasons after their dashes ("— due Tue, Nov 10"). */
  asksOf(name) {
    return this.asker(name).getByRole('button')
  }

  reasons(name) {
    return this.asker(name).getByText(/^— /)
  }

  /** A text of the right panel, as it reads. */
  panelSays(text) {
    return this.rightPanel.getByText(text, { exact: true })
  }


  /** A person of Asks: their name, when you meet next, and what to ask them. */
  asker(name) {
    return this.rightPanel.getByRole('group', { name, exact: true })
  }

  /** A thing to ask, by the words it starts with (its name starts with the dot drawn before it). */
  ask(words) {
    return this.rightPanel.getByRole('button', { name: new RegExp(`^(· )?${literal(words)}`) })
  }
}
